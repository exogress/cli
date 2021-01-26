use exogress_common::config_core::{
    default_rules, Action, CatchAction, CatchMatcher, ClientConfig, ClientHandler,
    ClientHandlerVariant, Filter, MatchPathSegment, MatchingPath, MethodMatcher, Proxy, RescueItem,
    Rule, StaticDir, StatusCodeRange, UpstreamDefinition, UpstreamSocketAddr, DEFAULT_CONFIG_FILE,
};
use exogress_common::config_core::{ClientMount, CURRENT_VERSION};
use exogress_common::entities::{MountPointName, Upstream};
use http::{Method, StatusCode};
use maplit::btreemap;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

pub fn create_exofile(
    dir: impl AsRef<Path>,
    config_file: impl AsRef<Path>,
    framework: &str,
) -> Result<(), anyhow::Error> {
    let file_path = dir.as_ref().join(config_file);

    let cfg = match framework {
        "rails" => default_config_rails(),
        "svelte" => default_config_svelte(),
        _ => ClientConfig::sample(None, None, None, None),
    };

    let str = serde_yaml::to_vec(&cfg)?;

    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file_path)?;
    f.write_all(str.as_ref())?;

    info!("{} successfully created", DEFAULT_CONFIG_FILE);

    Ok(())
}

fn default_config_rails() -> ClientConfig {
    let mut upstreams = BTreeMap::new();
    let upstream: Upstream = "rails-server".parse().unwrap();
    upstreams.insert(
        upstream.clone(),
        UpstreamDefinition {
            addr: UpstreamSocketAddr {
                port: 3000,
                host: None,
            },
            health_checks: Default::default(),
        },
    );

    let static_rules = vec![
        Rule {
            filter: Filter {
                path: MatchingPath::LeftWildcard(vec![MatchPathSegment::Exact(
                    "assets".parse().unwrap(),
                )]),
                methods: MethodMatcher::Exact(vec![Method::GET, Method::HEAD]),
                trailing_slash: Default::default(),
            },
            action: Action::Invoke {
                modify_request: None,
                modify_response: vec![],
                rescue: vec![],
            },
            profiles: None,
        },
        Rule {
            filter: Filter {
                path: MatchingPath::Wildcard,
                methods: MethodMatcher::Exact(vec![Method::GET, Method::HEAD]),
                trailing_slash: Default::default(),
            },
            action: Action::Invoke {
                modify_request: None,
                modify_response: vec![],
                rescue: vec![RescueItem {
                    catch: CatchMatcher::StatusCode(StatusCodeRange::Single(StatusCode::NOT_FOUND)),
                    handle: CatchAction::NextHandler,
                }],
            },
            profiles: None,
        },
    ];

    let mut handlers = BTreeMap::new();
    handlers.insert(
        "public".parse().unwrap(),
        ClientHandler {
            variant: ClientHandlerVariant::StaticDir(StaticDir {
                dir: "./public".parse().unwrap(),
                rebase: Default::default(),
            }),
            rules: static_rules,
            priority: 10,
            rescue: Default::default(),
            profiles: None,
        },
    );
    handlers.insert(
        "rails-server".parse().unwrap(),
        ClientHandler {
            variant: ClientHandlerVariant::Proxy(Proxy {
                upstream,
                rebase: Default::default(),
            }),
            rules: default_rules(),
            priority: 50,
            rescue: Default::default(),
            profiles: None,
        },
    );

    let mount_points = btreemap! {
        MountPointName::from_str("default").unwrap() => ClientMount {
            handlers,
            rescue: Default::default(),
            static_responses: Default::default(),
            profiles: Default::default(),
        }
    };

    ClientConfig {
        version: CURRENT_VERSION.clone(),
        revision: 1.into(),
        name: "rails".parse().unwrap(),
        mount_points,
        upstreams,
        static_responses: Default::default(),
        rescue: vec![],
    }
}

fn default_config_svelte() -> ClientConfig {
    let mut upstreams = BTreeMap::new();
    let upstream: Upstream = "svelte-dev-server".parse().unwrap();
    upstreams.insert(
        upstream.clone(),
        UpstreamDefinition {
            addr: UpstreamSocketAddr {
                port: 5000,
                host: None,
            },
            health_checks: Default::default(),
        },
    );

    let mut handlers = BTreeMap::new();
    handlers.insert(
        "built-assets".parse().unwrap(),
        ClientHandler {
            variant: ClientHandlerVariant::StaticDir(StaticDir {
                dir: "./public".parse().unwrap(),
                rebase: Default::default(),
            }),
            rules: default_rules(),
            priority: 50,
            rescue: Default::default(),
            profiles: Some(vec!["production".parse().unwrap()]),
        },
    );
    handlers.insert(
        "dev-server".parse().unwrap(),
        ClientHandler {
            variant: ClientHandlerVariant::Proxy(Proxy {
                upstream,
                rebase: Default::default(),
            }),
            rules: default_rules(),
            priority: 50,
            rescue: Default::default(),
            profiles: Some(vec!["develop".parse().unwrap()]),
        },
    );

    let mount_points = btreemap! {
        MountPointName::from_str("default").unwrap() => ClientMount {
            handlers,
            rescue: Default::default(),
            static_responses: Default::default(),
            profiles: Default::default(),
        }
    };

    ClientConfig {
        version: CURRENT_VERSION.clone(),
        revision: 1.into(),
        name: "svelte".parse().unwrap(),
        mount_points,
        upstreams: Default::default(),
        static_responses: Default::default(),
        rescue: vec![],
    }
}
