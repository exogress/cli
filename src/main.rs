#[macro_use]
extern crate tracing;
#[macro_use]
extern crate shadow_clone;

mod init;
mod termination;

use std::{collections::VecDeque, process::Stdio};

use crate::termination::StopReason;
use clap::{crate_version, App, Arg};
use exogress_common::{
    client_core::{Client, DEFAULT_CLOUD_ENDPOINT},
    common_utils::termination::stop_signal_listener,
    entities::{LabelName, LabelValue, ProfileName, Ulid},
};
use futures::{future, future::Either, select_biased, FutureExt};
use stop_handle::stop_handle;
use tokio::{process::Command, runtime::Builder};

use exogress_common::{config_core::DEFAULT_CONFIG_FILE, entities::SmolStr};
use futures::channel::mpsc;
use hashbrown::HashMap;
use std::str::FromStr;
use trust_dns_resolver::{TokioAsyncResolver, TokioHandle};
use url::Url;

#[cfg(feature = "jemalloc")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

pub fn main() {
    let spawn_app = App::new("spawn")
        .about("spawn exogress client")
        .arg(
            Arg::with_name("no_watch_config")
                .long("no-watch")
                .help("Don't watch for config changes")
                .takes_value(false)
                .required(false),
        )
        .arg(
            Arg::with_name("access_key_id")
                .long("access-key-id")
                .value_name("ULID")
                .help("ACCESS_KEY_ID")
                .env("EXG_ACCESS_KEY_ID")
                .hide_env_values(true)
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("secret_access_key")
                .long("secret-access-key")
                .value_name("STRING")
                .help("SECRET_ACCESS_KEY")
                .env("EXG_SECRET_ACCESS_KEY")
                .hide_env_values(true)
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("account")
                .long("account")
                .value_name("STRING")
                .env("EXG_ACCOUNT")
                .help("Account")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("profile")
                .short("p")
                .long("profile")
                .help("Profile name")
                .env("EXG_PROFILE")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("project")
                .long("project")
                .value_name("STRING")
                .help("Project")
                .env("EXG_PROJECT")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("label")
                .long("label")
                .short("l")
                .value_name("KEY=VALUE")
                .help("Attach label to running instance")
                .multiple(true)
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("command")
                .help("Run this command")
                .last(true)
                .multiple(true),
        );

    let version = format!(
        "{} (lib {}, config {})",
        crate_version!(),
        exogress_common::client_core::VERSION,
        exogress_common::config_core::CURRENT_VERSION.0,
    );

    let app = App::new("Exogress Command-Line Client")
        .version(version.as_str())
        .author("Exogress Team <team@exogress.com>")
        .about("Exogress command-line client. See https://exogress.com for more details.")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .env("EXG_CONFIG_FILE")
                .default_value(DEFAULT_CONFIG_FILE)
                .takes_value(true),
        )
        .subcommand(init::init_app())
        .subcommand(exogress_common::common_utils::clap::threads::add_args(
            exogress_common::common_utils::clap::log::add_args(spawn_app),
        ));

    let mut app = exogress_common::common_utils::clap::autocompletion::add_args(app);

    let matches = app.clone().get_matches();

    let config_path = matches
        .value_of("config")
        .expect("--config is not set")
        .to_string();

    if let Some(init_subcommand) = matches.subcommand_matches("init") {
        init::handle_subcommand(init_subcommand);
        std::process::exit(0);
    }

    exogress_common::common_utils::clap::autocompletion::handle_autocompletion(
        &mut app.clone(),
        &matches,
        "exogress",
    );

    let spawn_matches = if let Some(matches) = matches.subcommand_matches("spawn") {
        matches
    } else {
        app.print_long_help().unwrap();
        println!();
        std::process::exit(1);
    };

    let cloud_endpoint: Url = std::env::var("EXG_CLOUD_ENDPOINT")
        .unwrap_or_else(|_| DEFAULT_CLOUD_ENDPOINT.to_string())
        .parse()
        .expect("bad cloud endpoint provided");

    let gw_tunnels_port: u16 = std::env::var("EXG_GW_TUNNELS_PORT")
        .map(|v| v.parse().expect("bad EXG_GW_TUNNELS_PORT"))
        .unwrap_or_else(|_| 443);

    exogress_common::common_utils::clap::log::handle(&spawn_matches, "exogress");
    let num_threads = exogress_common::common_utils::clap::threads::extract_matches(&spawn_matches);

    let rt = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(num_threads)
        .thread_name("exogress-reactor")
        .build()
        .unwrap();

    let should_watch_config = !spawn_matches.is_present("no_watch_config");

    let access_key_id: Ulid = spawn_matches
        .value_of("access_key_id")
        .expect("access_key_id is not set")
        .parse()
        .expect("access_key_id is not ULID");

    let secret_access_key = spawn_matches
        .value_of("secret_access_key")
        .expect("secret_access_key is not set")
        .to_string();

    let account = spawn_matches
        .value_of("account")
        .expect("account is not set")
        .to_string();

    let project = spawn_matches
        .value_of("project")
        .expect("project is not set")
        .to_string();

    let profile: Option<ProfileName> = spawn_matches
        .value_of("profile")
        .map(|p| p.parse().expect("Bad profile name"));

    let labels = spawn_matches
        .values_of("label")
        .map(|v| {
            v.map(|v| {
                let mut kv = v.split('=');
                let k = kv.next().expect("bad label format");
                let v = kv.next().expect("bad label format");
                assert!(kv.next().is_none(), "bad label format");
                let expanded_v = shellexpand::env(v).expect("Could not expand value");
                (
                    LabelName::from_str(k).expect("bad label name format"),
                    LabelValue::from_str(&expanded_v).expect("bad label value"),
                )
            })
        })
        .into_iter()
        .flatten()
        .collect::<HashMap<LabelName, LabelValue>>();

    let (app_stop_handle, app_stop_wait) = stop_handle::<StopReason>();

    rt.block_on(async move {
        tokio::spawn(stop_signal_listener(app_stop_handle.clone()));

        let (reload_config_tx, reload_config_rx) = mpsc::unbounded();

        #[cfg(unix)]
        tokio::spawn({
            shadow_clone!(reload_config_tx);

            async move {
                let mut hup_listener =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()).unwrap();
                while hup_listener.recv().await.is_some() {
                    info!("SIGHUP received");
                    reload_config_tx.unbounded_send(()).unwrap();
                }
            }
        });

        let resolver = TokioAsyncResolver::from_system_conf(TokioHandle).unwrap();

        let process = match spawn_matches.values_of("command") {
            Some(cmd_and_args) if cmd_and_args.len() > 0 => {
                let mut commands_deque: VecDeque<_> = cmd_and_args.collect();

                // We may unwrap here because cmd_and_args is > 0
                let mut command = Command::new(commands_deque.pop_front().unwrap());

                command
                    .args(commands_deque)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                let mut child = command.spawn().expect("failed to spawn command");

                let mut stderr = child.stderr.take().unwrap();
                let mut stdout = child.stdout.take().unwrap();

                let stdout_forward = async move {
                    tokio::io::copy(&mut stdout, &mut tokio::io::stdout())
                        .await
                        .ok();
                }
                .fuse();

                let stderr_forward = async move {
                    tokio::io::copy(&mut stderr, &mut tokio::io::stderr())
                        .await
                        .ok();
                }
                .fuse();

                Either::Left(async move {
                    futures::pin_mut!(stdout_forward);
                    futures::pin_mut!(stderr_forward);

                    let wait_child = child.wait().fuse();

                    futures::pin_mut!(wait_child);

                    select_biased! {
                        _ = wait_child => {}
                        _ = stdout_forward => {}
                        _ = stderr_forward => {}
                    }
                })
            }
            _ => {
                info!("running in standalone mode");
                Either::Right(future::pending())
            }
        }
        .fuse();

        let client = Client::builder()
            .config_path(config_path)
            .access_key_id(access_key_id)
            .secret_access_key(secret_access_key)
            .cloud_endpoint(cloud_endpoint.to_string())
            .account(account)
            .project(project)
            .watch_config(should_watch_config)
            .labels(labels)
            .profile(profile)
            .gw_tunnels_port(gw_tunnels_port)
            .additional_connection_params({
                let mut map = HashMap::<SmolStr, SmolStr>::new();
                map.insert("client".into(), "cli".into());
                map.insert("cli_version".into(), crate_version!().into());
                map
            })
            .build()
            .unwrap()
            .spawn(reload_config_tx, reload_config_rx, resolver)
            .fuse();

        tokio::select! {
            r = client => {
                if let Err(e) = r {
                    error!("Client stopped with error: {}", e);
                }
            },
            _ = process => {},
            r = app_stop_wait => {
                info!("Stop request received: {}", r);
            },
        }
    });

    info!("Exiting");
}
