//! Shared Tailwind class strings for recurring controls, so buttons and
//! error banners stay visually consistent across sections.

use yew::prelude::*;

/// Solid high-contrast button for a section's main action.
pub const PRIMARY_BUTTON: &str = "px-4 py-2 text-sm font-medium \
    text-white bg-neutral-900 hover:bg-neutral-700 dark:bg-neutral-100 \
    dark:text-neutral-900 dark:hover:bg-neutral-300 rounded-md \
    transition-colors disabled:opacity-50 disabled:cursor-not-allowed";

/// Bordered neutral button for secondary actions.
pub const SECONDARY_BUTTON: &str = "px-4 py-2 text-sm font-medium \
    text-neutral-700 dark:text-neutral-300 bg-white dark:bg-neutral-800 \
    border border-neutral-300 dark:border-neutral-600 rounded-md \
    hover:bg-neutral-50 dark:hover:bg-neutral-700 transition-colors \
    disabled:opacity-50 disabled:cursor-not-allowed";

pub const ERROR_BANNER: &str = "p-4 rounded-md bg-red-50 \
    dark:bg-red-900/20 border border-red-200 dark:border-red-800";

pub const ERROR_TEXT: &str = "text-sm text-red-700 dark:text-red-400";

/// A contextual error banner card. No outer margin; callers add their
/// own spacing.
pub fn error_banner(message: &str) -> Html {
    html! {
        <div class={ERROR_BANNER}>
            <p class={ERROR_TEXT}>
                {message.to_string()}
            </p>
        </div>
    }
}
