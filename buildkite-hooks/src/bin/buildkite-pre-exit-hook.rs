use std::io;

use failure::Fail;
use paw;

use buildkite_hooks::cmd;
use buildkite_hooks::config::Config;
use buildkite_hooks::container::docker::*;

#[derive(Debug, Fail)]
enum Error {
    #[fail(display = "{}", 0)]
    Cmd(cmd::Error),

    #[fail(display = "{}", 0)]
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<cmd::Error> for Error {
    fn from(e: cmd::Error) -> Self {
        Self::Cmd(e)
    }
}

#[paw::main]
fn main(config: Config) -> Result<(), Error> {
    env_logger::init();

    Docker::new()
        .reap_containers(&config.valid().command_id())
        .map_err(|e| e.into())
}
