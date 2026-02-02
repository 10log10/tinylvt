use payloads::{CommunityId, requests};
use yew::prelude::*;

use crate::components::Modal;
use crate::get_api_client;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub on_success: Callback<()>,
}

#[function_component]
pub fn ResetBalancesButton(props: &Props) -> Html {
    let show_modal = use_state(|| false);
    let note_input = use_state(String::new);
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);
    let success_message = use_state(|| None::<String>);

    let open_modal = {
        let show_modal = show_modal.clone();
        Callback::from(move |_| show_modal.set(true))
    };

    let close_modal = {
        let show_modal = show_modal.clone();
        let note_input = note_input.clone();
        let error_message = error_message.clone();
        let success_message = success_message.clone();
        Callback::from(move |_| {
            show_modal.set(false);
            note_input.set(String::new());
            error_message.set(None);
            success_message.set(None);
        })
    };

    let on_note_change = {
        let note_input = note_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
            note_input.set(input.value());
        })
    };

    let on_submit = {
        let community_id = props.community_id;
        let note_input = note_input.clone();
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let success_message = success_message.clone();
        let on_success = props.on_success.clone();
        let show_modal = show_modal.clone();

        Callback::from(move |_| {
            let community_id = community_id;
            let note = (*note_input).clone();
            let note = if note.is_empty() { None } else { Some(note) };
            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let success_message = success_message.clone();
            let on_success = on_success.clone();
            let show_modal = show_modal.clone();

            wasm_bindgen_futures::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);
                success_message.set(None);

                let api_client = get_api_client();
                let result = api_client
                    .reset_all_balances(&requests::ResetAllBalances {
                        community_id,
                        note,
                    })
                    .await;

                is_submitting.set(false);

                match result {
                    Ok(response) => {
                        success_message.set(Some(format!(
                            "Reset {} account(s). {} transferred to treasury.",
                            response.accounts_reset, response.total_transferred
                        )));
                        on_success.emit(());
                        // Close modal after 2 seconds
                        let show_modal_timeout = show_modal.clone();
                        gloo_timers::callback::Timeout::new(2000, move || {
                            show_modal_timeout.set(false);
                        })
                        .forget();
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Error: {}", e)));
                    }
                }
            });
        })
    };

    html! {
        <>
            <button
                onclick={open_modal}
                class="px-4 py-2 bg-red-600 dark:bg-red-700 text-white rounded
                       hover:bg-red-700 dark:hover:bg-red-600 transition"
            >
                {"Reset All Balances"}
            </button>

            if *show_modal {
                <Modal on_close={close_modal.clone()}>
                    <h3 class="text-lg font-semibold text-neutral-900
                              dark:text-neutral-100 mb-4">
                        {"Confirm Balance Reset"}
                    </h3>

                    <div class="mb-4">
                        <p class="text-sm text-neutral-700 dark:text-neutral-300 mb-2">
                            {"This will transfer all member balances to the treasury. \
                             All member accounts will be locked during this operation."}
                        </p>
                        <p class="text-sm text-red-600 dark:text-red-400 font-medium">
                            {"This operation cannot be performed during active auctions."}
                        </p>
                    </div>

                    <div class="mb-4">
                        <label
                            for="reset-note"
                            class="block text-sm font-medium text-neutral-700
                                  dark:text-neutral-300 mb-2"
                        >
                            {"Note (optional)"}
                        </label>
                        <textarea
                            id="reset-note"
                            value={(*note_input).clone()}
                            oninput={on_note_change}
                            disabled={*is_submitting}
                            rows="3"
                            class="w-full px-3 py-2 border border-neutral-300
                                  dark:border-neutral-600 rounded
                                  bg-white dark:bg-neutral-700
                                  text-neutral-900 dark:text-neutral-100
                                  focus:outline-none focus:ring-2
                                  focus:ring-blue-500 dark:focus:ring-blue-400
                                  disabled:bg-neutral-100
                                  dark:disabled:bg-neutral-800
                                  disabled:cursor-not-allowed"
                            placeholder="e.g., Test auction reset"
                        />
                    </div>

                    if let Some(error) = (*error_message).clone() {
                        <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20
                                   border border-red-200 dark:border-red-800
                                   rounded">
                            <p class="text-sm text-red-800 dark:text-red-200">
                                {error}
                            </p>
                        </div>
                    }

                    if let Some(success) = (*success_message).clone() {
                        <div class="mb-4 p-3 bg-green-50 dark:bg-green-900/20
                                   border border-green-200 dark:border-green-800
                                   rounded">
                            <p class="text-sm text-green-800 dark:text-green-200">
                                {success}
                            </p>
                        </div>
                    }

                    <div class="flex gap-3 justify-end">
                        <button
                            onclick={close_modal.reform(|_| ())}
                            disabled={*is_submitting}
                            class="px-4 py-2 border border-neutral-300
                                  dark:border-neutral-600 rounded
                                  text-neutral-700 dark:text-neutral-300
                                  hover:bg-neutral-50 dark:hover:bg-neutral-700
                                  transition disabled:cursor-not-allowed
                                  disabled:opacity-50"
                        >
                            {"Cancel"}
                        </button>
                        <button
                            onclick={on_submit}
                            disabled={*is_submitting}
                            class="px-4 py-2 bg-red-600 dark:bg-red-700
                                  text-white rounded hover:bg-red-700
                                  dark:hover:bg-red-600 transition
                                  disabled:bg-red-400 dark:disabled:bg-red-500
                                  disabled:cursor-not-allowed"
                        >
                            if *is_submitting {
                                {"Resetting..."}
                            } else {
                                {"Confirm Reset"}
                            }
                        </button>
                    </div>
                </Modal>
            }
        </>
    }
}
