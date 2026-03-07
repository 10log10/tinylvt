use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub email: AttrValue,
}

#[function_component]
pub fn EmailVerificationBanner(props: &Props) -> Html {
    let resend_loading = use_state(|| false);
    let resend_success = use_state(|| false);
    let resend_error = use_state(|| false);

    let on_resend_email = {
        let resend_loading = resend_loading.clone();
        let resend_success = resend_success.clone();
        let resend_error = resend_error.clone();

        Callback::from(move |_: MouseEvent| {
            let resend_loading = resend_loading.clone();
            let resend_success = resend_success.clone();
            let resend_error = resend_error.clone();

            resend_loading.set(true);
            resend_success.set(false);
            resend_error.set(false);

            wasm_bindgen_futures::spawn_local(async move {
                let client = crate::get_api_client();

                match client.resend_verification_email().await {
                    Ok(_) => {
                        resend_success.set(true);
                        resend_loading.set(false);
                    }
                    Err(_) => {
                        resend_error.set(true);
                        resend_loading.set(false);
                    }
                }
            });
        })
    };

    html! {
        <div class="bg-amber-50 dark:bg-amber-900/20 border border-amber-200 \
                    dark:border-amber-800 rounded-lg p-6">
            <div class="flex items-start space-x-3">
                <div class="flex-shrink-0">
                    <svg class="h-6 w-6 text-amber-600 dark:text-amber-400"
                         fill="none"
                         stroke="currentColor"
                         viewBox="0 0 24 24">
                        <path stroke-linecap="round"
                              stroke-linejoin="round"
                              stroke-width="2"
                              d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 \
                                 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964\
                                 -.833-2.732 0L3.732 16.5c-.77.833.192 2.5 \
                                 1.732 2.5z" />
                    </svg>
                </div>
                <div class="flex-1">
                    <h3 class="text-lg font-semibold text-amber-800 \
                               dark:text-amber-200 mb-2">
                        {"Email Verification Required"}
                    </h3>
                    <p class="text-sm text-amber-700 dark:text-amber-300 mb-4">
                        {"Please verify your email address before you can \
                          create or join communities. Check your inbox for a \
                          verification email we sent to "}
                        <span class="font-medium">{&props.email}</span>
                        {"."}
                    </p>
                    if *resend_success {
                        <div class="bg-green-50 dark:bg-green-900/20 border \
                                    border-green-200 dark:border-green-800 \
                                    rounded-md p-3 mb-3">
                            <p class="text-sm text-green-800 \
                                      dark:text-green-200">
                                {"Verification email sent! Check your inbox."}
                            </p>
                        </div>
                    }
                    if *resend_error {
                        <div class="bg-red-50 dark:bg-red-900/20 border \
                                    border-red-200 dark:border-red-800 \
                                    rounded-md p-3 mb-3">
                            <p class="text-sm text-red-800 dark:text-red-200">
                                {"Failed to send verification email. Please \
                                  try again."}
                            </p>
                        </div>
                    }
                    <div class="flex items-center gap-2 text-xs text-amber-600 \
                                dark:text-amber-400">
                        <span>{"Didn't receive the email?"}</span>
                        <button
                            onclick={on_resend_email}
                            disabled={*resend_loading}
                            class="underline hover:text-amber-700 \
                                   dark:hover:text-amber-300 \
                                   disabled:opacity-50 \
                                   disabled:cursor-not-allowed font-medium"
                        >
                            {if *resend_loading {
                                "Sending..."
                            } else {
                                "Resend email"
                            }}
                        </button>
                        <span>{"or"}</span>
                        <Link<Route>
                            to={Route::Help}
                            classes="underline hover:text-amber-700 \
                                     dark:hover:text-amber-300"
                        >
                            {"contact support"}
                        </Link<Route>>
                    </div>
                </div>
            </div>
        </div>
    }
}
