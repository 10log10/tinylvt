use payloads::{requests, responses};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{AuthState, State};

#[derive(Clone, Copy, PartialEq)]
pub enum AuthMode {
    Login,
    CreateAccount,
}

#[derive(Properties, PartialEq)]
pub struct LoginFormProps {
    pub title: AttrValue,
    pub description: AttrValue,
    pub submit_text: AttrValue,
    pub mode: AuthMode,
    pub on_success: Callback<responses::UserProfile>,
    #[prop_or_default]
    pub show_dev_credentials: bool,
}

#[function_component]
pub fn LoginForm(props: &LoginFormProps) -> Html {
    let (_state, dispatch) = use_store::<State>();

    let email_ref = use_node_ref();
    let username_ref = use_node_ref();
    let password_ref = use_node_ref();
    let confirm_password_ref = use_node_ref();
    let error_message = use_state(|| None::<String>);
    let is_loading = use_state(|| false);

    // Shared login callback that handles the login API call and state management
    let perform_login = {
        let error_message = error_message.clone();
        let is_loading = is_loading.clone();
        let on_success = props.on_success.clone();
        let dispatch = dispatch.clone();

        Callback::from(move |credentials: requests::LoginCredentials| {
            let error_message = error_message.clone();
            let is_loading = is_loading.clone();
            let on_success = on_success.clone();
            let dispatch = dispatch.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error_message.set(None);

                let api_client = crate::get_api_client();
                match api_client.login(&credentials).await {
                    Ok(_) => {
                        // Fetch user profile after successful login
                        match api_client.user_profile().await {
                            Ok(profile) => {
                                dispatch.reduce_mut(|state| {
                                    state.auth_state =
                                        AuthState::LoggedIn(profile.clone());
                                });
                                on_success.emit(profile);
                            }
                            Err(_) => {
                                error_message.set(Some("Login succeeded but failed to load profile".to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        dispatch.reduce_mut(|state| {
                            state.auth_state = AuthState::LoggedOut;
                        });
                        error_message.set(Some(e.to_string()));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    let on_submit = {
        let email_ref = email_ref.clone();
        let username_ref = username_ref.clone();
        let password_ref = password_ref.clone();
        let confirm_password_ref = confirm_password_ref.clone();
        let error_message = error_message.clone();
        let is_loading = is_loading.clone();
        let mode = props.mode;
        let perform_login = perform_login.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let username_input =
                username_ref.cast::<HtmlInputElement>().unwrap();
            let password_input =
                password_ref.cast::<HtmlInputElement>().unwrap();

            let username = username_input.value();
            let password = password_input.value();

            // Basic validation
            if username.is_empty() || password.is_empty() {
                error_message.set(Some(
                    "Please enter both username and password".to_string(),
                ));
                return;
            }

            match mode {
                AuthMode::Login => {
                    // For login, just call the shared login function directly
                    let credentials =
                        requests::LoginCredentials { username, password };
                    perform_login.emit(credentials);
                }
                AuthMode::CreateAccount => {
                    let email_input =
                        email_ref.cast::<HtmlInputElement>().unwrap();
                    let confirm_password_input = confirm_password_ref
                        .cast::<HtmlInputElement>()
                        .unwrap();

                    let email = email_input.value();
                    let confirm_password = confirm_password_input.value();

                    // Additional validation for signup
                    if email.is_empty() {
                        error_message
                            .set(Some("Please enter your email".to_string()));
                        return;
                    }

                    if !email.contains('@') {
                        error_message.set(Some(
                            "Please enter a valid email address".to_string(),
                        ));
                        return;
                    }

                    if password != confirm_password {
                        error_message
                            .set(Some("Passwords do not match".to_string()));
                        return;
                    }

                    if password.len() < 6 {
                        error_message.set(Some(
                            "Password must be at least 6 characters"
                                .to_string(),
                        ));
                        return;
                    }

                    // For account creation, create account first then auto-login
                    let create_account_request = requests::CreateAccount {
                        email,
                        username: username.clone(),
                        password: password.clone(),
                    };

                    let error_message = error_message.clone();
                    let is_loading = is_loading.clone();
                    let perform_login = perform_login.clone();

                    yew::platform::spawn_local(async move {
                        is_loading.set(true);
                        error_message.set(None);

                        let api_client = crate::get_api_client();
                        match api_client
                            .create_account(&create_account_request)
                            .await
                        {
                            Ok(_) => {
                                // Account created successfully, now log them in automatically using shared logic
                                let login_credentials =
                                    requests::LoginCredentials {
                                        username: create_account_request
                                            .username,
                                        password: create_account_request
                                            .password,
                                    };
                                perform_login.emit(login_credentials);
                            }
                            Err(e) => {
                                error_message.set(Some(e.to_string()));
                                is_loading.set(false);
                            }
                        }
                    });
                }
            }
        })
    };

    html! {
        <div class="max-w-md w-full bg-white dark:bg-neutral-800 p-8 rounded-lg shadow-md">
            <div class="mb-8 text-center">
                <h1 class="text-2xl font-bold text-neutral-900 dark:text-neutral-100 mb-2">
                    {&props.title}
                </h1>
                <p class="text-neutral-600 dark:text-neutral-400">
                    {&props.description}
                </p>
            </div>

            <form onsubmit={on_submit} class="space-y-6">
                if let Some(error) = &*error_message {
                    <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                        <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
                    </div>
                }

                if props.mode == AuthMode::CreateAccount {
                    <div>
                        <label for="email" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                            {"Email"}
                        </label>
                        <input
                            ref={email_ref}
                            type="email"
                            id="email"
                            name="email"
                            autocomplete="email"
                            required={true}
                            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm bg-white dark:bg-neutral-700
                                   text-neutral-900 dark:text-neutral-100
                                   focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                   dark:focus:ring-neutral-400 dark:focus:border-neutral-400"
                            placeholder="Enter your email"
                        />
                    </div>
                }

                <div>
                    <label for="username" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                        {"Username"}
                    </label>
                    <input
                        ref={username_ref}
                        type="text"
                        id="username"
                        name="username"
                        autocomplete="username"
                        required={true}
                        class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                               rounded-md shadow-sm bg-white dark:bg-neutral-700
                               text-neutral-900 dark:text-neutral-100
                               focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                               dark:focus:ring-neutral-400 dark:focus:border-neutral-400"
                        placeholder="Enter your username"
                    />
                </div>

                <div>
                    <label for="password" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                        {"Password"}
                    </label>
                    <input
                        ref={password_ref}
                        type="password"
                        id="password"
                        name="password"
                        autocomplete={if props.mode == AuthMode::CreateAccount { "new-password" } else { "current-password" }}
                        required={true}
                        class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                               rounded-md shadow-sm bg-white dark:bg-neutral-700
                               text-neutral-900 dark:text-neutral-100
                               focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                               dark:focus:ring-neutral-400 dark:focus:border-neutral-400"
                        placeholder={if props.mode == AuthMode::CreateAccount { "Choose a password" } else { "Enter your password" }}
                    />
                </div>

                if props.mode == AuthMode::CreateAccount {
                    <div>
                        <label for="confirm-password" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                            {"Confirm Password"}
                        </label>
                        <input
                            ref={confirm_password_ref}
                            type="password"
                            id="confirm-password"
                            name="confirm-password"
                            autocomplete="new-password"
                            required={true}
                            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm bg-white dark:bg-neutral-700
                                   text-neutral-900 dark:text-neutral-100
                                   focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                   dark:focus:ring-neutral-400 dark:focus:border-neutral-400"
                            placeholder="Confirm your password"
                        />
                    </div>
                }

                <button
                    type="submit"
                    disabled={*is_loading}
                    class="w-full flex justify-center py-2 px-4 border border-transparent
                           rounded-md shadow-sm text-sm font-medium text-white
                           bg-neutral-900 hover:bg-neutral-800
                           dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200
                           focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                           disabled:opacity-50 disabled:cursor-not-allowed
                           transition-colors duration-200"
                >
                    if *is_loading {
                        {match props.mode {
                            AuthMode::Login => "Signing in...",
                            AuthMode::CreateAccount => "Creating account...",
                        }}
                    } else {
                        {&props.submit_text}
                    }
                </button>
            </form>

            if props.show_dev_credentials {
                <div class="mt-6 text-center">
                    <p class="text-sm text-neutral-600 dark:text-neutral-400">
                        {"Development credentials: alice / supersecret"}
                    </p>
                </div>
            }
        </div>
    }
}
