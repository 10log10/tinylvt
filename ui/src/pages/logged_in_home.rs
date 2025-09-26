use payloads::responses;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub profile: responses::UserProfile,
}

#[function_component]
pub fn LoggedInHomePage(props: &Props) -> Html {
    let profile = &props.profile;
    let navigator = use_navigator().unwrap();

    let on_view_communities = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            navigator.push(&Route::Communities);
        })
    };

    html! {
        <div class="space-y-8">
            <div class="text-center">
                <h1 class="text-3xl font-bold text-neutral-900 dark:text-neutral-100 mb-4">
                    {format!("Welcome back, {}!", profile.username)}
                </h1>
                <p class="text-lg text-neutral-600 dark:text-neutral-400">
                    {"You're successfully logged in to TinyLVT"}
                </p>
            </div>

            if !profile.email_verified {
                <div class="bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg p-6">
                    <div class="flex items-start space-x-3">
                        <div class="flex-shrink-0">
                            <svg class="h-6 w-6 text-amber-600 dark:text-amber-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L3.732 16.5c-.77.833.192 2.5 1.732 2.5z"></path>
                            </svg>
                        </div>
                        <div class="flex-1">
                            <h3 class="text-lg font-semibold text-amber-800 dark:text-amber-200 mb-2">
                                {"Email Verification Required"}
                            </h3>
                            <p class="text-sm text-amber-700 dark:text-amber-300 mb-4">
                                {"Please verify your email address before you can create or join communities. Check your inbox for a verification email we sent to "}
                                <span class="font-medium">{&profile.email}</span>
                                {"."}
                            </p>
                            <p class="text-xs text-amber-600 dark:text-amber-400">
                                {"Didn't receive the email? Check your spam folder or contact support for help."}
                            </p>
                        </div>
                    </div>
                </div>
            }

            <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700">
                    <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100 mb-2">
                        {"Profile"}
                    </h2>
                    <div class="space-y-2 text-sm text-neutral-600 dark:text-neutral-400">
                        <p><span class="font-medium">{"Username: "}</span> {&profile.username}</p>
                        <p><span class="font-medium">{"Email: "}</span> {&profile.email}</p>
                        <p><span class="font-medium">{"Email Verified: "}</span> {
                            if profile.email_verified { "Yes" } else { "No" }
                        }</p>
                    </div>
                </div>

                <div class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700">
                    <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100 mb-2">
                        {"Communities"}
                    </h2>
                    <p class="text-sm text-neutral-600 dark:text-neutral-400 mb-4">
                        {"Manage your community memberships and create new communities"}
                    </p>
                    if profile.email_verified {
                        <button
                            onclick={on_view_communities}
                            class="w-full bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors"
                        >
                            {"View Communities"}
                        </button>
                    } else {
                        <button
                            disabled={true}
                            class="w-full bg-neutral-400 dark:bg-neutral-600 text-neutral-200 dark:text-neutral-400 px-4 py-2 rounded-md text-sm font-medium cursor-not-allowed"
                            title="Email verification required"
                        >
                            {"Verify Email First"}
                        </button>
                    }
                </div>
            </div>
        </div>
    }
}
