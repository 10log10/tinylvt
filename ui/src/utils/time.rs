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

/// Parse datetime-local input string (YYYY-MM-DDTHH:MM) to Timestamp.
/// If timezone is provided, use it; otherwise use system timezone.
pub fn parse_datetime_local(
    s: &str,
    timezone: Option<&str>,
) -> Result<Timestamp, String> {
    // datetime-local format: "2024-01-15T14:30"
    // Parse using jiff's civil datetime
    let civil_dt = jiff::civil::DateTime::strptime("%Y-%m-%dT%H:%M", s)
        .map_err(|e| format!("Failed to parse datetime: {}", e))?;

    // Convert to timestamp in specified or system timezone
    let tz = if let Some(tz_name) = timezone {
        tz::TimeZone::get(tz_name)
            .map_err(|e| format!("Invalid timezone '{}': {}", tz_name, e))?
    } else {
        tz::TimeZone::system()
    };

    civil_dt
        .to_zoned(tz)
        .map_err(|e| format!("Failed to convert to zoned datetime: {}", e))
        .map(|zdt| zdt.timestamp())
}
