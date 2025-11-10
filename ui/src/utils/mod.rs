pub mod time;

/// Returns true if the application is running in development mode.
/// Checks if BACKEND_URL contains "localhost".
pub fn is_dev_mode() -> bool {
    option_env!("BACKEND_URL")
        .map(|url| url.contains("localhost"))
        .unwrap_or(false)
}
