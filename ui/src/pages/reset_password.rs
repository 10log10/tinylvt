use crate::{Route, get_api_client};
use payloads::requests;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component]
pub fn ResetPasswordPage() -> Html {
    let password = use_state(String::new);
    let confirm_password = use_state(String::new);
    let error = use_state(|| None::<String>);
    let success = use_state(|| false);
    let loading = use_state(|| false);
    let token = use_state(String::new);
    let navigator = use_navigator().unwrap();

    // Extract token from query string on mount
    {
        let token = token.clone();
        let error = error.clone();

        use_effect_with((), move |_| {
            let window = web_sys::window().unwrap();
            let location = window.location();
            let search = location.search().unwrap_or_default();

            // Parse query string manually (format: "?token=xxx")
            let token_value = if search.starts_with("?token=") {
                search.trim_start_matches("?token=").to_string()
            } else {
                String::new()
            };

            if token_value.is_empty() {
                error.set(Some("Invalid or missing reset token".to_string()));
            }

            token.set(token_value);

            || ()
        });
    }

    let onsubmit = {
        let password = password.clone();
        let confirm_password = confirm_password.clone();
        let error = error.clone();
        let success = success.clone();
        let loading = loading.clone();
        let token = token.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let password = password.clone();
            let confirm_password = confirm_password.clone();
            let error = error.clone();
            let success = success.clone();
            let loading = loading.clone();
            let token = token.clone();

            // Validate passwords match
            if *password != *confirm_password {
                error.set(Some("Passwords do not match".to_string()));
                return;
            }

            // Validate password length
            if password.len() < 8 {
                error.set(Some(
                    "Password must be at least 8 characters long".to_string(),
                ));
                return;
            }

            loading.set(true);
            error.set(None);

            let password_value = (*password).clone();
            let token_value = (*token).clone();

            wasm_bindgen_futures::spawn_local(async move {
                let client = get_api_client();
                let request = requests::ResetPassword {
                    token: token_value,
                    password: password_value,
                };

                match client.reset_password(&request).await {
                    Ok(_) => {
                        success.set(true);
                        loading.set(false);
                    }
                    Err(e) => {
                        error.set(Some(format!("Error: {}", e)));
                        loading.set(false);
                    }
                }
            });
        })
    };

    let on_password_input = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            password.set(input.value());
        })
    };

    let on_confirm_password_input = {
        let confirm_password = confirm_password.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            confirm_password.set(input.value());
        })
    };

    html! {
        <div class="flex items-center justify-center min-h-[60vh]">
            <div class="max-w-md w-full space-y-6">
                <div class="text-center">
                    <h1 class="text-3xl font-bold text-neutral-900 dark:text-white mb-2">
                        {"Set new password"}
                    </h1>
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"Enter your new password below"}
                    </p>
                </div>

                if *success {
                    <div class="bg-white dark:bg-neutral-800 border border-neutral-200 dark:border-neutral-700 rounded-lg p-6 space-y-4">
                        <div class="text-center">
                            <p class="text-neutral-900 dark:text-white font-semibold mb-2">
                                {"Password reset successful"}
                            </p>
                            <p class="text-sm text-neutral-600 dark:text-neutral-400 mb-4">
                                {"Your password has been updated. You can now log in with your new password."}
                            </p>
                            <button
                                onclick={Callback::from(move |_| navigator.push(&Route::Login))}
                                class="bg-neutral-900 dark:bg-white text-white dark:text-neutral-900
                                       px-6 py-2 rounded-md hover:bg-neutral-800 dark:hover:bg-neutral-100
                                       font-medium"
                            >
                                {"Go to login"}
                            </button>
                        </div>
                    </div>
                } else {
                    <form onsubmit={onsubmit} class="bg-white dark:bg-neutral-800 border border-neutral-200 dark:border-neutral-700 rounded-lg p-6 space-y-4">
                        if let Some(error_msg) = (*error).as_ref() {
                            <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-md p-3">
                                <p class="text-sm text-red-800 dark:text-red-200">{error_msg}</p>
                            </div>
                        }

                        <div>
                            <label for="password" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                {"New password"}
                            </label>
                            <input
                                type="password"
                                id="password"
                                required={true}
                                value={(*password).clone()}
                                oninput={on_password_input}
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600 rounded-md
                                       bg-white dark:bg-neutral-900 text-neutral-900 dark:text-white
                                       focus:outline-none focus:ring-2 focus:ring-neutral-500"
                            />
                            <p class="text-xs text-neutral-500 dark:text-neutral-500 mt-1">
                                {"Must be at least 8 characters"}
                            </p>
                        </div>

                        <div>
                            <label for="confirm_password" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                {"Confirm new password"}
                            </label>
                            <input
                                type="password"
                                id="confirm_password"
                                required={true}
                                value={(*confirm_password).clone()}
                                oninput={on_confirm_password_input}
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600 rounded-md
                                       bg-white dark:bg-neutral-900 text-neutral-900 dark:text-white
                                       focus:outline-none focus:ring-2 focus:ring-neutral-500"
                            />
                        </div>

                        <button
                            type="submit"
                            disabled={*loading}
                            class="w-full bg-neutral-900 dark:bg-white text-white dark:text-neutral-900
                                   px-4 py-2 rounded-md hover:bg-neutral-800 dark:hover:bg-neutral-100
                                   disabled:opacity-50 disabled:cursor-not-allowed font-medium"
                        >
                            {if *loading { "Resetting..." } else { "Reset password" }}
                        </button>
                    </form>
                }
            </div>
        </div>
    }
}
