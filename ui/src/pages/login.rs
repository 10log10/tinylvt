use payloads::responses;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use crate::Route;
use crate::components::{LoginForm, login_form::AuthMode};
use crate::state::State;
use crate::utils::is_dev_mode;

#[function_component]
pub fn LoginPage() -> Html {
    let navigator = use_navigator().unwrap();
    let mode = use_state(|| AuthMode::Login);
    let (state, _) = use_store::<State>();

    // Redirect to home if already logged in
    {
        let navigator = navigator.clone();
        let is_authenticated = state.is_authenticated();

        use_effect_with(is_authenticated, move |is_auth| {
            if *is_auth {
                navigator.push(&Route::Home);
            }
        });
    }

    // Check for signup query parameter
    {
        let mode = mode.clone();

        use_effect_with((), move |_| {
            let window = web_sys::window().unwrap();
            let location = window.location();
            let search = location.search().unwrap_or_default();

            // Parse query string for signup parameter
            if search.contains("signup") {
                mode.set(AuthMode::CreateAccount);
            }
        });
    }

    let on_auth_success = {
        let navigator = navigator.clone();

        Callback::from(move |_profile: responses::UserProfile| {
            // Both login and account creation now navigate to home
            // since we auto-login users after successful account creation
            navigator.push(&Route::Home);
        })
    };

    let toggle_mode = {
        let mode = mode.clone();

        Callback::from(move |_: MouseEvent| {
            mode.set(match *mode {
                AuthMode::Login => AuthMode::CreateAccount,
                AuthMode::CreateAccount => AuthMode::Login,
            });
        })
    };

    let (title, description, submit_text, toggle_text, toggle_link_text) =
        match *mode {
            AuthMode::Login => (
                "Sign in to TinyLVT",
                "Enter your credentials to continue",
                "Sign in",
                "Don't have an account?",
                "Create one",
            ),
            AuthMode::CreateAccount => (
                "Create your account",
                "Join TinyLVT to get started",
                "Create account",
                "Already have an account?",
                "Sign in",
            ),
        };

    html! {
        <div class="flex items-center justify-center min-h-[60vh]">
            <div class="max-w-md w-full space-y-4">
                <LoginForm
                    title={title}
                    description={description}
                    submit_text={submit_text}
                    mode={*mode}
                    on_success={on_auth_success}
                    show_dev_credentials={*mode == AuthMode::Login && is_dev_mode()}
                />

                <div class="text-center space-y-2">
                    <p class="text-sm text-neutral-600 dark:text-neutral-400">
                        {toggle_text}
                        {" "}
                        <button
                            onclick={toggle_mode}
                            class="text-neutral-900 dark:text-neutral-100 hover:text-neutral-700 dark:hover:text-neutral-300 font-medium underline"
                        >
                            {toggle_link_text}
                        </button>
                    </p>
                    if *mode == AuthMode::Login {
                        <p class="text-sm text-neutral-600 dark:text-neutral-400">
                            <Link<Route> to={Route::ForgotPassword} classes="text-neutral-900 dark:text-neutral-100 hover:text-neutral-700 dark:hover:text-neutral-300 font-medium underline">
                                {"Lost your password?"}
                            </Link<Route>>
                        </p>
                    }
                </div>
            </div>
        </div>
    }
}
