use std::io;

use paw;

use buildkite_hooks::config::Config;
use buildkite_hooks::container::docker::*;

#[paw::main]
fn main(config: Config) -> Result<(), io::Error> {
    let config = config.valid();
    Docker::new()
        .reap_containers(&config.command_id())
        .map(|_| ())
}
