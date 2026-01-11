use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;
use crate::components::RequireAuth;
use crate::hooks::use_communities;

#[function_component]
pub fn CommunitiesPage() -> Html {
    html! {
        <RequireAuth>
            <CommunitiesPageInner />
        </RequireAuth>
    }
}

#[function_component]
fn CommunitiesPageInner() -> Html {
    let navigator = use_navigator().unwrap();
    let communities_hook = use_communities();

    let on_create_community = {
        let navigator = navigator.clone();
        Callback::from(move |_| {
            navigator.push(&Route::CreateCommunity);
        })
    };

    html! {
        <div class="space-y-8">
            <div class="flex justify-between items-center">
                <div>
                    <h1 class="text-3xl font-bold text-neutral-900 dark:text-neutral-100">
                        {"Communities"}
                    </h1>
                    <p class="text-lg text-neutral-600 dark:text-neutral-400 mt-2">
                        {"Manage your community memberships"}
                    </p>
                </div>
                <button
                    onclick={on_create_community.clone()}
                    class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors"
                >
                    {"Create New Community"}
                </button>
            </div>

            if communities_hook.is_loading {
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">{"Loading communities..."}</p>
                </div>
            } else if let Some(error) = &communities_hook.error {
                <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                    <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
                </div>
            } else if let Some(community_list) = communities_hook.communities.as_ref() {
                if community_list.is_empty() {
                    <div class="text-center py-12">
                        <p class="text-neutral-600 dark:text-neutral-400 mb-4">
                            {"You're not a member of any communities yet."}
                        </p>
                        <button
                            onclick={on_create_community.clone()}
                            class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors"
                        >
                            {"Create Your First Community"}
                        </button>
                    </div>
                } else {
                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                        {community_list.iter().map(|community| {
                            html! {
                                <div key={community.id.to_string()} class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700">
                                    <div class="space-y-4">
                                        <div>
                                            <h3 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                                                {&community.name}
                                            </h3>
                                            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                                                {"Role: "}{format!("{:?}", community.user_role)}
                                            </p>
                                        </div>

                                        <div class="text-sm text-neutral-600 dark:text-neutral-400">
                                            <p>{"Created: "}{community.created_at.to_zoned(jiff::tz::TimeZone::system()).strftime("%B %d, %Y").to_string()}</p>
                                        </div>

                                        <div class="pt-2">
                                            <Link<Route>
                                                to={Route::CommunityDetail { id: community.id }}
                                                classes="block w-full bg-neutral-100 hover:bg-neutral-200 dark:bg-neutral-700 dark:hover:bg-neutral-600 text-neutral-900 dark:text-neutral-100 px-4 py-2 rounded-md text-sm font-medium transition-colors text-center"
                                            >
                                                {"View Details"}
                                            </Link<Route>>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect::<Html>()}
                    </div>
                }
            }
        </div>
    }
}
