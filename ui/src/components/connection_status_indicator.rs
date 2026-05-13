use yew::prelude::*;

use crate::hooks::ConnectionStatus;

/// Renders a small "Live updates unavailable — refresh" notice when the
/// subscription has failed to connect, and nothing otherwise. Pages that
/// own a subscribed-fetch hook (`use_subscribed_fetch`) pass that hook's
/// `connection_status` here.
#[derive(Properties, PartialEq)]
pub struct Props {
    pub status: ConnectionStatus,
}

#[function_component]
pub fn ConnectionStatusIndicator(props: &Props) -> Html {
    if matches!(props.status, ConnectionStatus::Failed) {
        html! {
            <div class="text-sm text-neutral-600 dark:text-neutral-400 mb-4">
                {"Live updates unavailable — refresh to see latest changes."}
            </div>
        }
    } else {
        html! {}
    }
}
