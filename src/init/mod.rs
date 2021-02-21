use clap::{App, ArgMatches};
use include_dir::Dir;
use std::fs;

pub mod laravel_artisan;
pub mod matrix_synapse;
pub mod rails;
pub mod svelte;

pub fn from_skeleton(skeleton: &Dir) -> anyhow::Result<()> {
    for dir in skeleton.dirs() {
        fs::create_dir_all(dir.path())?;
        for file in dir.files() {
            fs::write(file.path(), file.contents())?;
        }
    }

    for file in skeleton.files() {
        fs::write(file.path(), file.contents())?;
    }

    Ok(())
}

pub fn init_app() -> App<'static, 'static> {
    App::new("init")
        .subcommand(matrix_synapse::init_subcommand())
        .subcommand(laravel_artisan::init_subcommand())
        .subcommand(rails::init_subcommand())
        .subcommand(svelte::init_subcommand())
        .about("Initialize directory with exogress configuration")
}

pub fn handle_subcommand(args: &ArgMatches) {
    matrix_synapse::handle_subcommand(args);
    laravel_artisan::handle_subcommand(args);
    rails::handle_subcommand(args);
    svelte::handle_subcommand(args);
    match args.subcommand_name() {
        Some(subcommand_name) => {
            println!("Platform {} is not supported", subcommand_name);
        }
        None => {
            println!("Please, provide a valid platform name");
        }
    }
}
