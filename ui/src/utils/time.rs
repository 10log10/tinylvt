use jiff::{Timestamp, Zoned, tz};

/// Helper function to localize a timestamp to the appropriate timezone
pub fn localize_timestamp(
    timestamp: Timestamp,
    site_timezone: Option<&str>,
) -> Zoned {
    match site_timezone {
        Some(tz_name) => {
            // Try to parse the site timezone using in_tz
            match timestamp.in_tz(tz_name) {
                Ok(zoned) => zoned,
                Err(_) => {
                    // Fall back to system timezone if site timezone is invalid
                    timestamp.to_zoned(tz::TimeZone::system())
                }
            }
        }
        None => {
            // Use system timezone when no site timezone is specified
            timestamp.to_zoned(tz::TimeZone::system())
        }
    }
}

/// Format a zoned timestamp for display in RFC 2822 format
pub fn format_zoned_timestamp(zoned: &Zoned) -> String {
    zoned.strftime("%a, %d %b %Y %H:%M:%S %Z").to_string()
}