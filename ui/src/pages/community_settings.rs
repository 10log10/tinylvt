use payloads::{CommunityId, Role, responses::CommunityWithRole};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::{
    ActiveTab, CommunityPageWrapper, CommunityTabHeader, ConfirmationModal,
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
                <CommunitySettingsContent community={community} community_id={community_id} />
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

    // Modal and deletion state
    let show_delete_modal = use_state(|| false);
    let is_deleting = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    let community_name = props.community.name.clone();
    let is_leader = props.community.user_role == Role::Leader;

    let on_open_modal = {
        let show_delete_modal = show_delete_modal.clone();
        let error_message = error_message.clone();
        Callback::from(move |_| {
            error_message.set(None);
            show_delete_modal.set(true);
        })
    };

    let on_close_modal = {
        let show_delete_modal = show_delete_modal.clone();
        Callback::from(move |()| {
            show_delete_modal.set(false);
        })
    };

    let on_delete = {
        let is_deleting = is_deleting.clone();
        let error_message = error_message.clone();
        let navigator = navigator.clone();
        let community_id = props.community_id;
        let refetch_communities = communities_hook.refetch.clone();

        Callback::from(move |()| {
            let is_deleting = is_deleting.clone();
            let error_message = error_message.clone();
            let navigator = navigator.clone();
            let refetch_communities = refetch_communities.clone();

            is_deleting.set(true);
            error_message.set(None);

            wasm_bindgen_futures::spawn_local(async move {
                let client = get_api_client();
                match client.delete_community(&community_id).await {
                    Ok(_) => {
                        refetch_communities.emit(());
                        navigator.push(&Route::Communities);
                    }
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                        is_deleting.set(false);
                    }
                }
            });
        })
    };

    // Non-leaders should not see this page
    if !is_leader {
        return html! {
            <div>
                <CommunityTabHeader
                    community={props.community.clone()}
                    active_tab={ActiveTab::Sites}
                />
                <div class="py-6">
                    <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                        <p class="text-sm text-red-700 dark:text-red-400">
                            {"You do not have permission to access community settings."}
                        </p>
                    </div>
                </div>
            </div>
        };
    }

    html! {
        <div>
            <CommunityTabHeader
                community={props.community.clone()}
                active_tab={ActiveTab::Sites}
            />

            <div class="py-6 max-w-2xl">
                <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100 mb-6">
                    {"Community Settings"}
                </h2>

                // Danger Zone Section
                <div class="bg-red-50 dark:bg-red-900/10 rounded-lg border border-red-200 dark:border-red-800 p-6">
                    <h3 class="text-lg font-semibold text-red-800 dark:text-red-200 mb-2">
                        {"Danger Zone"}
                    </h3>
                    <p class="text-sm text-red-700 dark:text-red-300 mb-4">
                        {"Once you delete this community, there is no going back. All sites, spaces, auctions, and member data will be permanently removed."}
                    </p>

                    <button
                        onclick={on_open_modal}
                        class="px-4 py-2 text-sm font-medium text-red-700 dark:text-red-300
                               bg-white dark:bg-red-900/20 border border-red-300 dark:border-red-700
                               rounded-md hover:bg-red-50 dark:hover:bg-red-900/30
                               transition-colors"
                    >
                        {"Delete Community"}
                    </button>
                </div>
            </div>

            // Delete Confirmation Modal
            if *show_delete_modal {
                <ConfirmationModal
                    title="Delete Community"
                    message="This will permanently delete the community and all associated data."
                    confirm_text="Delete Community"
                    confirmation_value={community_name.clone()}
                    confirmation_label="the community name"
                    on_confirm={on_delete}
                    on_close={on_close_modal}
                    is_loading={*is_deleting}
                    error_message={(*error_message).clone().map(AttrValue::from)}
                />
            }
        </div>
    }
}
