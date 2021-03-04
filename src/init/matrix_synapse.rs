use crate::init::from_skeleton;
use clap::{App, ArgMatches};
use include_dir::{include_dir, Dir};
use std::fs;

const SUBCOMMAND: &str = "matrix-synapse-docker";

pub fn init_subcommand() -> App<'static, 'static> {
    App::new(SUBCOMMAND).about("Initialize matrix-synapse docker app with Exofile.yml")
}

static SKELETON: Dir = include_dir!("platforms_templates/synapse");

pub fn generate() -> anyhow::Result<()> {
    fs::create_dir_all("data")?;

    from_skeleton(&SKELETON, Default::default())?;

    println!("Fill-in required data in files: \n");
    println!(" - exogress.env");
    println!(" - synapse.env\n\n");

    println!("Prepare synapse environment: \n");
    println!("  docker-compose run synapse generate\n\n");

    println!("Configure data/homeserver.yaml \n");
    println!("  - bind_addresses: ['127.0.0.1']\n\n");

    println!("Start: \n");
    println!("  docker-compose up\n\n");

    Ok(())
}

pub fn handle_subcommand(args: &ArgMatches) {
    if args.subcommand_matches(SUBCOMMAND).is_some() {
        generate().expect("could not generate");
        println!("Configuration generated");
        std::process::exit(0);
    }
}
