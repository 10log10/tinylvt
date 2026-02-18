use payloads::responses::{UserIdentity, UserProfile};
use yew::prelude::*;

/// Trait for types that have username and display_name fields.
pub trait HasUserName {
    fn username(&self) -> &str;
    fn display_name(&self) -> Option<&str>;
}

impl HasUserName for UserIdentity {
    fn username(&self) -> &str {
        &self.username
    }
    fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
}

impl HasUserName for UserProfile {
    fn username(&self) -> &str {
        &self.username
    }
    fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
}

/// Renders a user's display name (if set) or username as Html.
///
/// If display_name is set, shows it with a tooltip containing the username.
/// This allows for later enhancement to include hyperlinks to user profiles.
pub fn render_user_name<T: HasUserName>(user: &T) -> Html {
    match user.display_name() {
        Some(display_name) => {
            html! {
                <span title={format!("@{}", user.username())}>
                    {display_name}
                </span>
            }
        }
        None => {
            html! { {format!("@{}", user.username())} }
        }
    }
}

/// Returns the user's preferred display name (display_name if set, otherwise
/// @username). Use this for plain text contexts like welcome messages.
pub fn format_user_name<T: HasUserName>(user: &T) -> String {
    match user.display_name() {
        Some(display_name) => display_name.to_string(),
        None => format!("@{}", user.username()),
    }
}

/// Renders a user's identity unambiguously as a string, showing both display
/// name and username. Format: "Display Name (@username)" or "@username" if no
/// display name is set.
///
/// Use this for contexts where disambiguation is important, such as member
/// selection dropdowns for transfers.
pub fn format_user_name_unambiguous<T: HasUserName>(user: &T) -> String {
    match user.display_name() {
        Some(display_name) => {
            format!("{} (@{})", display_name, user.username())
        }
        None => format!("@{}", user.username()),
    }
}

/// Renders a circular avatar with the first character of the user's
/// display name or username.
///
/// Takes optional CSS classes for the avatar container and text.
/// If None, uses default neutral styling.
pub fn render_user_avatar<T: HasUserName>(
    user: &T,
    container_classes: Option<String>,
    text_classes: Option<String>,
) -> Html {
    let name = user.display_name().unwrap_or(user.username());

    let initial = name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();

    let default_container =
        "w-8 h-8 bg-neutral-200 dark:bg-neutral-600 rounded-full flex \
         items-center justify-center"
            .to_string();
    let default_text =
        "text-sm font-medium text-neutral-600 dark:text-neutral-300"
            .to_string();

    let container_classes = container_classes.unwrap_or(default_container);
    let text_classes = text_classes.unwrap_or(default_text);

    html! {
        <div class={container_classes} title={format!("@{}", user.username())}>
            <span class={text_classes}>
                {initial}
            </span>
        </div>
    }
}
