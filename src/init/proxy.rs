use crate::init::from_skeleton;
use clap::{App, Arg, ArgMatches};
use include_dir::{include_dir, Dir};
use serde_json::json;

const SUBCOMMAND: &str = "proxy";

pub fn init_subcommand() -> App<'static, 'static> {
    App::new(SUBCOMMAND)
        .arg(
            Arg::with_name("port")
                .long("port")
                .short("p")
                .value_name("PORT")
                .help("Port to proxy to")
                .required(true)
                .takes_value(true),
        )
        .about("Initialize Exofile.yml for simple proxying")
}

static SKELETON: Dir = include_dir!("platforms_templates/proxy");

pub fn generate(port: u16) -> anyhow::Result<()> {
    from_skeleton(&SKELETON, json!({"port": port.to_string()}))
}

pub fn handle_subcommand(args: &ArgMatches) {
    if let Some(app) = args.subcommand_matches(SUBCOMMAND) {
        match app.value_of("port").unwrap().parse::<u16>() {
            Ok(port) => {
                generate(port).expect("could not generate");
            }
            Err(_) => {
                println!("Bad port provided");
                std::process::exit(0);
            }
        }
        println!("Configuration generated");
        std::process::exit(0);
    }
}
