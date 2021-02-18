use exogress_common::{
    config_core::{referenced::Container, refinable::Refinable, *},
    entities::*,
};
use include_dir::{include_dir, Dir};
use maplit::btreemap;
use std::{collections::BTreeMap, fs, str::FromStr};

static SKELETON: Dir = include_dir!("platforms_templates/synapse");

fn synapse_server_config() -> ClientConfig {
    let mut upstreams = BTreeMap::new();
    let upstream: Upstream = "synapse".parse().unwrap();
    upstreams.insert(
        upstream.clone(),
        UpstreamDefinition {
            addr: UpstreamSocketAddr {
                port: 8008,
                host: None,
            },
            health_checks: Default::default(),
            profiles: None,
        },
    );

    let rules = vec![
        Rule {
            filter: Filter {
                path: MatchingPath::Strict(vec![
                    MatchPathSegment::Single(MatchPathSingleSegment::Exact(
                        ".well-known".parse().unwrap(),
                    )),
                    MatchPathSegment::Single(MatchPathSingleSegment::Exact(
                        "matrix".parse().unwrap(),
                    )),
                    MatchPathSegment::Single(MatchPathSingleSegment::Exact(
                        "server".parse().unwrap(),
                    )),
                ]),
                query_params: Default::default(),
                methods: MethodMatcher::All,
                trailing_slash: Default::default(),
            },
            action: Action::Respond {
                static_response: Container::Shared("delegation".parse().unwrap()),
                status_code: None,
                data: Default::default(),
                rescue: vec![],
            },
            profiles: None,
        },
        Rule {
            filter: Filter {
                path: MatchingPath::LeftWildcard(vec![MatchPathSegment::Choice(vec![
                    "_synapse".parse().unwrap(),
                    "_matrix".parse().unwrap(),
                    "_client".parse().unwrap(),
                ])]),
                query_params: Default::default(),
                methods: MethodMatcher::All,
                trailing_slash: Default::default(),
            },
            action: Action::Invoke {
                modify_request: None,
                on_response: vec![],
                rescue: vec![],
            },
            profiles: None,
        },
    ];

    let mut handlers = BTreeMap::new();
    handlers.insert(
        "synapse".parse().unwrap(),
        ClientHandler {
            variant: ClientHandlerVariant::Proxy(Proxy {
                upstream,
                rebase: Default::default(),
                cache: Default::default(),
                post_processing: Default::default(),
                websockets: true,
            }),
            rules,
            priority: 50,
            refinable: Refinable::default(),
            profiles: None,
            // languages: None,
        },
    );

    let mount_points = btreemap! {
        MountPointName::from_str("default").unwrap() => ClientMount {
            handlers,
            refinable: Refinable::default(),
            profiles: Default::default(),
        }
    };

    ClientConfig {
        version: CURRENT_VERSION.clone(),
        revision: 1.into(),
        name: "synapse".parse().unwrap(),
        mount_points,
        upstreams,
        refinable: Refinable {
            static_responses: btreemap! {
                StaticResponseName::from_str("delegation").unwrap() => StaticResponse::Raw(RawResponse {
                    fallback_accept: Some(mime::APPLICATION_JSON.into()),
                    status_code: http::StatusCode::OK.into(),
                    body: vec![
                        ResponseBody {
                            content_type: mime::APPLICATION_JSON.into(),
                            content: "{ \"m.server\": \"{{ this.facts.mount_point_hostname }}:443\" }".into(),
                            engine: Some(TemplateEngine::Handlebars),
                        }
                    ],
                    headers: Default::default(),
                })
            },
            rescue: vec![],
        },
    }
}

fn synapse_admin_config() -> ClientConfig {
    let mut handlers = BTreeMap::new();
    handlers.insert(
        HandlerName::from_str("synapse-admin").unwrap(),
        ClientHandler {
            variant: ClientHandlerVariant::StaticDir(StaticDir {
                dir: "/app".parse().unwrap(),
                rebase: Rebase::default(),
                cache: Default::default(),
                post_processing: Default::default(),
            }),
            rules: default_rules(),
            priority: 100,
            refinable: Default::default(),
            profiles: None,
            // languages: None,
        },
    );

    let mount_points = btreemap! {
        MountPointName::from_str("default").unwrap() => ClientMount {
            handlers,
            profiles: Default::default(),
            refinable: Refinable::default(),
        }
    };

    ClientConfig {
        version: CURRENT_VERSION.clone(),
        revision: 1.into(),
        name: "synapse-admin".parse().unwrap(),
        mount_points,
        upstreams: Default::default(),
        refinable: Refinable::default(),
    }
}

pub fn generate(_docker: bool, _docker_compose: bool) -> anyhow::Result<()> {
    fs::create_dir_all("data")?;

    for dir in SKELETON.dirs() {
        fs::create_dir_all(dir.path())?;
        for file in dir.files() {
            fs::write(file.path(), file.contents())?;
        }
    }

    for file in SKELETON.files() {
        fs::write(file.path(), file.contents())?;
    }

    let synapse_config = synapse_server_config();
    let synapse_admin_config = synapse_admin_config();

    fs::write(
        "synapse-server/Exofile.yml",
        serde_yaml::to_string(&synapse_config).unwrap(),
    )?;
    fs::write(
        "synapse-admin/Exofile.yml",
        serde_yaml::to_string(&synapse_admin_config).unwrap(),
    )?;

    println!("Fill-in required data in files: \n");
    println!(" - exogress.env");
    println!(" - synapse.env\n\n");

    println!("Prepare synapse environment: \n");
    println!("  docker-compose run synapse generate\n\n");

    println!("Configure data/homeserver.yaml \n");
    println!("  - bind_addresses: ['127.0.0.1']\n\n");

    println!("Start: \n");
    println!("  docker-compose up --force-recreate --build\n\n");

    Ok(())
}
