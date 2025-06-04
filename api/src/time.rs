use jiff::Timestamp;
#[cfg(feature = "test-utils")]
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct TimeSource {
    #[cfg(feature = "test-utils")]
    time: Arc<Mutex<Timestamp>>,
}

impl TimeSource {
    #[allow(clippy::new_without_default)]
    #[cfg(not(feature = "test-utils"))]
    pub fn new() -> Self {
        Self {}
    }

    #[cfg(feature = "test-utils")]
    pub fn new(initial_time: Timestamp) -> Self {
        Self {
            time: Arc::new(Mutex::new(initial_time)),
        }
    }

    #[cfg(not(feature = "test-utils"))]
    pub fn now(&self) -> Timestamp {
        Timestamp::now()
    }

    #[cfg(feature = "test-utils")]
    pub fn now(&self) -> Timestamp {
        *self.time.lock().unwrap()
    }

    #[cfg(feature = "test-utils")]
    pub fn advance(&self, duration: jiff::Span) {
        *self.time.lock().unwrap() += duration;
    }

    #[cfg(feature = "test-utils")]
    pub fn set(&self, time: Timestamp) {
        *self.time.lock().unwrap() = time;
    }
}
