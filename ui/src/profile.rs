use crate::get_api_client;
use payloads::{requests, responses};
use yew::prelude::*;

#[function_component]
pub fn Profile() -> Html {
    let profile = use_state(|| None::<responses::UserProfile>);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let edit_name = use_state(|| String::new());
    let saving = use_state(|| false);
    let save_msg = use_state(|| None::<String>);
    let resending_verification = use_state(|| false);
    let verification_msg = use_state(|| None::<String>);

    // Fetch profile on mount
    {
        let profile = profile.clone();
        let loading = loading.clone();
        let error = error.clone();
        let edit_name = edit_name.clone();
        use_effect_with((), move |_| {
            yew::platform::spawn_local(async move {
                loading.set(true);
                let client = get_api_client();
                match client.user_profile().await {
                    Ok(p) => {
                        edit_name
                            .set(p.display_name.clone().unwrap_or_default());
                        profile.set(Some(p));
                        loading.set(false);
                    }
                    Err(e) => {
                        error.set(Some(format!(
                            "Failed to load profile: {}",
                            e
                        )));
                        loading.set(false);
                    }
                }
            });
            || ()
        });
    }

    // Save profile handler
    let on_save = {
        let saving = saving.clone();
        let save_msg = save_msg.clone();
        let profile = profile.clone();
        let edit_name = edit_name.clone();
        Callback::from(move |_: MouseEvent| {
            let saving = saving.clone();
            let save_msg = save_msg.clone();
            let profile = profile.clone();
            let edit_name = edit_name.clone();
            yew::platform::spawn_local(async move {
                saving.set(true);
                save_msg.set(None);
                let client = get_api_client();
                let req = requests::UpdateProfile {
                    display_name: if edit_name.is_empty() {
                        None
                    } else {
                        Some((*edit_name).clone())
                    },
                };
                match client.update_profile(&req).await {
                    Ok(updated) => {
                        profile.set(Some(updated));
                        save_msg.set(Some(
                            "Profile updated successfully!".to_string(),
                        ));
                    }
                    Err(e) => {
                        save_msg.set(Some(format!(
                            "Failed to update profile: {}",
                            e
                        )));
                    }
                }
                saving.set(false);
            });
        })
    };

    // Resend verification email handler
    let on_resend_verification = {
        let profile = profile.clone();
        let resending_verification = resending_verification.clone();
        let verification_msg = verification_msg.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(profile_data) = profile.as_ref() {
                let email = profile_data.email.clone();
                let resending_verification = resending_verification.clone();
                let verification_msg = verification_msg.clone();
                yew::platform::spawn_local(async move {
                    resending_verification.set(true);
                    verification_msg.set(None);
                    let client = get_api_client();
                    let req = requests::ResendVerificationEmail { email };
                    match client.resend_verification_email(&req).await {
                        Ok(response) => {
                            verification_msg.set(Some(response.message));
                        }
                        Err(e) => {
                            verification_msg.set(Some(format!(
                                "Failed to resend verification: {}",
                                e
                            )));
                        }
                    }
                    resending_verification.set(false);
                });
            }
        })
    };

    let on_name_input = {
        let edit_name = edit_name.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            edit_name.set(input.value());
        })
    };

    html! {
        <main class="min-h-screen bg-gray-50 dark:bg-gray-900 py-8">
            <div class="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8">
                <div class="bg-white dark:bg-gray-800 shadow rounded-lg">
                    <div class="px-4 py-5 sm:p-6">
                        <h1 class="text-2xl font-bold text-gray-900 dark:text-white mb-6">{"Profile Settings"}</h1>

                        if *loading {
                            <div class="flex justify-center items-center py-12">
                                <svg class="animate-spin h-8 w-8 text-blue-600" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                </svg>
                                <span class="ml-2 text-gray-600 dark:text-gray-400">{"Loading profile..."}</span>
                            </div>
                        } else if let Some(err) = error.as_ref() {
                            <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 px-4 py-3 rounded-md">
                                <div class="flex">
                                    <svg class="h-5 w-5 text-red-400 mr-2" viewBox="0 0 20 20" fill="currentColor">
                                        <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd"></path>
                                    </svg>
                                    {err}
                                </div>
                            </div>
                        } else if let Some(profile) = profile.as_ref() {
                            <div class="space-y-8">
                                // Account Information Section
                                <div>
                                    <h2 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Account Information"}</h2>
                                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                                        <div>
                                            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{"Username"}</label>
                                            <div class="px-3 py-2 bg-gray-50 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-md text-gray-900 dark:text-white">
                                                {&profile.username}
                                            </div>
                                        </div>
                                        <div>
                                            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{"Email Address"}</label>
                                            <div class="px-3 py-2 bg-gray-50 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-md text-gray-900 dark:text-white">
                                                {&profile.email}
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                // Email Verification Section
                                <div>
                                    <h2 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Email Verification"}</h2>
                                    <div class={format!("flex items-center justify-between p-4 border rounded-md {}", if profile.email_verified {"border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-900/20"} else {"border-yellow-200 bg-yellow-50 dark:border-yellow-800 dark:bg-yellow-900/20"})}>
                                        <div class="flex items-center">
                                            if profile.email_verified {
                                                <svg class="h-5 w-5 text-green-400 mr-2" viewBox="0 0 20 20" fill="currentColor">
                                                    <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd"></path>
                                                </svg>
                                                <span class="text-green-800 dark:text-green-200 font-medium">{"Email verified"}</span>
                                            } else {
                                                <svg class="h-5 w-5 text-yellow-400 mr-2" viewBox="0 0 20 20" fill="currentColor">
                                                    <path fill-rule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clip-rule="evenodd"></path>
                                                </svg>
                                                <span class="text-yellow-800 dark:text-yellow-200 font-medium">{"Email not verified"}</span>
                                            }
                                        </div>
                                        if !profile.email_verified {
                                            <button
                                                onclick={on_resend_verification}
                                                disabled={*resending_verification}
                                                class="px-3 py-1 text-sm font-medium text-yellow-800 dark:text-yellow-200 bg-yellow-100 dark:bg-yellow-800 hover:bg-yellow-200 dark:hover:bg-yellow-700 border border-yellow-300 dark:border-yellow-600 rounded-md disabled:opacity-50 disabled:cursor-not-allowed"
                                            >
                                                if *resending_verification {
                                                    {"Sending..."}
                                                } else {
                                                    {"Resend verification email"}
                                                }
                                            </button>
                                        }
                                    </div>
                                    if let Some(msg) = verification_msg.as_ref() {
                                        <div class="mt-2 text-sm text-blue-600 dark:text-blue-400">
                                            {msg}
                                        </div>
                                    }
                                </div>

                                // Profile Settings Section
                                <div>
                                    <h2 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Profile Settings"}</h2>
                                    <div class="space-y-4">
                                        <div>
                                            <label for="display-name" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                                {"Display Name"}
                                                <span class="text-gray-500 dark:text-gray-400 font-normal">{" (optional)"}</span>
                                            </label>
                                            <input
                                                id="display-name"
                                                type="text"
                                                class="block w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 sm:text-sm"
                                                value={(*edit_name).clone()}
                                                oninput={on_name_input}
                                                disabled={*saving}
                                                placeholder="Enter your display name"
                                            />
                                            <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                                                {"This name will be shown to other community members."}
                                            </p>
                                        </div>

                                        if let Some(msg) = save_msg.as_ref() {
                                            <div class={format!("text-sm {}", if msg.contains("success") {"text-green-600 dark:text-green-400"} else {"text-red-600 dark:text-red-400"})}>
                                                {msg}
                                            </div>
                                        }

                                        <button
                                            type="button"
                                            onclick={on_save}
                                            disabled={*saving}
                                            class={format!("px-4 py-2 rounded-md shadow-sm text-sm font-medium focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 border {}",
                                                if *saving {
                                                    "border-gray-300 bg-gray-300 text-gray-500 cursor-not-allowed opacity-50"
                                                } else {
                                                    "border-transparent bg-blue-600 text-white hover:bg-blue-700"
                                                }
                                            )}
                                        >
                                            if *saving {
                                                <span class="flex items-center">
                                                    <svg class="animate-spin -ml-1 mr-2 h-4 w-4 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                                    </svg>
                                                    {"Saving..."}
                                                </span>
                                            } else {
                                                {"Save Changes"}
                                            }
                                        </button>
                                    </div>
                                </div>

                                // Account Balance Section - Commented out for MVP
                                // Payment functionality omitted; participants handle settlement themselves
                                /*
                                <div>
                                    <h2 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Account Balance"}</h2>
                                    <div class="p-4 bg-gray-50 dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded-md">
                                        <div class="text-2xl font-bold text-gray-900 dark:text-white font-mono">
                                            {"$"}{profile.balance.to_string()}
                                        </div>
                                        <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                                            {"Available for auction bidding"}
                                        </p>
                                    </div>
                                </div>
                                */

                                // Security Section
                                <div>
                                    <h2 class="text-lg font-medium text-gray-900 dark:text-white mb-4">{"Security"}</h2>
                                    <div class="space-y-3">
                                        <a
                                            href="/forgot-password"
                                            class="inline-flex items-center px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 hover:bg-gray-50 dark:hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                                        >
                                            <svg class="h-4 w-4 mr-2" viewBox="0 0 20 20" fill="currentColor">
                                                <path fill-rule="evenodd" d="M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z" clip-rule="evenodd"></path>
                                            </svg>
                                            {"Change Password"}
                                        </a>
                                    </div>
                                </div>
                            </div>
                        }
                    </div>
                </div>
            </div>
        </main>
    }
}
