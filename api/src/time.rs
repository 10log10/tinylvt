use jiff::Timestamp;
#[cfg(feature = "mock-time")]
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct TimeSource {
    #[cfg(feature = "mock-time")]
    time: Arc<Mutex<Timestamp>>,
}

impl TimeSource {
    #[allow(clippy::new_without_default)]
    #[cfg(not(feature = "mock-time"))]
    pub fn new() -> Self {
        Self {}
    }

    #[cfg(feature = "mock-time")]
    pub fn new(initial_time: Timestamp) -> Self {
        Self {
            time: Arc::new(Mutex::new(initial_time)),
        }
    }

    #[cfg(not(feature = "mock-time"))]
    pub fn now(&self) -> Timestamp {
        Timestamp::now()
    }

    #[cfg(feature = "mock-time")]
    pub fn now(&self) -> Timestamp {
        *self.time.lock().unwrap()
    }

    #[cfg(feature = "mock-time")]
    pub fn advance(&self, duration: jiff::Span) {
        *self.time.lock().unwrap() += duration;
    }

    #[cfg(feature = "mock-time")]
    pub fn set(&self, time: Timestamp) {
        *self.time.lock().unwrap() = time;
    }
}
