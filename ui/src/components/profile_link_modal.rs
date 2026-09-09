use payloads::{CommunityId, requests};
use yew::prelude::*;

use crate::components::Modal;
use crate::get_api_client;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    /// The member's current link, prefilled for editing.
    #[prop_or_default]
    pub current_link: Option<String>,
    pub on_close: Callback<()>,
    pub on_success: Callback<()>,
}

/// Modal for a member to set or clear their own profile link (a URL or
/// social handle shown beside their username in the member list). Saving an
/// empty field clears the link.
#[function_component]
pub fn ProfileLinkModal(props: &Props) -> Html {
    let link_input =
        use_state(|| props.current_link.clone().unwrap_or_default());
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    let too_long = link_input.trim().len() > payloads::MAX_PROFILE_LINK_LENGTH;

    let on_input = {
        let link_input = link_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            link_input.set(input.value());
        })
    };

    let on_submit = {
        let community_id = props.community_id;
        let link_input = link_input.clone();
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let on_success = props.on_success.clone();

        Callback::from(move |_: MouseEvent| {
            let trimmed = link_input.trim().to_string();
            let profile_link = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };

            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let on_success = on_success.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);

                let request = requests::SetProfileLink {
                    community_id,
                    profile_link,
                };

                match get_api_client().set_profile_link(&request).await {
                    Ok(()) => on_success.emit(()),
                    Err(e) => {
                        error_message.set(Some(format!(
                            "Failed to update profile link: {}",
                            e
                        )));
                    }
                }

                is_submitting.set(false);
            });
        })
    };

    html! {
        <Modal on_close={props.on_close.clone()} max_width="max-w-md">
            <h2 class="text-xl font-semibold text-neutral-900 \
                       dark:text-neutral-100 mb-4">
                {"Profile Link"}
            </h2>

            <p class="text-sm text-neutral-600 dark:text-neutral-400 mb-4">
                {"A URL or social handle shown beside your name in the \
                  member list. Leave empty to remove it."}
            </p>

            if let Some(msg) = &*error_message {
                <div class="bg-red-50 dark:bg-red-900/20 border \
                            border-red-200 dark:border-red-800 rounded p-3 \
                            text-sm text-red-800 dark:text-red-200 mb-4">
                    {msg}
                </div>
            }

            <input
                type="text"
                placeholder="https://example.com or @handle"
                value={(*link_input).clone()}
                oninput={on_input}
                disabled={*is_submitting}
                class="w-full px-3 py-2 text-sm border border-neutral-300 \
                       dark:border-neutral-600 rounded-md bg-white \
                       dark:bg-neutral-700 text-neutral-900 \
                       dark:text-neutral-100 placeholder-neutral-400 \
                       dark:placeholder-neutral-500 focus:outline-none \
                       focus:ring-2 focus:ring-neutral-500 \
                       disabled:opacity-50 disabled:cursor-not-allowed"
            />
            if too_long {
                <p class="mt-1 text-sm text-red-600 dark:text-red-400">
                    {format!(
                        "Must be at most {} characters",
                        payloads::MAX_PROFILE_LINK_LENGTH
                    )}
                </p>
            }

            <div class="flex gap-3 mt-6">
                <button
                    onclick={on_submit}
                    disabled={too_long || *is_submitting}
                    class="flex-1 justify-center py-2 px-4 border \
                           border-transparent rounded-md shadow-sm text-sm \
                           font-medium text-white bg-neutral-900 \
                           hover:bg-neutral-800 dark:bg-neutral-100 \
                           dark:text-neutral-900 dark:hover:bg-neutral-200 \
                           focus:outline-none focus:ring-2 \
                           focus:ring-offset-2 focus:ring-neutral-500 \
                           disabled:opacity-50 disabled:cursor-not-allowed \
                           transition-colors duration-200"
                >
                    {if *is_submitting { "Saving..." } else { "Save" }}
                </button>
                <button
                    onclick={props.on_close.reform(|_| ())}
                    disabled={*is_submitting}
                    class="flex-1 py-2 px-4 border border-neutral-300 \
                           dark:border-neutral-600 rounded-md shadow-sm \
                           text-sm font-medium text-neutral-700 \
                           dark:text-neutral-300 bg-white \
                           dark:bg-neutral-800 hover:bg-neutral-50 \
                           dark:hover:bg-neutral-700 focus:outline-none \
                           focus:ring-2 focus:ring-offset-2 \
                           focus:ring-neutral-500 disabled:opacity-50 \
                           disabled:cursor-not-allowed transition-colors \
                           duration-200"
                >
                    {"Cancel"}
                </button>
            </div>
        </Modal>
    }
}
