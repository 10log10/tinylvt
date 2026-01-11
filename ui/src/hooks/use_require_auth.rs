use crate::components::AuthForm;
use crate::{AuthState, State};
use payloads::responses::UserProfile;
use yew::prelude::*;
use yewdux::use_store;

/// Hook that requires authentication. Returns the user profile if logged in,
/// or None if logged out or still checking auth status.
#[hook]
pub fn use_require_auth() -> Option<UserProfile> {
    let (state, _) = use_store::<State>();

    match &state.auth_state {
        AuthState::LoggedIn(profile) => Some(profile.clone()),
        AuthState::LoggedOut | AuthState::Unknown => None,
    }
}

/// Component that shows a spinner while auth is being checked,
/// or a login/signup form if the user is logged out.
#[function_component]
fn LoginFormFallback() -> Html {
    let (state, _) = use_store::<State>();

    match &state.auth_state {
        AuthState::Unknown => {
            // Show spinner while checking auth
            html! {
                <div class="text-center py-8">
                    <div class="inline-block animate-spin rounded-full h-8 w-8 border-2 border-neutral-900 dark:border-neutral-100 border-t-transparent dark:border-t-transparent"></div>
                </div>
            }
        }
        AuthState::LoggedOut => {
            // Show auth form with login/signup toggle
            html! {
                <div class="flex items-center justify-center min-h-[60vh]">
                    <AuthForm on_success={Callback::noop()} />
                </div>
            }
        }
        AuthState::LoggedIn(_) => {
            // Should not happen, but handle gracefully
            html! {}
        }
    }
}

/// Returns an inline login form for use when auth is required.
/// Shows a spinner while auth is being checked, then the login form if logged out.
pub fn login_form() -> Html {
    html! { <LoginFormFallback /> }
}
