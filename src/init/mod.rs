use exogress_common::config_core::{
    default_rules, Action, CatchAction, CatchMatcher, ClientConfig, ClientHandler,
    ClientHandlerVariant, Filter, MatchPathSegment, MatchPathSingleSegment, MatchingPath,
    MethodMatcher, Proxy, RescueItem, Rule, StaticDir, StatusCodeRange, TrailingSlashFilterRule,
    UpstreamDefinition, UpstreamSocketAddr, DEFAULT_CONFIG_FILE,
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

pub mod matrix_synapse;

pub fn create_exofile(
    dir: impl AsRef<Path>,
    config_file: impl AsRef<Path>,
    framework: &str,
) -> Result<(), anyhow::Error> {
    let file_path = dir.as_ref().join(config_file);

    let cfg = match framework {
        "rails" => default_config_rails(),
        "svelte" => default_config_svelte(),
        "laravel-artisan" => default_config_laravel(),
        "synaps" => {
            matrix_synapse::generate(true, true).expect("Error creating configuration");
            return Ok(());
        }
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
            profiles: None,
        },
    );

    let static_rules = vec![
        Rule {
            filter: Filter {
                path: MatchingPath::LeftWildcard(vec![MatchPathSegment::Single(
                    MatchPathSingleSegment::Exact("assets".parse().unwrap()),
                )]),
                query: Default::default(),
                methods: MethodMatcher::Exact(vec![Method::GET, Method::HEAD]),
                trailing_slash: Default::default(),
            },
            action: Action::Invoke {
                modify_request: None,
                on_response: vec![],
                rescue: vec![],
            },
            profiles: None,
        },
        Rule {
            filter: Filter {
                path: MatchingPath::Wildcard,
                query: Default::default(),
                methods: MethodMatcher::Exact(vec![Method::GET, Method::HEAD]),
                trailing_slash: Default::default(),
            },
            action: Action::Invoke {
                modify_request: None,
                on_response: vec![],
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
                cache: Default::default(),
                post_processing: Default::default(),
            }),
            rules: static_rules,
            priority: 10,
            rescue: Default::default(),
            profiles: None,
            languages: None,
        },
    );
    handlers.insert(
        "rails-server".parse().unwrap(),
        ClientHandler {
            variant: ClientHandlerVariant::Proxy(Proxy {
                upstream,
                rebase: Default::default(),
                cache: Default::default(),
                post_processing: Default::default(),
                websockets: true,
            }),
            rules: default_rules(),
            priority: 50,
            rescue: Default::default(),
            profiles: None,
            languages: None,
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
            profiles: Some(vec!["develop".parse().unwrap()]),
        },
    );

    let mut handlers = BTreeMap::new();
    handlers.insert(
        "built-assets".parse().unwrap(),
        ClientHandler {
            variant: ClientHandlerVariant::StaticDir(StaticDir {
                dir: "./public".parse().unwrap(),
                rebase: Default::default(),
                cache: Default::default(),
                post_processing: Default::default(),
            }),
            rules: default_rules(),
            priority: 50,
            rescue: Default::default(),
            profiles: Some(vec!["production".parse().unwrap()]),
            languages: None,
        },
    );
    handlers.insert(
        "dev-server".parse().unwrap(),
        ClientHandler {
            variant: ClientHandlerVariant::Proxy(Proxy {
                upstream,
                rebase: Default::default(),
                cache: Default::default(),
                post_processing: Default::default(),
                websockets: true,
            }),
            rules: default_rules(),
            priority: 50,
            rescue: Default::default(),
            profiles: Some(vec!["develop".parse().unwrap()]),
            languages: None,
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

fn default_config_laravel() -> ClientConfig {
    let mut upstreams = BTreeMap::new();
    let upstream: Upstream = "artisan-server".parse().unwrap();
    upstreams.insert(
        upstream.clone(),
        UpstreamDefinition {
            addr: UpstreamSocketAddr {
                port: 8000,
                host: None,
            },
            health_checks: Default::default(),
            profiles: None,
        },
    );

    let static_rules = vec![
        Rule {
            filter: Filter {
                path: MatchingPath::Strict(vec![MatchPathSegment::Single(
                    MatchPathSingleSegment::Exact("index.php".parse().unwrap()),
                )]),
                query: Default::default(),
                methods: MethodMatcher::All,
                trailing_slash: TrailingSlashFilterRule::Deny,
            },
            action: Action::NextHandler,
            profiles: None,
        },
        Rule {
            filter: Filter {
                path: MatchingPath::Wildcard,
                query: Default::default(),
                methods: MethodMatcher::Exact(vec![Method::GET, Method::HEAD]),
                trailing_slash: Default::default(),
            },
            action: Action::Invoke {
                modify_request: None,
                on_response: vec![],
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
        "laravel".parse().unwrap(),
        ClientHandler {
            variant: ClientHandlerVariant::Proxy(Proxy {
                upstream,
                rebase: Default::default(),
                cache: Default::default(),
                post_processing: Default::default(),
                websockets: true,
            }),
            rules: default_rules(),
            priority: 50,
            rescue: Default::default(),
            profiles: None,
            languages: None,
        },
    );
    handlers.insert(
        "public".parse().unwrap(),
        ClientHandler {
            variant: ClientHandlerVariant::StaticDir(StaticDir {
                dir: "./public".parse().unwrap(),
                rebase: Default::default(),
                cache: Default::default(),
                post_processing: Default::default(),
            }),
            rules: static_rules,
            priority: 10,
            rescue: Default::default(),
            profiles: None,
            languages: None,
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
        name: "laravel".parse().unwrap(),
        mount_points,
        upstreams,
        static_responses: Default::default(),
        rescue: vec![],
    }
}
