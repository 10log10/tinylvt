use crate::get_api_client;
use crate::hooks::use_title;
use payloads::requests;
use yew::prelude::*;

#[function_component]
pub fn ForgotPasswordPage() -> Html {
    use_title("Forgot Password - TinyLVT");
    let email = use_state(String::new);
    let error = use_state(|| None::<String>);
    let success = use_state(|| false);
    let loading = use_state(|| false);

    let onsubmit = {
        let email = email.clone();
        let error = error.clone();
        let success = success.clone();
        let loading = loading.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let email = email.clone();
            let error = error.clone();
            let success = success.clone();
            let loading = loading.clone();

            loading.set(true);
            error.set(None);

            let email_value = (*email).clone();

            wasm_bindgen_futures::spawn_local(async move {
                let client = get_api_client();
                let request = requests::ForgotPassword { email: email_value };

                match client.forgot_password(&request).await {
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

    let on_email_input = {
        let email = email.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            email.set(input.value());
        })
    };

    html! {
        <div class="flex items-center justify-center min-h-[60vh]">
            <div class="max-w-md w-full space-y-6">
                <div class="text-center">
                    <h1 class="text-3xl font-bold text-neutral-900 dark:text-white mb-2">
                        {"Reset your password"}
                    </h1>
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"Enter your email address and we'll send you a link to reset your password"}
                    </p>
                </div>

                if *success {
                    <div class="bg-white dark:bg-neutral-800 border border-neutral-200 dark:border-neutral-700 rounded-lg p-6">
                        <div class="text-center">
                            <p class="text-neutral-900 dark:text-white font-semibold mb-2">
                                {"Check your email"}
                            </p>
                            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                                {"If an account with that email exists, a password reset link has been sent."}
                            </p>
                            <p class="text-xs text-neutral-500 dark:text-neutral-500 mt-4">
                                {"Check your spam folder if you don't see it in your inbox."}
                            </p>
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
                            <label for="email" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                                {"Email address"}
                            </label>
                            <input
                                type="email"
                                id="email"
                                required={true}
                                value={(*email).clone()}
                                oninput={on_email_input}
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
                            {if *loading { "Sending..." } else { "Send reset link" }}
                        </button>
                    </form>
                }
            </div>
        </div>
    }
}
