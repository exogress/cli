#[macro_use]
extern crate tracing;

mod init;
mod termination;

use std::collections::VecDeque;
use std::process::Stdio;

use crate::termination::StopReason;
use clap::{crate_version, App, Arg};
use exogress_client_core::{Client, DEFAULT_CLOUD_ENDPOINT};
use exogress_common_utils::termination::stop_signal_listener;
use exogress_entities::{LabelName, LabelValue, Ulid, Upstream};
use futures::future::Either;
use futures::{future, select_biased, FutureExt};
use stop_handle::stop_handle;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::runtime::{Builder, Handle};

use exogress_config_core::{UpstreamSocketAddr, DEFAULT_CONFIG_FILE};
use hashbrown::HashMap;
use std::str::FromStr;
use trust_dns_resolver::TokioAsyncResolver;
use url::Url;

pub fn main() {
    let spawn_args = App::new("spawn")
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
            Arg::with_name("upstream_addr")
                .long("upstream-addr")
                .value_name("UPSTREAM_NAME=SOCKET_ADDR")
                .help("Redefine upstream address ")
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

    let args = App::new("Exogress CLI")
        .version(crate_version!())
        .author("Exogress Team <team@exogress.com>")
        .about("Expose your app to Exogress cloud load balancer")
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
        .subcommand(App::new("init").about("create Exofile"))
        .subcommand(exogress_common_utils::clap::threads::add_args(
            exogress_common_utils::clap::log::add_args(spawn_args),
        ));

    let mut args = exogress_common_utils::clap::autocompletion::add_args(args);

    let matches = args.clone().get_matches();
    exogress_common_utils::clap::autocompletion::handle_autocompletion(
        &mut args, &matches, "exogress",
    );

    if matches.subcommand_matches("init").is_some() {
        init::create_exofile(".").expect("Could not init");

        std::process::exit(0);
    }

    let spawn_matches = matches
        .subcommand_matches("spawn")
        .expect("unknown subcommand");

    let cloud_endpoint: Url = std::env::var("EXG_CLOUD_ENDPOINT")
        .unwrap_or_else(|_| DEFAULT_CLOUD_ENDPOINT.to_string())
        .parse()
        .expect("bad cloud endpoint provided");

    exogress_common_utils::clap::log::handle(&spawn_matches, "exogress");
    let num_threads = exogress_common_utils::clap::threads::extract_matches(&spawn_matches);

    let mut rt = Builder::new()
        .threaded_scheduler()
        .enable_all()
        .core_threads(num_threads)
        .thread_name("exogress-reactor")
        .build()
        .unwrap();

    let config_path = matches
        .value_of("config")
        .expect("--config is not set")
        .to_string();
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

    info!("labels = {:?}", labels);

    let upstreame_redefinitions = spawn_matches
        .values_of("upstream_addr")
        .map(|v| {
            v.map(|v| {
                let mut kv = v.split('=');
                let upstream_name: Upstream = kv
                    .next()
                    .expect("bad upstream addr format")
                    .parse()
                    .expect("bad upstream name");
                let addr: UpstreamSocketAddr = kv
                    .next()
                    .expect("bad upstream addr format")
                    .parse()
                    .expect("bad upstream socket addr");
                assert!(kv.next().is_none(), "bad upstream addr format");
                (upstream_name, addr)
            })
        })
        .into_iter()
        .flatten()
        .collect::<HashMap<Upstream, UpstreamSocketAddr>>();

    info!("upstreame_redefinition = {:?}", upstreame_redefinitions);

    let (app_stop_handle, app_stop_wait) = stop_handle::<StopReason>();

    rt.block_on(async move {
        tokio::spawn(stop_signal_listener(app_stop_handle.clone()));

        let resolver = TokioAsyncResolver::from_system_conf(Handle::current())
            .await
            .unwrap();

        let process = match spawn_matches.values_of("command") {
            Some(cmd_and_args) if cmd_and_args.len() > 0 => {
                let mut c: VecDeque<_> = cmd_and_args.collect();

                let mut command = Command::new(c.pop_front().expect("FIXME"));

                command
                    .args(c)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                let mut child = command.spawn().expect("failed to spawn command");

                let stderr = child.stderr.take().unwrap();
                let stdout = child.stdout.take().unwrap();

                let mut stdout_reader = BufReader::new(stdout).lines();
                let mut stderr_reader = BufReader::new(stderr).lines();

                let stdout_forward = {
                    async move {
                        while let Ok(Some(line)) = stdout_reader.next_line().await {
                            info!("O {}", line);
                        }
                    }
                }
                .fuse();

                let stderr_forward = {
                    async move {
                        while let Ok(Some(line)) = stderr_reader.next_line().await {
                            info!("E {}", line);
                        }
                    }
                }
                .fuse();

                Either::Left(async move {
                    futures::pin_mut!(stdout_forward);
                    futures::pin_mut!(stderr_forward);

                    select_biased! {
                        _ = child.fuse() => {}
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
            .refined_upstream_addrs(upstreame_redefinitions)
            .labels(labels)
            .build()
            .unwrap()
            .spawn(resolver)
            .fuse();

        tokio::select! {
            r = client => {
                if let Err(e) = r {
                    error!("client stopped with error: {:?}", e);
                }
            },
            _ = process => {},
            r = app_stop_wait => {
                info!("stop request received: {}", r);
            },
        }
    });

    info!("we are done");
}
