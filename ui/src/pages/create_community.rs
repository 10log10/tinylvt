use payloads::{CurrencyModeConfig, CurrencySettings, IOUConfig, requests};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::Route;
use crate::components::{CurrencyConfigEditor, RequireAuth};
use crate::hooks::{use_communities, use_push_route};

#[function_component]
pub fn CreateCommunityPage() -> Html {
    html! {
        <RequireAuth>
            <CreateCommunityPageInner />
        </RequireAuth>
    }
}

#[function_component]
fn CreateCommunityPageInner() -> Html {
    let push_route = use_push_route();
    let communities_hook = use_communities();

    let name_ref = use_node_ref();
    let error_message = use_state(|| None::<String>);
    let is_loading = use_state(|| false);

    // Currency config state
    let currency = use_state(|| CurrencySettings {
        mode_config: CurrencyModeConfig::DistributedClearing(IOUConfig {
            default_credit_limit: None,
            debts_callable: true,
        }),
        name: "dollars".to_string(),
        symbol: "$".to_string(),
        minor_units: 2,
        balances_visible_to_members: true,
        new_members_default_active: true,
    });

    let on_submit = {
        let name_ref = name_ref.clone();
        let error_message = error_message.clone();
        let is_loading = is_loading.clone();
        let push_route = push_route.clone();
        let currency = currency.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let name_input = name_ref.cast::<HtmlInputElement>().unwrap();
            let name = name_input.value().trim().to_string();

            if name.is_empty() {
                error_message
                    .set(Some("Please enter a community name".to_string()));
                return;
            }

            let community_request = requests::CreateCommunity {
                name,
                description: None,
                currency: (*currency).clone(),
            };

            let error_message = error_message.clone();
            let is_loading = is_loading.clone();
            let push_route = push_route.clone();
            let refetch_communities = communities_hook.refetch.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error_message.set(None);

                let api_client = crate::get_api_client();
                match api_client.create_community(&community_request).await {
                    Ok(_community_id) => {
                        // Refresh communities in global state
                        refetch_communities.emit(());
                        // Navigate back to communities page
                        push_route.emit(Route::Communities);
                    }
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    let on_cancel = {
        let push_route = push_route.clone();
        Callback::from(move |_| {
            push_route.emit(Route::Communities);
        })
    };

    let on_currency_config_change = {
        let currency = currency.clone();

        Callback::from(move |new_currency: CurrencySettings| {
            currency.set(new_currency);
        })
    };

    html! {
        <div class="flex items-center justify-center min-h-[60vh]">
            <div class="max-w-2xl w-full bg-white dark:bg-neutral-800 p-8 rounded-lg shadow-md">
                <div class="mb-8 text-center">
                    <h1 class="text-2xl font-bold text-neutral-900 dark:text-neutral-100 mb-2">
                        {"Create New Community"}
                    </h1>
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"Set up your new community space"}
                    </p>
                </div>

                <form onsubmit={on_submit} class="space-y-6">
                    if let Some(error) = &*error_message {
                        <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                            <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
                        </div>
                    }

                    <div>
                        <label for="community-name" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                            {"Community Name"}
                        </label>
                        <input
                            ref={name_ref}
                            type="text"
                            id="community-name"
                            name="name"
                            required={true}
                            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm bg-white dark:bg-neutral-700 
                                   text-neutral-900 dark:text-neutral-100
                                   focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                   dark:focus:ring-neutral-400 dark:focus:border-neutral-400"
                            placeholder="Enter community name"
                        />
                    </div>

                    <div>
                        <h2 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                            {"Currency Configuration"}
                        </h2>
                        <CurrencyConfigEditor
                            currency={(*currency).clone()}
                            on_change={on_currency_config_change}
                            disabled={*is_loading}
                            can_change_mode={true}
                        />
                    </div>

                    <div class="flex space-x-3">
                        <button
                            type="button"
                            onclick={on_cancel}
                            disabled={*is_loading}
                            class="flex-1 py-2 px-4 border border-neutral-300 dark:border-neutral-600
                                   rounded-md shadow-sm text-sm font-medium text-neutral-700 dark:text-neutral-300
                                   bg-white dark:bg-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-600
                                   focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed
                                   transition-colors duration-200"
                        >
                            {"Cancel"}
                        </button>

                        <button
                            type="submit"
                            disabled={*is_loading}
                            class="flex-1 flex justify-center py-2 px-4 border border-transparent
                                   rounded-md shadow-sm text-sm font-medium text-white
                                   bg-neutral-900 hover:bg-neutral-800 
                                   dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200
                                   focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed
                                   transition-colors duration-200"
                        >
                            if *is_loading {
                                {"Creating..."}
                            } else {
                                {"Create Community"}
                            }
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}
