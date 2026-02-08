use payloads::{CommunityId, requests, responses::CommunityWithRole};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::{
    ActiveTab, CommunityPageWrapper, CommunityTabHeader, ConfirmationModal,
    CurrencyConfigEditor, LeaveCommunityButton,
};
use crate::hooks::use_communities;
use crate::{Route, get_api_client};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
}

#[function_component]
pub fn CommunitySettingsPage(props: &Props) -> Html {
    let render_content = {
        let community_id = props.community_id;
        Callback::from(move |community: CommunityWithRole| {
            html! {
                <CommunitySettingsContent
                    community={community}
                    community_id={community_id}
                />
            }
        })
    };

    html! {
        <CommunityPageWrapper
            community_id={props.community_id}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
struct ContentProps {
    pub community: CommunityWithRole,
    pub community_id: CommunityId,
}

#[function_component]
fn CommunitySettingsContent(props: &ContentProps) -> Html {
    let navigator = use_navigator().unwrap();
    let communities_hook = use_communities();

    // Permission flags
    let is_coleader_plus = props.community.user_role.is_ge_coleader();
    let is_leader = props.community.user_role.is_leader();

    // Edited community state - holds all currency config fields
    let edited_community = use_state(|| props.community.community.clone());

    // Currency save state
    let is_saving = use_state(|| false);
    let save_error = use_state(|| None::<String>);
    let save_success = use_state(|| false);

    // Delete modal state
    let show_delete_modal = use_state(|| false);
    let is_deleting = use_state(|| false);
    let delete_error = use_state(|| None::<String>);

    let community_name = props.community.name.clone();

    // Check if currency config has changes
    let has_changes = *edited_community != props.community.community;

    // Currency config change handler
    let on_currency_config_change = {
        let edited_community = edited_community.clone();
        let save_success = save_success.clone();
        let save_error = save_error.clone();

        Callback::from(move |new_currency: payloads::CurrencySettings| {
            let mut updated = (*edited_community).clone();
            updated.currency = new_currency;
            edited_community.set(updated);
            save_success.set(false);
            save_error.set(None);
        })
    };

    // Save currency config handler
    let on_save_currency = {
        let edited_community = edited_community.clone();
        let is_saving = is_saving.clone();
        let save_error = save_error.clone();
        let save_success = save_success.clone();
        let community_id = props.community_id;
        let refetch_communities = communities_hook.refetch.clone();

        Callback::from(move |_| {
            let edited = (*edited_community).clone();
            let is_saving = is_saving.clone();
            let save_error = save_error.clone();
            let save_success = save_success.clone();
            let refetch_communities = refetch_communities.clone();

            is_saving.set(true);
            save_error.set(None);
            save_success.set(false);

            wasm_bindgen_futures::spawn_local(async move {
                let client = get_api_client();
                let details = requests::UpdateCurrencyConfig {
                    community_id,
                    currency: edited.currency,
                };

                match client.update_currency_config(&details).await {
                    Ok(_) => {
                        save_success.set(true);
                        refetch_communities.emit(());
                    }
                    Err(e) => {
                        save_error.set(Some(e.to_string()));
                    }
                }
                is_saving.set(false);
            });
        })
    };

    // Cancel currency config changes
    let on_cancel_currency = {
        let edited_community = edited_community.clone();
        let save_success = save_success.clone();
        let save_error = save_error.clone();
        let original = props.community.community.clone();

        Callback::from(move |_| {
            edited_community.set(original.clone());
            save_success.set(false);
            save_error.set(None);
        })
    };

    // Delete modal handlers
    let on_open_delete_modal = {
        let show_delete_modal = show_delete_modal.clone();
        let delete_error = delete_error.clone();
        Callback::from(move |_| {
            delete_error.set(None);
            show_delete_modal.set(true);
        })
    };

    let on_close_delete_modal = {
        let show_delete_modal = show_delete_modal.clone();
        Callback::from(move |()| {
            show_delete_modal.set(false);
        })
    };

    let on_delete = {
        let is_deleting = is_deleting.clone();
        let delete_error = delete_error.clone();
        let navigator = navigator.clone();
        let community_id = props.community_id;
        let refetch_communities = communities_hook.refetch.clone();

        Callback::from(move |()| {
            let is_deleting = is_deleting.clone();
            let delete_error = delete_error.clone();
            let navigator = navigator.clone();
            let refetch_communities = refetch_communities.clone();

            is_deleting.set(true);
            delete_error.set(None);

            wasm_bindgen_futures::spawn_local(async move {
                let client = get_api_client();
                match client.delete_community(&community_id).await {
                    Ok(_) => {
                        refetch_communities.emit(());
                        navigator.push(&Route::Communities);
                    }
                    Err(e) => {
                        delete_error.set(Some(e.to_string()));
                        is_deleting.set(false);
                    }
                }
            });
        })
    };

    html! {
        <div>
            <CommunityTabHeader
                community={props.community.clone()}
                active_tab={ActiveTab::Settings}
            />

            <div class="py-6 max-w-2xl space-y-8">
                // Currency Configuration Section (visible to all members)
                <div>
                    <h2 class="text-xl font-semibold text-neutral-900 \
                               dark:text-neutral-100 mb-6">
                        {"Currency Configuration"}
                    </h2>

                    <CurrencyConfigEditor
                        currency={edited_community.currency.clone()}
                        on_change={on_currency_config_change}
                        disabled={!is_coleader_plus || *is_saving}
                        can_change_mode={false}
                    />

                    // Save/Cancel buttons (only for coleaders+)
                    if is_coleader_plus {
                        <div class="mt-6 flex gap-3">
                            <button
                                onclick={on_save_currency}
                                disabled={!has_changes || *is_saving}
                                class="px-4 py-2 text-sm font-medium text-white \
                                       bg-neutral-900 dark:bg-neutral-100 \
                                       dark:text-neutral-900 rounded-md \
                                       hover:bg-neutral-700 \
                                       dark:hover:bg-neutral-300 \
                                       transition-colors disabled:opacity-50 \
                                       disabled:cursor-not-allowed"
                            >
                                {if *is_saving { "Saving..." } else { "Save Changes" }}
                            </button>
                            <button
                                onclick={on_cancel_currency}
                                disabled={!has_changes || *is_saving}
                                class="px-4 py-2 text-sm font-medium \
                                       text-neutral-700 dark:text-neutral-300 \
                                       bg-white dark:bg-neutral-800 \
                                       border border-neutral-300 \
                                       dark:border-neutral-600 rounded-md \
                                       hover:bg-neutral-50 \
                                       dark:hover:bg-neutral-700 \
                                       transition-colors disabled:opacity-50 \
                                       disabled:cursor-not-allowed"
                            >
                                {"Cancel"}
                            </button>
                        </div>
                    }

                    // Success message
                    if *save_success {
                        <div class="mt-4 p-4 rounded-md bg-green-50 \
                                    dark:bg-green-900/20 border \
                                    border-green-200 dark:border-green-800">
                            <p class="text-sm text-green-700 \
                                      dark:text-green-400">
                                {"Settings saved successfully"}
                            </p>
                        </div>
                    }

                    // Error message
                    if let Some(error) = &*save_error {
                        <div class="mt-4 p-4 rounded-md bg-red-50 \
                                    dark:bg-red-900/20 border \
                                    border-red-200 dark:border-red-800">
                            <p class="text-sm text-red-700 \
                                      dark:text-red-400">
                                {error}
                            </p>
                        </div>
                    }
                </div>

                // Leave Community Section (non-leaders)
                if !is_leader {
                    <div class="bg-red-50 dark:bg-red-900/10 rounded-lg \
                                border border-red-200 dark:border-red-800 p-6">
                        <h3 class="text-lg font-semibold text-red-800 \
                                   dark:text-red-200 mb-2">
                            {"Leave Community"}
                        </h3>
                        <p class="text-sm text-red-700 dark:text-red-300 mb-4">
                            {"You can leave this community at any time. Your \
                              account balance may be preserved and you can rejoin \
                              if invited again."}
                        </p>

                        <LeaveCommunityButton
                            community_id={props.community_id}
                            community_name={props.community.community.name.clone()}
                            user_role={props.community.user_role}
                        />
                    </div>
                }

                // Danger Zone Section (Leaders only)
                if is_leader {
                    <div class="bg-red-50 dark:bg-red-900/10 rounded-lg \
                                border border-red-200 dark:border-red-800 p-6">
                        <h3 class="text-lg font-semibold text-red-800 \
                                   dark:text-red-200 mb-2">
                            {"Danger Zone"}
                        </h3>
                        <p class="text-sm text-red-700 dark:text-red-300 mb-4">
                            {"Once you delete this community, there is no \
                             going back. All sites, spaces, auctions, and \
                             member data will be permanently removed."}
                        </p>

                        <button
                            onclick={on_open_delete_modal}
                            class="px-4 py-2 text-sm font-medium \
                                   text-red-700 dark:text-red-300 \
                                   bg-white dark:bg-red-900/20 border \
                                   border-red-300 dark:border-red-700 \
                                   rounded-md hover:bg-red-50 \
                                   dark:hover:bg-red-900/30 transition-colors"
                        >
                            {"Delete Community"}
                        </button>
                    </div>
                }
            </div>

            // Delete Confirmation Modal
            if *show_delete_modal {
                <ConfirmationModal
                    title="Delete Community"
                    message="This will permanently delete the community and \
                             all associated data."
                    confirm_text="Delete Community"
                    confirmation_value={community_name.clone()}
                    confirmation_label="the community name"
                    on_confirm={on_delete}
                    on_close={on_close_delete_modal}
                    is_loading={*is_deleting}
                    error_message={(*delete_error).clone().map(AttrValue::from)}
                />
            }
        </div>
    }
}
