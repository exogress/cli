#[macro_use]
extern crate tracing;

pub mod git;

use include_dir::{include_dir, Dir};
use std::{env, fs, process};

use crate::git::Repo;
use clap::{crate_version, App, Arg, SubCommand};
use cloud_storage::Object;
use flate2::{Compression, GzBuilder};
use hex;
use reqwest;
use semver::Version;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs::File, io::Write, time::Duration};
use tempfile::TempDir;
use tokio::{
    self,
    process::Command,
    stream::StreamExt,
    time::{delay_for, timeout},
};
use tracing::Level;
use url::Url;

const DEB_ARCHS: [&str; 4] = ["amd64", "arm64", "armel", "armhf"];
const TEMPLATES_DIR: Dir = include_dir!("templates");

const HOMEBREW_FILE: &str = "exogress.rb";

async fn fetch_archive(url: &str) -> Vec<u8> {
    let resp = reqwest::get(url).await.unwrap();

    if !resp.status().is_success() {
        panic!("{} status code is {:?}", url, resp.status());
    }

    resp.bytes().await.unwrap().to_vec()
}

fn hash_archive(archive: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.input(archive);
    hex::encode(&hasher.result()[..])
}

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("no global subscriber has been set");

    let matches = App::new("Exogress Publisher")
        .version(crate_version!())
        .author("Exogress Team <team@exogress.com>")
        .about("Publish exogress binaries to package repositories")
        .arg(
            Arg::with_name("version")
                .help("version")
                .long("version")
                .takes_value(true)
                .required(true),
        )
        .subcommand(
            SubCommand::with_name("check_version")
                .about("Check versions match")
                .arg(
                    Arg::with_name("cargo_toml")
                        .help("cargo-toml-path")
                        .long("cargo-toml-path")
                        .takes_value(true)
                        .required(true)
                        .default_value("../Cargo.toml"),
                ),
        )
        .subcommand(
            SubCommand::with_name("docker")
                .about("Generate template for docker")
                .arg(
                    Arg::with_name("os_family")
                        .help("os-family")
                        .long("os-family")
                        .possible_values(&["debian-based"])
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("parent_image")
                        .help("parent docker image to use")
                        .long("parent")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("publish")
                .about("Generate and publish packages")
                .arg(
                    Arg::with_name("additional_message")
                        .long("message")
                        .help("message")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("github_token")
                        .help("github-token")
                        .long("github-token")
                        .env("GITHUB_TOKEN")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("gpg_key_id")
                        .help("gpg-key-id")
                        .long("gpg-key-id")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .get_matches();

    let version: Version = matches
        .value_of("version")
        .expect("version not set")
        .trim_start_matches('v')
        .parse()
        .expect("bad version");

    let version_string = version.to_string();

    if let Some(matches) = matches.subcommand_matches("docker") {
        let os_family = matches
            .value_of("os_family")
            .expect("os_family not set")
            .to_string();

        let parent_image = matches
            .value_of("parent_image")
            .expect("parent_image not set")
            .to_string();

        let template = match os_family.as_str() {
            "debian-based" => "Dockerfile.deb.mustache".to_string(),
            _ => panic!("Unknown os_family"),
        };

        let dockerfile_tpl = TEMPLATES_DIR
            .get_file(template)
            .unwrap()
            .contents_utf8()
            .unwrap();

        let template = mustache::compile_str(dockerfile_tpl).expect("Failed to compile");

        let mut data = HashMap::new();

        let deb_version = version_string.replace('-', "~");

        data.insert("DEB_VERSION", deb_version.as_str());
        data.insert("PARENT", parent_image.as_str());

        let mut bytes = vec![];

        template
            .render(&mut bytes, &data)
            .expect("Failed to render");

        let content = std::str::from_utf8(&bytes).unwrap();

        println!("{}", content);

        process::exit(0);
    }

    if let Some(matches) = matches.subcommand_matches("check_version") {
        let cargo_toml_path = matches
            .value_of("cargo_toml")
            .expect("cargo_toml not set")
            .to_string();

        eprintln!("Using cargo.toml at {}", cargo_toml_path);
        let content = fs::read(cargo_toml_path).unwrap();
        let parsed = toml::from_slice::<toml::Value>(content.as_ref()).unwrap();
        let pkg_version = parsed["package"]["version"]
            .as_str()
            .expect("No version in Cargo.toml")
            .to_string();
        if pkg_version != version_string {
            eprintln!("{} != {}", pkg_version, version_string);
            process::exit(1);
        } else {
            eprintln!("versions match");
        }

        println!("{}", version_string);

        process::exit(0);
    }

    if let Some(matches) = matches.subcommand_matches("publish") {
        let github_token = matches
            .value_of("github_token")
            .expect("github token not set");

        let additional_message = matches
            .value_of("additional_message")
            .expect("additional_message not set");

        let gpg_key_id = matches
            .value_of("gpg_key_id")
            .expect("gpg_key_id not set")
            .to_string();

        let macos_url = format!(
            "https://github.com/exogress/cli/archive/refs/tags/v{version}.tar.gz",
            version = version_string
        );

        // let repo_url = format!(
        //     "https://github.com/exogress/exogress/archive/{}.tar.gz",
        //     version
        // );

        let macos_archive = fetch_archive(&macos_url).await;

        let macos_hash = hash_archive(&macos_archive);

        info!("generate homebrew...");
        let homebrew_tpl = TEMPLATES_DIR
            .get_file("homebrew.mustache")
            .unwrap()
            .contents_utf8()
            .unwrap();

        let template = mustache::compile_str(homebrew_tpl).expect("Failed to compile");

        let mut data = HashMap::new();
        data.insert("MACOS_URL", macos_url.as_str());
        data.insert("VERSION", version_string.as_str());
        data.insert("MACOS_SHA256", macos_hash.as_str());

        let mut bytes = vec![];

        template
            .render(&mut bytes, &data)
            .expect("Failed to render");

        let content = std::str::from_utf8(&bytes).unwrap();

        let homebrew_repo = Repo::new(
            "https://github.com/exogress/homebrew-brew.git"
                .parse()
                .unwrap(),
        )
        .unwrap();

        homebrew_repo.add_file(HOMEBREW_FILE, content).unwrap();
        homebrew_repo
            .commit(format!("{}: {}", version, additional_message).as_str())
            .unwrap();

        info!("sync apt repo");
        const APT_BUCKET: &str = "exogress-apt";
        let bucket_dir = TempDir::new().unwrap();

        let mut all_objects = Box::pin(
            Object::list(APT_BUCKET)
                .await
                .expect("cannot retrieve apt packets"),
        );

        while let Some(objects_res) = all_objects.next().await {
            let objects = objects_res.unwrap();
            for object in &objects {
                let object_name = object.name.clone();
                let content = Object::download(APT_BUCKET, object_name.as_str())
                    .await
                    .unwrap();
                let mut p = bucket_dir.path().to_owned();
                p.push(object_name.as_str());
                let mut file = File::create(p).unwrap();
                file.write_all(&content).unwrap();
            }
        }

        for arch in &DEB_ARCHS {
            let deb_version = version_string.replace('-', ".");
            let url: Url = format!(
                "https://github.com/exogress/cli/releases/download/v{version}/exogress_{deb_version}_{arch}.deb",
                version = version_string,
                deb_version = deb_version,
                arch = arch
            ).parse().unwrap();
            let filename = url
                .path_segments()
                .unwrap()
                .rev()
                .next()
                .unwrap()
                .to_string();
            let content = fetch_archive(url.as_str()).await;
            let mut p = bucket_dir.path().to_owned();
            p.push(filename.clone());

            let mut file = File::create(p).unwrap();
            file.write_all(&content).unwrap();

            Object::create(
                APT_BUCKET,
                content,
                filename.as_str(),
                "application/octet-stream",
            )
            .await
            .expect("could not create deb file in apt bucket");
        }

        info!("path = {}", bucket_dir.path().to_str().unwrap());

        env::set_current_dir(bucket_dir.path().to_str().unwrap()).unwrap();

        let output_gen_packages = Command::new("apt-ftparchive")
            .arg("packages")
            .arg(".")
            .output()
            .await
            .unwrap();

        assert!(output_gen_packages.status.success());
        let packages_content = output_gen_packages.stdout;

        info!("write Packages...");
        let mut file = File::create("Packages").unwrap();
        file.write_all(packages_content.clone().as_ref()).unwrap();
        Object::create(
            APT_BUCKET,
            packages_content.clone(),
            "Packages",
            "text/plain",
        )
        .await
        .expect("could not create deb file in apt bucket");

        {
            info!("write Packages.gz...");
            let mut f = Vec::new();
            let mut gz = GzBuilder::new()
                .filename("Packages")
                .write(&mut f, Compression::default());

            gz.write_all(packages_content.as_ref()).unwrap();
            gz.finish().unwrap();

            let mut file = File::create("Packages.gz").unwrap();
            file.write_all(f.clone().as_ref()).unwrap();

            Object::create(APT_BUCKET, f, "Packages.gz", "application/gzip")
                .await
                .expect("could not create deb file in apt bucket");
        }

        let output_release = Command::new("apt-ftparchive")
            .arg("release")
            .arg(".")
            .output()
            .await
            .unwrap();
        assert!(output_gen_packages.status.success());

        info!("write Release...");
        let mut file = File::create("Release").unwrap();
        file.write_all(output_release.stdout.clone().as_ref())
            .unwrap();

        Object::create(APT_BUCKET, output_release.stdout, "Release", "text/plain")
            .await
            .expect("could not create deb file in apt bucket");

        let release_gpg = Command::new("gpg")
            .arg("--default-key")
            .arg(gpg_key_id.as_str())
            .arg("-abs")
            .arg("-o")
            .arg("-")
            .arg(bucket_dir.path().join("Release"))
            .output()
            .await
            .unwrap();
        info!(
            "STDOUT: {}",
            std::str::from_utf8(release_gpg.stdout.as_ref()).unwrap()
        );
        info!(
            "STDERR: {}",
            std::str::from_utf8(release_gpg.stderr.as_ref()).unwrap()
        );
        assert!(release_gpg.status.success());
        info!("write GPG...");
        let mut file = File::create("Release.gpg").unwrap();
        file.write_all(release_gpg.stdout.clone().as_ref()).unwrap();

        Object::create(
            APT_BUCKET,
            release_gpg.stdout,
            "Release.gpg",
            "application/octet-stream",
        )
        .await
        .expect("could not create deb file in apt bucket");

        let in_release = Command::new("gpg")
            .arg("--default-key")
            .arg(gpg_key_id.as_str())
            .arg("--clearsign")
            .arg("-o")
            .arg("-")
            .arg(bucket_dir.path().join("Release"))
            .output()
            .await
            .unwrap();
        assert!(in_release.status.success());

        info!("write InRelease...");
        let mut file = File::create("InRelease").unwrap();
        file.write_all(in_release.stdout.clone().as_ref()).unwrap();
        Object::create(APT_BUCKET, in_release.stdout, "InRelease", "text/plain")
            .await
            .expect("could not create deb file in apt bucket");

        homebrew_repo.push(github_token).unwrap();

        let try_download = async move {
            for arch in &DEB_ARCHS {
                let deb_version = version_string.replace('-', ".");
                let url: Url = format!(
                    "https://apt.exogress.com/exogress_{deb_version}_{arch}.deb",
                    deb_version = deb_version,
                    arch = arch
                )
                .parse()
                .unwrap();
                loop {
                    match reqwest::Client::new().head(url.clone()).send().await {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                info!("{} found", url);
                                break;
                            } else {
                                warn!("{} still not found: {}", url, resp.status());
                            }
                        }
                        Err(e) => {
                            warn!("{} error: {}", url, e);
                        }
                    }
                    delay_for(Duration::from_secs(5)).await;
                }
            }
        };

        if timeout(Duration::from_secs(2000), try_download)
            .await
            .is_err()
        {
            error!("Failed to wait for deb packages to appear");
            process::exit(-1);
        };
    }
}
