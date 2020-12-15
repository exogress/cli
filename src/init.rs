use exogress::config_core::{ClientConfig, DEFAULT_CONFIG_FILE};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub fn create_exofile(dir: impl AsRef<Path>) -> Result<(), anyhow::Error> {
    let file_path = dir.as_ref().join(DEFAULT_CONFIG_FILE);

    let cfg = ClientConfig::sample(None, None, None, None);

    let str = serde_yaml::to_vec(&cfg)?;

    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file_path)?;
    f.write_all(str.as_ref())?;

    info!("{} successfully created", DEFAULT_CONFIG_FILE);

    Ok(())
}
