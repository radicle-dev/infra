use failure::Error;
use paw;

use buildkite_hooks::config::Config;
use buildkite_hooks::container::docker::*;

#[paw::main]
fn main(config: Config) -> Result<(), Error> {
    env_logger::init();

    Docker::new()
        .reap_containers(&config.valid().command_id())
        .map_err(|e| e.into())
}
