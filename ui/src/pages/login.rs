use payloads::{ClientError, requests};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use crate::{AuthState, Route, State};

#[function_component]
pub fn LoginPage() -> Html {
    let navigator = use_navigator().unwrap();
    let (_state, dispatch) = use_store::<State>();

    let username_ref = use_node_ref();
    let password_ref = use_node_ref();
    let error_message = use_state(|| None::<String>);
    let is_loading = use_state(|| false);

    let on_submit = {
        let username_ref = username_ref.clone();
        let password_ref = password_ref.clone();
        let error_message = error_message.clone();
        let is_loading = is_loading.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let username_input =
                username_ref.cast::<HtmlInputElement>().unwrap();
            let password_input =
                password_ref.cast::<HtmlInputElement>().unwrap();

            let username = username_input.value();
            let password = password_input.value();

            if username.is_empty() || password.is_empty() {
                error_message.set(Some(
                    "Please enter both username and password".to_string(),
                ));
                return;
            }

            let credentials = requests::LoginCredentials { username, password };
            let error_message = error_message.clone();
            let is_loading = is_loading.clone();
            let navigator = navigator.clone();
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
                                        AuthState::LoggedIn(profile);
                                });
                                navigator.push(&Route::Home);
                            }
                            Err(_) => {
                                error_message.set(Some("Login succeeded but failed to load profile".to_string()));
                            }
                        }
                    }
                    Err(ClientError::APIError(_, msg)) => {
                        dispatch.reduce_mut(|state| {
                            state.auth_state = AuthState::LoggedOut;
                        });
                        error_message.set(Some(msg));
                    }
                    Err(ClientError::Network(_)) => {
                        error_message.set(Some(
                            "Network error. Please check your connection."
                                .to_string(),
                        ));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    html! {
        <div class="flex items-center justify-center min-h-[60vh]">
            <div class="max-w-md w-full bg-white dark:bg-neutral-800 p-8 rounded-lg shadow-md">
                <div class="mb-8 text-center">
                    <h1 class="text-2xl font-bold text-neutral-900 dark:text-neutral-100 mb-2">
                        {"Sign in to TinyLVT"}
                    </h1>
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"Enter your credentials to continue"}
                    </p>
                </div>

                <form onsubmit={on_submit} class="space-y-6">
                    if let Some(error) = &*error_message {
                        <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                            <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
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
                            autocomplete="current-password"
                            required={true}
                            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm bg-white dark:bg-neutral-700 
                                   text-neutral-900 dark:text-neutral-100
                                   focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                   dark:focus:ring-neutral-400 dark:focus:border-neutral-400"
                            placeholder="Enter your password"
                        />
                    </div>

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
                            {"Signing in..."}
                        } else {
                            {"Sign in"}
                        }
                    </button>
                </form>

                <div class="mt-6 text-center">
                    <p class="text-sm text-neutral-600 dark:text-neutral-400">
                        {"Development credentials: alice / supersecret"}
                    </p>
                </div>
            </div>
        </div>
    }
}
