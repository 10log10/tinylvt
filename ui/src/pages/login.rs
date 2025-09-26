use payloads::responses;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;
use crate::components::{LoginForm, login_form::AuthMode};

#[function_component]
pub fn LoginPage() -> Html {
    let navigator = use_navigator().unwrap();
    let mode = use_state(|| AuthMode::Login);

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
                    show_dev_credentials={*mode == AuthMode::Login}
                />

                <div class="text-center">
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
                </div>
            </div>
        </div>
    }
}
