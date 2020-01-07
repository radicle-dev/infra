use std::fs;
use std::io;
use std::process::Command;
use std::time::Duration;

use buildkite_hooks::cmd::CommandExt;
use buildkite_hooks::env;

#[derive(Debug)]
enum Error {
    Var(env::VarError),
    Io(io::Error),
    Killed,
}

impl From<env::VarError> for Error {
    fn from(e: env::VarError) -> Self {
        Error::Var(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

fn main() -> Result<(), Error> {
    let checkout_path =
        env::var_os("BUILDKITE_BUILD_CHECKOUT_PATH").ok_or(env::VarError::NotPresent)?;

    fs::create_dir_all(&checkout_path)?;

    let status = Command::new("sudo")
        .args(&["chown", "-R", "buildkite-agent"])
        .arg(checkout_path)
        .safe_status(Duration::from_secs(1))
        .map_err(Error::Io)?;

    if status.success() {
        Ok(())
    } else {
        status.code().map_or(Err(Error::Killed), |code| {
            Err(Error::Io(io::Error::from_raw_os_error(code)))
        })
    }
}
