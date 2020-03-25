use failure::Error;
use paw;

use buildkite_hooks::{config::Config, container::docker::*};

#[paw::main]

fn main(config: Config) -> Result<(), Error> {
    env_logger::init();

    Docker::new(&config.valid().command_id())
        .reap_containers()
        .map_err(|e| e.into())
}
