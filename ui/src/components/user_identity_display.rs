use payloads::responses::UserIdentity;
use yew::prelude::*;

/// Renders a user's display name (if set) or username as Html.
///
/// If display_name is set, shows it with a tooltip containing the username.
/// This allows for later enhancement to include hyperlinks to user profiles.
pub fn render_user_name(identity: &UserIdentity) -> Html {
    match &identity.display_name {
        Some(display_name) => {
            html! {
                <span title={format!("@{}", identity.username)}>
                    {display_name}
                </span>
            }
        }
        None => {
            html! { {&identity.username} }
        }
    }
}

/// Renders a circular avatar with the first character of the user's
/// display name or username.
///
/// Takes optional CSS classes for the avatar container and text.
/// If None, uses default neutral styling.
pub fn render_user_avatar(
    identity: &UserIdentity,
    container_classes: Option<String>,
    text_classes: Option<String>,
) -> Html {
    let name = identity.display_name.as_ref().unwrap_or(&identity.username);

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
        <div class={container_classes} title={format!("@{}", identity.username)}>
            <span class={text_classes}>
                {initial}
            </span>
        </div>
    }
}
