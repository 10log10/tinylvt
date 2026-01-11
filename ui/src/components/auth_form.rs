use payloads::responses::UserProfile;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;
use crate::components::{LoginForm, login_form::AuthMode};
use crate::utils::is_dev_mode;

/// A higher-level authentication form component that handles switching
/// between login and signup modes. Use this when you want a complete
/// auth experience with mode toggling.
#[derive(Properties, PartialEq)]
pub struct AuthFormProps {
    /// Initial mode (defaults to Login)
    #[prop_or(AuthMode::Login)]
    pub initial_mode: AuthMode,
    /// Callback fired on successful authentication
    #[prop_or_default]
    pub on_success: Callback<UserProfile>,
    /// Whether to show the "Lost your password?" link (defaults to true)
    #[prop_or(true)]
    pub show_forgot_password: bool,
    /// Login mode title
    #[prop_or_else(|| "Sign in to continue".into())]
    pub login_title: AttrValue,
    /// Login mode description
    #[prop_or_else(|| "Please sign in to access this page".into())]
    pub login_description: AttrValue,
    /// Login mode submit button text
    #[prop_or_else(|| "Sign in".into())]
    pub login_submit_text: AttrValue,
    /// Signup mode title
    #[prop_or_else(|| "Create your account".into())]
    pub signup_title: AttrValue,
    /// Signup mode description
    #[prop_or_else(|| "Join TinyLVT to get started".into())]
    pub signup_description: AttrValue,
    /// Signup mode submit button text
    #[prop_or_else(|| "Create account".into())]
    pub signup_submit_text: AttrValue,
}

#[function_component]
pub fn AuthForm(props: &AuthFormProps) -> Html {
    let mode = use_state(|| props.initial_mode);

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
                props.login_title.clone(),
                props.login_description.clone(),
                props.login_submit_text.clone(),
                "Don't have an account?",
                "Create one",
            ),
            AuthMode::CreateAccount => (
                props.signup_title.clone(),
                props.signup_description.clone(),
                props.signup_submit_text.clone(),
                "Already have an account?",
                "Sign in",
            ),
        };

    html! {
        <div class="max-w-md w-full space-y-4">
            <LoginForm
                title={title}
                description={description}
                submit_text={submit_text}
                mode={*mode}
                on_success={props.on_success.clone()}
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
                if *mode == AuthMode::Login && props.show_forgot_password {
                    <p class="text-sm text-neutral-600 dark:text-neutral-400">
                        <Link<Route> to={Route::ForgotPassword} classes="text-neutral-900 dark:text-neutral-100 hover:text-neutral-700 dark:hover:text-neutral-300 font-medium underline">
                            {"Lost your password?"}
                        </Link<Route>>
                    </p>
                }
            </div>
        </div>
    }
}
