use std::{
    env::args,
    io,
    process::{exit, Command},
    time::Duration,
};

use buildkite_hooks::cmd::CommandExt;

/// Test program for [CommandExt]

fn main() {
    let mut argv = args().skip(1);

    if let Some(cmd) = argv.next() {
        match Command::new(cmd)
            .args(argv)
            .safe()
            .unwrap()
            .timeout(Duration::from_secs(5))
            .status()
        {
            Ok(status) => {
                println!("Command exited with {:?}", status);

                exit(0)
            },
            Err(e) if e.kind() == io::ErrorKind::TimedOut => {
                println!("{}", e);

                exit(1)
            },
            Err(e) => {
                println!("Unexpected error: {:?}", e);

                exit(255)
            },
        }
    } else {
        println!("Usage: test_cmd CMD ARGS...")
    }
}
