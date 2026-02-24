use payloads::{CommunityId, Role, requests};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::State;
use crate::components::ConfirmationModal;
use crate::hooks::use_push_route;
use crate::{Route, get_api_client};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub community_name: AttrValue,
    pub user_role: Role,
}

#[function_component]
pub fn LeaveCommunityButton(props: &Props) -> Html {
    let push_route = use_push_route();
    let (_state, dispatch) = use_store::<State>();
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);
    let show_confirm = use_state(|| false);

    // Leader cannot leave
    if props.user_role.is_leader() {
        return html! {
            <div class="p-4 bg-neutral-100 dark:bg-neutral-800 rounded">
                <p class="text-sm text-neutral-600 dark:text-neutral-400 italic">
                    {"Leaders must transfer leadership before leaving the \
                      community."}
                </p>
            </div>
        };
    }

    let onclick = {
        let show_confirm = show_confirm.clone();
        Callback::from(move |_| {
            show_confirm.set(true);
        })
    };

    let on_confirm = {
        let community_id = props.community_id;
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let push_route = push_route.clone();
        let dispatch = dispatch.clone();

        Callback::from(move |_| {
            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let push_route = push_route.clone();
            let dispatch = dispatch.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);

                let request = requests::LeaveCommunity { community_id };

                match get_api_client().leave_community(&request).await {
                    Ok(_) => {
                        // Clear cached communities so they'll be refetched
                        dispatch.reduce_mut(|s| s.clear_communities());
                        // Navigate to home page after leaving
                        push_route.emit(Route::Home);
                    }
                    Err(e) => {
                        error_message.set(Some(format!(
                            "Failed to leave community: {}",
                            e
                        )));
                        is_submitting.set(false);
                    }
                }
            });
        })
    };

    let on_close = {
        let show_confirm = show_confirm.clone();
        let error_message = error_message.clone();
        Callback::from(move |_| {
            show_confirm.set(false);
            error_message.set(None);
        })
    };

    html! {
        <>
            <button
                onclick={onclick}
                disabled={*is_submitting}
                class="px-4 py-2 bg-red-600 hover:bg-red-700 text-white \
                       rounded disabled:opacity-50 disabled:cursor-not-allowed"
            >
                {"Leave Community"}
            </button>

            {if *show_confirm {
                html! {
                    <ConfirmationModal
                        title="Leave Community"
                        message="Your account balance will be preserved and you \
                                 can rejoin later if invited again."
                        confirm_text="Leave Community"
                        confirmation_value={props.community_name.clone()}
                        confirmation_label="the community name"
                        on_confirm={on_confirm}
                        on_close={on_close}
                        is_loading={*is_submitting}
                        error_message={(*error_message).clone().map(AttrValue::from)}
                        is_irreversible={false}
                    />
                }
            } else {
                html! {}
            }}
        </>
    }
}
