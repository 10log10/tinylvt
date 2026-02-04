use payloads::{CommunityId, UserId, requests};
use yew::prelude::*;

use crate::get_api_client;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub member_user_id: UserId,
    pub current_status: bool,
    pub on_success: Callback<()>,
    pub disabled: bool,
}

#[function_component]
pub fn ActiveStatusToggle(props: &Props) -> Html {
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    let on_toggle = {
        let community_id = props.community_id;
        let member_user_id = props.member_user_id;
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let on_success = props.on_success.clone();

        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            let new_status = input.checked();

            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let on_success = on_success.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);

                let request = requests::UpdateMemberActiveStatus {
                    community_id,
                    member_user_id,
                    is_active: new_status,
                };

                match get_api_client()
                    .update_member_active_status(&request)
                    .await
                {
                    Ok(_) => {
                        on_success.emit(());
                    }
                    Err(e) => {
                        error_message.set(Some(format!(
                            "Failed to update active status: {}",
                            e
                        )));
                    }
                }

                is_submitting.set(false);
            });
        })
    };

    html! {
        <div class="flex items-center gap-2">
            <label class="relative inline-flex items-center cursor-pointer">
                <input
                    type="checkbox"
                    checked={props.current_status}
                    onchange={on_toggle}
                    disabled={props.disabled || *is_submitting}
                    class="sr-only peer"
                />
                <div class="w-11 h-6 bg-neutral-200 peer-focus:outline-none \
                            peer-focus:ring-2 peer-focus:ring-neutral-300 \
                            dark:peer-focus:ring-neutral-600 rounded-full peer \
                            dark:bg-neutral-700 peer-checked:after:translate-x-full \
                            after:content-[''] \
                            after:absolute after:top-[2px] after:left-[2px] \
                            after:bg-white after:border-neutral-300 after:border \
                            after:rounded-full after:h-5 after:w-5 \
                            after:transition-all \
                            dark:after:bg-neutral-900 dark:after:border-neutral-500 \
                            peer-checked:bg-neutral-900 \
                            dark:peer-checked:bg-neutral-100 \
                            peer-disabled:opacity-50 peer-disabled:cursor-not-allowed">
                </div>
                <span class="ml-3 text-sm font-medium text-neutral-700 \
                             dark:text-neutral-300">
                    {if props.current_status { "Active" } else { "Inactive" }}
                </span>
            </label>
            {if let Some(error) = (*error_message).clone() {
                html! {
                    <span class="text-xs text-red-600 dark:text-red-400">
                        {error}
                    </span>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
