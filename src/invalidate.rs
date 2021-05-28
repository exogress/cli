use crate::{add_authentication_args, extract_authentication_args, Authentication};
use anyhow::anyhow;
use clap::{App, Arg, ArgMatches};
use exogress_common::{api::SingleInvalidationRequest, client_core::api::ApiClient};
use tokio::runtime::Runtime;

pub fn invalidations_args<'a>() -> clap::App<'a, 'a> {
    add_authentication_args(
        App::new("invalidate")
            .about("invalidate cache records")
            .arg(
                Arg::with_name("invalidations")
                    .help("The list of invalidations in the format \"<invalidation_name>/<handler_name>/<mount_point_name>[/<config_name>]\". \
                    config_name should not be provided if invalidation relates to Project Config.")
                    .last(true)
                    .multiple(true),
            ),
    )
}

fn parse_invalidation_params(
    items: impl Iterator<Item = impl AsRef<str>>,
) -> impl Iterator<Item = anyhow::Result<SingleInvalidationRequest>> {
    items.map(|invalidation| {
        let invalidation_str = invalidation.as_ref().to_string();

        (|| {
            let mut item = invalidation_str.split("/");

            let invalidation_name = item
                .next()
                .ok_or_else(|| anyhow!("no invalidation name as a first item"))?
                .parse()?;
            let handler_name = item
                .next()
                .ok_or_else(|| anyhow!("no handler name as a second item"))?
                .parse()?;
            let mount_point_name = item
                .next()
                .ok_or_else(|| anyhow!("no mount point name as a third item"))?
                .parse()?;
            let config_name = item.next().map(|c| c.parse()).transpose()?;

            Ok(SingleInvalidationRequest {
                invalidation_name,
                mount_point_name,
                handler_name,
                config_name,
            })
        })()
        .map_err(|e: anyhow::Error| e.context(invalidation_str))
    })
}

pub fn handle_subcommand(args: &ArgMatches) {
    let rt = Runtime::new().unwrap();

    let Authentication {
        access_key_id,
        secret_access_key,
        account,
        project,
        api_endpoint,
        ..
    } = extract_authentication_args(args).expect("bad authentication data provided");

    let api = ApiClient::new(
        &project,
        &account,
        &access_key_id,
        &secret_access_key,
        &api_endpoint,
    )
    .unwrap();

    let invalidations = args.values_of("invalidations").expect("bad invalidations");

    let parsed_invalidations: Result<Vec<SingleInvalidationRequest>, _> =
        parse_invalidation_params(invalidations).collect::<Result<_, _>>();

    match parsed_invalidations {
        Err(e) => {
            println!("Bad invalidation: {}", e);
            std::process::exit(-1);
        }
        Ok(invalidations) => {
            println!("Sending invalidation request...");

            let spinner = indicatif::ProgressBar::new_spinner();

            spinner.enable_steady_tick(100);

            let res = rt.block_on(async move { api.invalidate(&invalidations).await });

            spinner.finish();

            match res {
                Ok(_) => {
                    println!("Succeeded");
                }
                Err(e) => {
                    println!("{:?}", e);
                    std::process::exit(-1);
                }
            }
        }
    }

    std::process::exit(0);
}
