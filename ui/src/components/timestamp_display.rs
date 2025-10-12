use jiff::{Timestamp, tz};
use yew::prelude::*;

use crate::utils::time::{format_zoned_timestamp, localize_timestamp};

/// A component for displaying timestamps with timezone awareness.
///
/// This component handles two distinct use cases:
///
/// 1. **User-local times** (auction start/end): When no `site_timezone` is
///    provided, displays the timestamp in the user's local timezone. This is
///    appropriate for events that are relative to "now" for the user.
///
/// 2. **Site-local times** (possession periods): When `site_timezone` is
///    provided, displays the timestamp in the site's timezone. If this differs
///    from the user's local timezone, adds a visual indicator (italics +
///    border) and a tooltip explaining the timezone difference. This is
///    appropriate for events that require physical coordination at a specific
///    location.
///
/// The visual indicator only appears when the site timezone differs from the
/// user's timezone, alerting users to interpret the time in a different context
/// than their local clock.
#[derive(Properties, PartialEq)]
pub struct TimestampDisplayProps {
    /// The timestamp to display
    pub timestamp: Timestamp,
    /// Optional site timezone (e.g., "America/New_York")
    /// If provided and different from user's timezone, adds visual indicator
    #[prop_or_default]
    pub site_timezone: Option<String>,
}

#[function_component]
pub fn TimestampDisplay(props: &TimestampDisplayProps) -> Html {
    // Get user's local timezone
    let user_tz = tz::TimeZone::system();
    let user_tz_name = user_tz.iana_name().unwrap_or("UTC");

    // Determine if we should show timezone indicator
    let (zoned, show_indicator) = match &props.site_timezone {
        Some(site_tz_name) => {
            let site_zoned =
                localize_timestamp(props.timestamp, Some(site_tz_name));
            let is_different = site_tz_name.as_str() != user_tz_name;
            (site_zoned, is_different)
        }
        None => {
            let user_zoned = localize_timestamp(props.timestamp, None);
            (user_zoned, false)
        }
    };

    let formatted = format_zoned_timestamp(&zoned);

    if show_indicator {
        let site_tz_name = props.site_timezone.as_deref().unwrap_or("UTC");
        let tooltip_text = format!(
            "Displayed in site timezone ({}) which differs from your local timezone ({})",
            site_tz_name, user_tz_name
        );

        html! {
            <span
                class="italic border border-neutral-400 dark:border-neutral-500 px-1 rounded cursor-help"
                title={tooltip_text}
            >
                {formatted}
            </span>
        }
    } else {
        html! {
            <span>{formatted}</span>
        }
    }
}
