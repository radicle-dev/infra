use std::time::{Duration, Instant};

pub struct Timeout {
    timeout: Duration,
    started: Instant,
}

impl Timeout {
    const ZERO_DURATION: Duration = Duration::from_secs(0);

    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            started: Instant::now(),
        }
    }

    pub fn remaining(&self) -> Duration {
        Instant::now()
            .checked_duration_since(self.started)
            .and_then(|elapsed| self.timeout.checked_sub(elapsed))
            .unwrap_or(Self::ZERO_DURATION)
    }
}
