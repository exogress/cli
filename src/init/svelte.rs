use crate::init::from_skeleton;
use clap::{App, ArgMatches};
use include_dir::{include_dir, Dir};

const SUBCOMMAND: &str = "svelte";

pub fn init_subcommand() -> App<'static, 'static> {
    App::new(SUBCOMMAND).about("Initialize Exofile.yml for Svelte")
}

static SKELETON: Dir = include_dir!("platforms_templates/svelte");

pub fn generate() -> anyhow::Result<()> {
    from_skeleton(&SKELETON, Default::default())
}

pub fn handle_subcommand(args: &ArgMatches) {
    if args.subcommand_matches(SUBCOMMAND).is_some() {
        generate().expect("could not generate");
        println!("Configuration generated");
        std::process::exit(0);
    }
}
