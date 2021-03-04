use anyhow::anyhow;
use clap::{App, ArgMatches};
use handlebars::Handlebars;
use include_dir::{Dir, File};
use std::fs;

pub mod laravel_artisan;
pub mod matrix_synapse;
pub mod proxy;
pub mod rails;
pub mod svelte;

fn render(file: &File, values: &serde_json::Value) -> anyhow::Result<()> {
    if let Some(filename) = file.path().to_str().unwrap().strip_suffix(".handlebars") {
        let reg = Handlebars::new();

        let content = reg.render_template(
            file.contents_utf8()
                .ok_or_else(|| anyhow!("bad template"))?,
            values,
        )?;

        fs::write(filename, content)?;
    } else {
        let content = file
            .contents_utf8()
            .ok_or_else(|| anyhow!("bad template"))?
            .to_string();

        fs::write(file.path(), content)?;
    };

    Ok(())
}

pub fn from_skeleton(skeleton: &Dir, values: serde_json::Value) -> anyhow::Result<()> {
    for dir in skeleton.dirs() {
        fs::create_dir_all(dir.path())?;
        for file in dir.files() {
            render(file, &values)?;
        }
    }

    for file in skeleton.files() {
        render(file, &values)?;
    }

    Ok(())
}

pub fn init_app() -> App<'static, 'static> {
    App::new("init")
        .subcommand(matrix_synapse::init_subcommand())
        .subcommand(laravel_artisan::init_subcommand())
        .subcommand(rails::init_subcommand())
        .subcommand(svelte::init_subcommand())
        .subcommand(proxy::init_subcommand())
        .about("Initialize directory with exogress configuration")
}

pub fn handle_subcommand(args: &ArgMatches) {
    matrix_synapse::handle_subcommand(args);
    laravel_artisan::handle_subcommand(args);
    rails::handle_subcommand(args);
    svelte::handle_subcommand(args);
    proxy::handle_subcommand(args);
    match args.subcommand_name() {
        Some(subcommand_name) => {
            println!("Platform {} is not supported", subcommand_name);
        }
        None => {
            println!("Please, provide a valid platform name");
        }
    }
}
