use payloads::requests;
use yew::prelude::*;

use crate::Route;
use crate::contexts::use_toast;
use crate::hooks::{use_push_route, use_title};

#[function_component]
pub fn VerifyEmailPage() -> Html {
    use_title("Verify Email - TinyLVT");
    let push_route = use_push_route();
    let toast = use_toast();

    let is_verifying = use_state(|| true);
    let error_message = use_state(|| None::<String>);
    let success = use_state(|| false);

    // Extract token from query string and verify on mount
    {
        let is_verifying = is_verifying.clone();
        let error_message = error_message.clone();
        let success = success.clone();
        let push_route = push_route.clone();
        let toast = toast.clone();

        use_effect_with((), move |_| {
            let is_verifying = is_verifying.clone();
            let error_message = error_message.clone();
            let success = success.clone();
            let push_route = push_route.clone();
            let toast = toast.clone();

            yew::platform::spawn_local(async move {
                // Extract token from query parameter
                let window = web_sys::window().unwrap();
                let location = window.location();
                let search = location.search().unwrap_or_default();

                // Parse query string manually (format: "?token=xxx")
                let token = if search.starts_with("?token=") {
                    search.trim_start_matches("?token=")
                } else {
                    ""
                };

                if token.is_empty() {
                    error_message.set(Some(
                        "No verification token provided.".to_string(),
                    ));
                    is_verifying.set(false);
                    return;
                }

                // Call verify_email API
                let api_client = crate::get_api_client();
                match api_client
                    .verify_email(&requests::VerifyEmail {
                        token: token.to_string(),
                    })
                    .await
                {
                    Ok(_) => {
                        is_verifying.set(false);
                        success.set(true);
                        toast.success("Email verified successfully!");

                        // Wait before redirecting
                        yew::platform::time::sleep(
                            std::time::Duration::from_secs(2),
                        )
                        .await;

                        // Redirect to home - it will show login form if not
                        // authenticated
                        push_route.emit(Route::Home);
                    }
                    Err(e) => {
                        is_verifying.set(false);
                        error_message.set(Some(e.to_string()));
                    }
                }
            });
        });
    }

    html! {
        <div class="flex items-center justify-center min-h-[60vh]">
            <div class="max-w-md w-full bg-white dark:bg-neutral-800 p-8 \
                        rounded-lg shadow-md">
                <div class="mb-8 text-center">
                    <h1 class="text-2xl font-bold text-neutral-900 \
                               dark:text-neutral-100 mb-2">
                        {"Email Verification"}
                    </h1>
                </div>

                if *is_verifying {
                    <div class="text-center py-8">
                        <div class="animate-spin rounded-full h-12 w-12 \
                                    border-b-2 border-neutral-600 \
                                    dark:border-neutral-400 mx-auto mb-4">
                        </div>
                        <p class="text-neutral-600 dark:text-neutral-400">
                            {"Verifying your email..."}
                        </p>
                    </div>
                } else if *success {
                    <div class="text-center py-8">
                        <div class="mb-4">
                            <svg class="mx-auto h-12 w-12 text-green-600 \
                                        dark:text-green-400"
                                 fill="none"
                                 stroke="currentColor"
                                 viewBox="0 0 24 24">
                                <path stroke-linecap="round"
                                      stroke-linejoin="round"
                                      stroke-width="2"
                                      d="M5 13l4 4L19 7" />
                            </svg>
                        </div>
                        <p class="text-neutral-900 dark:text-neutral-100 \
                                  font-medium mb-2">
                            {"Email verified successfully!"}
                        </p>
                        <p class="text-sm text-neutral-600 \
                                  dark:text-neutral-400">
                            {"Redirecting..."}
                        </p>
                    </div>
                } else if let Some(error) = &*error_message {
                    <>
                        <div class="mb-6 p-4 rounded-md bg-red-50 \
                                    dark:bg-red-900 border border-red-200 \
                                    dark:border-red-800">
                            <p class="text-sm text-red-700 \
                                      dark:text-red-400">
                                {error}
                            </p>
                        </div>
                        <button
                            onclick={push_route.reform(|_| Route::Login)}
                            class="w-full flex justify-center py-2 px-4 \
                                   border border-transparent rounded-md \
                                   shadow-sm text-sm font-medium text-white \
                                   bg-neutral-900 hover:bg-neutral-800 \
                                   dark:bg-neutral-100 dark:text-neutral-900 \
                                   dark:hover:bg-neutral-200 \
                                   focus:outline-none focus:ring-2 \
                                   focus:ring-offset-2 \
                                   focus:ring-neutral-500 \
                                   transition-colors duration-200"
                        >
                            {"Go to Login"}
                        </button>
                    </>
                }
            </div>
        </div>
    }
}
