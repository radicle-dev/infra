use std::io;
use std::process::{Child, Command, ExitStatus};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use libc;
use signal_hook::{iterator::Signals, SIGINT, SIGQUIT, SIGTERM};

pub trait CommandExt {
    fn safe_status(&mut self, timeout: Duration) -> Result<ExitStatus, io::Error>;
}

impl CommandExt for Command {
    fn safe_status(&mut self, timeout: Duration) -> Result<ExitStatus, io::Error> {
        let signals = Signals::new(&[SIGINT, SIGQUIT, SIGTERM])?;

        let process: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));

        let start = Instant::now();

        let process2 = process.clone();
        let handler = thread::spawn(move || loop {
            // Check if we've received any signals
            for signal in signals.pending() {
                match signal {
                    SIGINT | SIGQUIT | SIGTERM => {
                        // If we have a child, TERM + KILL it
                        if let Some(ref mut child) = &mut *process2.lock().unwrap() {
                            shutdown(child);
                        }
                    }
                    _ => unreachable!(),
                }
            }

            if let Some(ref mut child) = &mut *process2.lock().unwrap() {
                // Return status if the child already exited
                if let Ok(Some(status)) = child.try_wait() {
                    return Ok(status);
                }

                // Shutdown if timed out
                let ran = Instant::now().saturating_duration_since(start);
                if ran >= timeout {
                    shutdown(child);
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("Command timed out after {}ms", ran.as_millis()),
                    ));
                }

                // Otherwise, loop
            }

            // ... but not too busy
            thread::sleep(Duration::from_millis(42));
        });

        self.spawn().and_then(|child| {
            *process.lock().unwrap() = Some(child);
            handler.join().expect("Command thread panicked")
        })
    }
}

fn shutdown(child: &mut Child) {
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    thread::sleep(Duration::from_millis(500));
    let _ = child.kill();
}
