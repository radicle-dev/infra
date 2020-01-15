use std::{
    io,
    process::{Child, Command, ExitStatus},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use failure::Fail;
use libc;
use signal_hook::{iterator::Signals, SIGINT, SIGQUIT, SIGTERM};

#[derive(Debug, Fail)]

pub enum Error {
    #[fail(display = "Child process killed by a signal, command was: {}", 0)]
    ChildKilled(CommandLine),

    #[fail(display = "Non-zero exit status {} for command {}", 1, 0)]
    NonZeroExitStatus(CommandLine, ExitStatus),

    #[fail(display = "{}", 0)]
    Signals(SignalsError),

    #[fail(display = "IO error while running {}: {}", 0, 1)]
    Io(CommandLine, io::Error),
}

impl From<SignalsError> for Error {
    fn from(e: SignalsError) -> Self { Self::Signals(e) }
}

pub type CommandLine = String;

pub fn command_line(cmd: &Command) -> CommandLine { format!("{:?}", cmd) }

pub struct Safe<'a> {
    command: &'a mut Command,
    signals: Signals,
    pub deadline: Option<Instant>,
}

impl<'a> Safe<'a> {
    pub fn timeout(mut self, after: Duration) -> Self {
        self.deadline = Instant::now().checked_add(after);

        self
    }

    pub fn status(&mut self) -> Result<ExitStatus, io::Error> {
        let process: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));

        let debug = command_line(&self.command);

        let signals = self.signals.clone();

        let deadline = self.deadline;

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
                    },
                    _ => unreachable!(),
                }
            }

            if let Some(ref mut child) = &mut *process2.lock().unwrap() {
                // Return status if the child already exited
                if let Ok(Some(status)) = child.try_wait() {
                    return Ok(status);
                }

                // Shutdown if timed out
                if let Some(deadline) = deadline {
                    if Instant::now() > deadline {
                        shutdown(child);

                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            format!(
                                "Deadline exceeded after {}ms for command: {:?}",
                                deadline.elapsed().as_millis(),
                                debug,
                            ),
                        ));
                    }
                }

                // Otherwise, loop
            }

            // ... but not too busy
            thread::sleep(Duration::from_millis(42));
        });

        self.command.spawn().and_then(|child| {
            *process.lock().unwrap() = Some(child);

            handler.join().expect("Command thread panicked")
        })
    }

    /// Run and wait for the command, and return unit if it terminates with a
    /// zero exit status, or an [`Error`] otherwise.

    pub fn succeed(&mut self) -> Result<(), Error> {
        self.status()
            .map_err(|e| Error::Io(command_line(self.command), e))
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    status.code().map_or(
                        Err(Error::ChildKilled(command_line(self.command))),
                        |code| {
                            Err(Error::Io(
                                command_line(self.command),
                                io::Error::from_raw_os_error(code),
                            ))
                        },
                    )
                }
            })
    }
}

#[derive(Debug, Fail)]
#[fail(display = "Failed to install signal handlers: {}", 0)]

pub struct SignalsError(#[fail(cause)] io::Error);

pub trait CommandExt {
    fn sudo() -> Command;

    fn safe(&'_ mut self) -> Result<Safe<'_>, SignalsError>;
}

impl CommandExt for Command {
    fn sudo() -> Command { Command::new("sudo") }

    fn safe(&'_ mut self) -> Result<Safe<'_>, SignalsError> {
        let signals = Signals::new(&[SIGINT, SIGQUIT, SIGTERM]).map_err(SignalsError)?;

        Ok(Safe {
            command: self,
            signals,
            deadline: None,
        })
    }
}

fn shutdown(child: &mut Child) {
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };

    thread::sleep(Duration::from_millis(500));

    let _ = child.kill();
}
