use std::env::args;
use std::io;
use std::process::{exit, Command};
use std::time::Duration;

use buildkite_hooks::cmd::CommandExt;

/// Test program for [CommandExt]
fn main() {
    let mut argv = args().skip(1);
    if let Some(cmd) = argv.nth(0) {
        match Command::new(cmd)
            .args(argv)
            .safe_status(Duration::from_secs(5))
        {
            Ok(status) => {
                println!("Command exited with {:?}", status);
                exit(0)
            }
            Err(e) if e.kind() == io::ErrorKind::TimedOut => {
                println!("Command timed out!");
                exit(1)
            }
            Err(e) => {
                println!("Unexpected error: {:?}", e);
                exit(255)
            }
        }
    } else {
        println!("Usage: test_cmd CMD ARGS...")
    }
}
