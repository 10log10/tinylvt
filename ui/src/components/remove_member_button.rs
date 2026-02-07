use payloads::{CommunityId, Role, requests, responses};
use yew::prelude::*;

use crate::get_api_client;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub member: responses::CommunityMember,
    pub on_success: Callback<()>,
    pub on_close: Callback<()>,
}

/// Modal for confirming member removal. Should be rendered conditionally by
/// parent when removal is requested.
#[function_component]
pub fn RemoveMemberModal(props: &Props) -> Html {
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    let on_confirm = {
        let community_id = props.community_id;
        let member_user_id = props.member.user.user_id;
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let on_success = props.on_success.clone();

        Callback::from(move |_| {
            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let on_success = on_success.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);

                let request = requests::RemoveMember {
                    community_id,
                    member_user_id,
                };

                match get_api_client().remove_member(&request).await {
                    Ok(_) => {
                        on_success.emit(());
                    }
                    Err(e) => {
                        error_message.set(Some(format!(
                            "Failed to remove member: {}",
                            e
                        )));
                    }
                }

                is_submitting.set(false);
            });
        })
    };

    let on_cancel = {
        let on_close = props.on_close.clone();
        let error_message = error_message.clone();
        Callback::from(move |_| {
            error_message.set(None);
            on_close.emit(());
        })
    };

    let on_backdrop_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            on_close.emit(());
        })
    };

    html! {
        <div
            onclick={on_backdrop_click}
            class="fixed inset-0 bg-black bg-opacity-50 flex \
                   items-center justify-center z-50"
        >
            <div
                onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                class="bg-white dark:bg-neutral-800 p-6 rounded-lg \
                       max-w-md w-full mx-4"
            >
                <h3 class="text-lg font-semibold mb-4 text-neutral-900 \
                           dark:text-neutral-100">
                    {"Confirm Member Removal"}
                </h3>
                <p class="mb-4 text-neutral-700 dark:text-neutral-300">
                    {"Remove "}
                    <span class="font-medium">
                        {&props.member.user.username}
                    </span>
                    {" from the community? Their account balance will be \
                      preserved and they can rejoin later."}
                </p>

                {if let Some(error) = (*error_message).clone() {
                    html! {
                        <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 \
                                    border border-red-200 dark:border-red-800 \
                                    rounded">
                            <p class="text-sm text-red-700 dark:text-red-400">
                                {error}
                            </p>
                        </div>
                    }
                } else {
                    html! {}
                }}

                <div class="flex gap-3 justify-end">
                    <button
                        onclick={on_cancel}
                        disabled={*is_submitting}
                        class="px-4 py-2 bg-neutral-200 hover:bg-neutral-300 \
                               dark:bg-neutral-700 dark:hover:bg-neutral-600 \
                               text-neutral-900 dark:text-neutral-100 \
                               rounded disabled:opacity-50"
                    >
                        {"Cancel"}
                    </button>
                    <button
                        onclick={on_confirm}
                        disabled={*is_submitting}
                        class="px-4 py-2 bg-red-600 hover:bg-red-700 \
                               text-white rounded disabled:opacity-50"
                    >
                        {if *is_submitting { "Removing..." } else { "Remove Member" }}
                    </button>
                </div>
            </div>
        </div>
    }
}

/// Check if actor can remove target based on role hierarchy
pub fn can_remove_role(actor_role: &Role, target_role: &Role) -> bool {
    match actor_role {
        Role::Leader => !target_role.is_leader(),
        Role::Coleader => matches!(target_role, Role::Member | Role::Moderator),
        Role::Moderator => matches!(target_role, Role::Member),
        Role::Member => false,
    }
}
