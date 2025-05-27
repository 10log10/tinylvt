use jiff::Timestamp;

#[cfg(feature = "test-utils")]
use std::cell::RefCell;

#[cfg(feature = "test-utils")]
thread_local! {
    static MOCK_TIME: RefCell<Timestamp> = RefCell::new(Timestamp::now());
}

#[cfg(feature = "test-utils")]
pub fn set_mock_time(time: Timestamp) {
    MOCK_TIME.with(|t| *t.borrow_mut() = time);
}

/// Advance the mocked time using a Span, which must have hours or smaller units
/// to avoid panics due to the under-specified day duration.
#[cfg(feature = "test-utils")]
pub fn advance_mock_time(duration: jiff::Span) {
    MOCK_TIME.with(|t| {
        let new_ts = *t.borrow() + duration;
        *t.borrow_mut() = new_ts;
    });
}

#[cfg(feature = "test-utils")]
pub fn now() -> Timestamp {
    MOCK_TIME.with(|t| *t.borrow())
}

#[cfg(not(feature = "test-utils"))]
pub fn now() -> Timestamp {
    Timestamp::now()
}
