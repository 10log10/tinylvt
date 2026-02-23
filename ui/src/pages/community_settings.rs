use payloads::{CommunityId, requests, responses::CommunityWithRole};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::{
    ActiveTab, CommunityPageWrapper, CommunityTabHeader, ConfirmationModal,
    CurrencyConfigEditor, LeaveCommunityButton, MarkdownEditor, MarkdownText,
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

    // Details editing state
    let edited_name = use_state(|| props.community.community.name.clone());
    let edited_description = use_state(|| {
        props
            .community
            .community
            .description
            .clone()
            .unwrap_or_default()
    });
    let is_saving_details = use_state(|| false);
    let details_error = use_state(|| None::<String>);
    let details_success = use_state(|| false);

    // Delete modal state
    let show_delete_modal = use_state(|| false);
    let is_deleting = use_state(|| false);
    let delete_error = use_state(|| None::<String>);

    let community_name = props.community.name.clone();

    // Check if details have changes
    let details_have_changes = {
        let name_changed = *edited_name != props.community.community.name;
        let original_desc = props
            .community
            .community
            .description
            .clone()
            .unwrap_or_default();
        let desc_changed = *edited_description != original_desc;
        name_changed || desc_changed
    };

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

    // Details editing handlers
    let on_name_change = {
        let edited_name = edited_name.clone();
        let details_success = details_success.clone();
        let details_error = details_error.clone();

        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            edited_name.set(input.value());
            details_success.set(false);
            details_error.set(None);
        })
    };

    let on_description_change = {
        let edited_description = edited_description.clone();
        let details_success = details_success.clone();
        let details_error = details_error.clone();

        Callback::from(move |new_desc: String| {
            edited_description.set(new_desc);
            details_success.set(false);
            details_error.set(None);
        })
    };

    let on_save_details = {
        let edited_name = edited_name.clone();
        let edited_description = edited_description.clone();
        let is_saving_details = is_saving_details.clone();
        let details_error = details_error.clone();
        let details_success = details_success.clone();
        let community_id = props.community_id;
        let refetch_communities = communities_hook.refetch.clone();

        Callback::from(move |_: MouseEvent| {
            let name = (*edited_name).clone();
            let description = (*edited_description).clone();
            let is_saving_details = is_saving_details.clone();
            let details_error = details_error.clone();
            let details_success = details_success.clone();
            let refetch_communities = refetch_communities.clone();

            is_saving_details.set(true);
            details_error.set(None);
            details_success.set(false);

            wasm_bindgen_futures::spawn_local(async move {
                let client = get_api_client();
                let details = requests::UpdateCommunityDetails {
                    community_id,
                    name,
                    description: if description.trim().is_empty() {
                        None
                    } else {
                        Some(description)
                    },
                };

                match client.update_community_details(&details).await {
                    Ok(_) => {
                        details_success.set(true);
                        refetch_communities.emit(());
                    }
                    Err(e) => {
                        details_error.set(Some(e.to_string()));
                    }
                }
                is_saving_details.set(false);
            });
        })
    };

    let on_cancel_details = {
        let edited_name = edited_name.clone();
        let edited_description = edited_description.clone();
        let details_success = details_success.clone();
        let details_error = details_error.clone();
        let original_name = props.community.community.name.clone();
        let original_desc = props
            .community
            .community
            .description
            .clone()
            .unwrap_or_default();

        Callback::from(move |_| {
            edited_name.set(original_name.clone());
            edited_description.set(original_desc.clone());
            details_success.set(false);
            details_error.set(None);
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

            <div class="py-6 space-y-8">
                // Community Details Section (coleader+ can edit)
                <div>
                    <h2 class="text-xl font-semibold text-neutral-900 \
                               dark:text-neutral-100 mb-6">
                        {"Community Details"}
                    </h2>

                    {if is_coleader_plus {
                        // Editable form for coleader+
                        html! {
                            <div class="space-y-4">
                                // Name field
                                <div>
                                    <label
                                        for="community-name"
                                        class="block text-sm font-medium \
                                               text-neutral-700 dark:text-neutral-300 mb-1"
                                    >
                                        {"Name"}
                                    </label>
                                    <input
                                        type="text"
                                        id="community-name"
                                        value={(*edited_name).clone()}
                                        onchange={on_name_change}
                                        disabled={*is_saving_details}
                                        class="w-full max-w-lg px-3 py-2 border border-neutral-300 \
                                               dark:border-neutral-600 rounded-md \
                                               bg-white dark:bg-neutral-700 \
                                               text-neutral-900 dark:text-neutral-100 \
                                               focus:outline-none focus:ring-1 \
                                               focus:ring-neutral-500 \
                                               disabled:opacity-50 disabled:cursor-not-allowed"
                                    />
                                </div>

                                // Description field (editor)
                                <div>
                                    <label class="block text-sm font-medium \
                                                  text-neutral-700 dark:text-neutral-300 mb-1">
                                        {"Description"}
                                    </label>
                                    <MarkdownEditor
                                        text={(*edited_description).clone()}
                                        on_change={on_description_change}
                                        community_id={props.community_id}
                                        disabled={*is_saving_details}
                                    />
                                </div>
                            </div>
                        }
                    } else {
                        // Read-only view for non-coleaders
                        html! {
                            <div class="space-y-4">
                                // Name (read-only)
                                <div>
                                    <label class="block text-sm font-medium \
                                                  text-neutral-700 dark:text-neutral-300 mb-1">
                                        {"Name"}
                                    </label>
                                    <p class="text-neutral-900 dark:text-neutral-100">
                                        {&props.community.community.name}
                                    </p>
                                </div>

                                // Description (read-only)
                                <div>
                                    <label class="block text-sm font-medium \
                                                  text-neutral-700 dark:text-neutral-300 mb-1">
                                        {"Description"}
                                    </label>
                                    {if let Some(desc) = &props.community.community.description {
                                        html! {
                                            <div class="max-h-48 overflow-y-auto border \
                                                        border-neutral-200 dark:border-neutral-700 \
                                                        rounded-md p-3">
                                                <div class="prose prose-sm dark:prose-invert max-w-none">
                                                    <MarkdownText text={desc.clone()} />
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <p class="text-neutral-500 dark:text-neutral-400 italic">
                                                {"No description"}
                                            </p>
                                        }
                                    }}
                                </div>
                            </div>
                        }
                    }}

                    // Save/Cancel buttons (only for coleaders+)
                    if is_coleader_plus {
                        <div class="mt-6 flex gap-3">
                            <button
                                onclick={on_save_details}
                                disabled={!details_have_changes || *is_saving_details}
                                class="px-4 py-2 text-sm font-medium text-white \
                                       bg-neutral-900 dark:bg-neutral-100 \
                                       dark:text-neutral-900 rounded-md \
                                       hover:bg-neutral-700 \
                                       dark:hover:bg-neutral-300 \
                                       transition-colors disabled:opacity-50 \
                                       disabled:cursor-not-allowed"
                            >
                                {if *is_saving_details { "Saving..." } else { "Save Details" }}
                            </button>
                            <button
                                onclick={on_cancel_details}
                                disabled={!details_have_changes || *is_saving_details}
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
                    if *details_success {
                        <div class="mt-4 p-4 rounded-md bg-green-50 \
                                    dark:bg-green-900/20 border \
                                    border-green-200 dark:border-green-800">
                            <p class="text-sm text-green-700 \
                                      dark:text-green-400">
                                {"Details saved successfully"}
                            </p>
                        </div>
                    }

                    // Error message
                    if let Some(error) = &*details_error {
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
