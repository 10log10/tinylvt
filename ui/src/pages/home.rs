use crate::{AuthState, State};
use yew::prelude::*;
use yewdux::prelude::*;

use super::{LoggedInHomePage, LoggedOutHomePage};

#[function_component]
pub fn HomePage() -> Html {
    let (state, _) = use_store::<State>();

    match &state.auth_state {
        AuthState::LoggedIn(profile) => html! {
            <LoggedInHomePage profile={profile.clone()} />
        },
        AuthState::LoggedOut => html! {
            <LoggedOutHomePage />
        },
        AuthState::Unknown => html! {
            <div class="text-center space-y-4">
                <div class="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-neutral-900 dark:border-neutral-100"></div>
                <p class="text-neutral-600 dark:text-neutral-400">{"Checking authentication..."}</p>
            </div>
        },
    }
}
