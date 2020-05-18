use std::{fs, process::Command};

use failure::Error;
use log::{debug, info};
use paw;

use buildkite_hooks::{cmd::CommandExt, config::Config};

#[paw::main]

fn main(cfg: Config) -> Result<(), Error> {
    env_logger::init();

    decrypt_repo_secrets(&cfg)?;

    fs::create_dir_all(&cfg.checkout_path)?;

    own_checkout_path(&cfg)
}

fn decrypt_repo_secrets(cfg: &Config) -> Result<(), Error> {
    if cfg.is_trusted_build() {
        info!("Decrypting secrets");

        let secrets_yaml = cfg.checkout_path.join(".buildkite/secrets.yaml");

        if secrets_yaml.exists() {
            Command::new("sops")
                .args(&[
                    "--output-type",
                    "dotenv",
                    "--output",
                    ".secrets",
                    "--decrypt",
                ])
                .arg(secrets_yaml)
                .safe()?
                .succeed()
                .map_err(|e| e.into())
        } else {
            debug!("No .buildkite/secrets.yaml in repository");

            Ok(())
        }
    } else {
        info!("Build secrets not available for unstrusted builds");

        Ok(())
    }
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
