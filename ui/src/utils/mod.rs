pub mod time;

/// Returns true if the application is running in development mode.
/// Checks if BACKEND_URL contains "localhost".
pub fn is_dev_mode() -> bool {
    option_env!("BACKEND_URL")
        .map(|url| url.contains("localhost"))
        .unwrap_or(false)
}

/// Returns the input with its first character uppercased. Handles multi-byte
/// characters correctly.
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
