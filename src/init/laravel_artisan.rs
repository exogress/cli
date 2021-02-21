use crate::init::from_skeleton;
use clap::{App, ArgMatches};
use include_dir::{include_dir, Dir};

const SUBCOMMAND: &str = "laravel-artisan";

pub fn init_subcommand() -> App<'static, 'static> {
    App::new(SUBCOMMAND).about("Initialize Exofile.yml for Laravel with Artisan server")
}

static SKELETON: Dir = include_dir!("platforms_templates/laravel-artisan");

pub fn generate() -> anyhow::Result<()> {
    from_skeleton(&SKELETON)
}

pub fn handle_subcommand(args: &ArgMatches) {
    if args.subcommand_matches(SUBCOMMAND).is_some() {
        generate().expect("could not generate");
        println!("Configuration generated");
        std::process::exit(0);
    }
}
