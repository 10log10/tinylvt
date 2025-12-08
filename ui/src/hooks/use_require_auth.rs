use crate::components::{LoginForm, login_form::AuthMode};
use crate::utils::is_dev_mode;
use crate::{AuthState, Route, State};
use payloads::responses::UserProfile;
use yew::prelude::*;
use yew_router::prelude::*;
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
/// or a login form if the user is logged out.
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
            // Show login form
            html! {
                <div class="flex items-center justify-center min-h-[60vh]">
                    <div class="max-w-md w-full space-y-4">
                        <LoginForm
                            title="Sign in to continue"
                            description="Please sign in to access this page"
                            submit_text="Sign in"
                            mode={AuthMode::Login}
                            on_success={Callback::noop()}
                            show_dev_credentials={is_dev_mode()}
                        />
                        <div class="text-center">
                            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                                {"Don't have an account? "}
                                <Link<Route>
                                    to={Route::Login}
                                    classes="text-neutral-900 dark:text-neutral-100 hover:text-neutral-700 dark:hover:text-neutral-300 font-medium underline"
                                >
                                    {"Create one"}
                                </Link<Route>>
                            </p>
                        </div>
                    </div>
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
