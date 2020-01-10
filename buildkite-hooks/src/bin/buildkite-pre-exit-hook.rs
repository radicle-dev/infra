use std::io;

use paw;

use buildkite_hooks::config::Config;
use buildkite_hooks::container::docker::*;

#[paw::main]
fn main(config: Config) -> Result<(), io::Error> {
    env_logger::init();

    Docker::new()
        .reap_containers(&config.valid().command_id())
        .map(|_| ())
}
