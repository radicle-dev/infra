use std::fs;
use std::io;
use std::process::Command;

use buildkite_hooks::cmd;
use buildkite_hooks::cmd::CommandExt;
use buildkite_hooks::env;

#[derive(Debug)]
enum Error {
    Var(env::VarError),
    Io(io::Error),
    Cmd(cmd::Error),
    Sig(cmd::SignalsError),
}

impl From<env::VarError> for Error {
    fn from(e: env::VarError) -> Self {
        Self::Var(e)
    }
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

impl From<cmd::SignalsError> for Error {
    fn from(e: cmd::SignalsError) -> Self {
        Self::Sig(e)
    }
}

fn main() -> Result<(), Error> {
    env_logger::init();

    let checkout_path =
        env::var_os("BUILDKITE_BUILD_CHECKOUT_PATH").ok_or(env::VarError::NotPresent)?;

    fs::create_dir_all(&checkout_path)?;

    Command::new("sudo")
        .args(&["chown", "-R", "buildkite-agent"])
        .arg(checkout_path)
        .safe()?
        .succeed()
        .map_err(|e| e.into())
}
