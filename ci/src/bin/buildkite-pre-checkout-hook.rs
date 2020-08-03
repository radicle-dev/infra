use std::{fs, process::Command};

use failure::Error;
use log::info;
use paw;

use buildkite_hooks::{cmd::CommandExt, config::Config};

#[paw::main]

fn main(cfg: Config) -> Result<(), Error> {
    env_logger::init();

    fs::create_dir_all(&cfg.checkout_path)?;

    own_checkout_path(&cfg)?;

    Ok(())
}

fn own_checkout_path(cfg: &Config) -> Result<(), Error> {
    info!("Adjusting checkout path ownership");

    Command::sudo()
        .args(&["chown", "-R", "buildkite-agent"])
        .arg(&cfg.checkout_path)
        .safe()?
        .succeed()?;
    Ok(())
}
