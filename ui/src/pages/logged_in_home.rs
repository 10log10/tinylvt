use payloads::responses;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub profile: responses::UserProfile,
}

#[function_component]
pub fn LoggedInHomePage(props: &Props) -> Html {
    let profile = &props.profile;

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
                    <button class="w-full bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors">
                        {"View Communities"}
                    </button>
                </div>
            </div>
        </div>
    }
}
