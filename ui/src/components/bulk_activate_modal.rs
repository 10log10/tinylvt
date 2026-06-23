use payloads::{CommunityId, requests, responses::BulkActivateMembersResult};
use yew::prelude::*;

use crate::components::Modal;
use crate::get_api_client;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub on_close: Callback<()>,
    /// Called after a successful activation so the parent can refetch the
    /// member list.
    pub on_success: Callback<()>,
}

#[function_component]
pub fn BulkActivateModal(props: &Props) -> Html {
    let input = use_state(String::new);
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);
    let result = use_state(|| None::<BulkActivateMembersResult>);

    let identifiers = parse_identifiers(&input);

    let on_input = {
        let input = input.clone();
        let result = result.clone();
        Callback::from(move |e: InputEvent| {
            let textarea: web_sys::HtmlTextAreaElement =
                e.target_unchecked_into();
            input.set(textarea.value());
            // Clear a prior result once the user edits the list again.
            result.set(None);
        })
    };

    let on_submit = {
        let community_id = props.community_id;
        let identifiers = identifiers.clone();
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let result = result.clone();
        let on_success = props.on_success.clone();

        Callback::from(move |_| {
            let identifiers = identifiers.clone();
            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let result = result.clone();
            let on_success = on_success.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);

                let request = requests::BulkActivateMembers {
                    community_id,
                    identifiers,
                };

                match get_api_client().bulk_activate_members(&request).await {
                    Ok(summary) => {
                        // Refetch the list so updated statuses show, then
                        // surface the summary in place rather than closing.
                        on_success.emit(());
                        result.set(Some(summary));
                    }
                    Err(e) => {
                        error_message.set(Some(format!(
                            "Failed to activate members: {}",
                            e
                        )));
                    }
                }

                is_submitting.set(false);
            });
        })
    };

    let over_limit =
        identifiers.len() > requests::MAX_BULK_ACTIVATE_IDENTIFIERS;
    let can_submit = !identifiers.is_empty() && !over_limit && !*is_submitting;

    html! {
        <Modal on_close={props.on_close.clone()} max_width="max-w-lg">
            <h2 class="text-xl font-semibold text-neutral-900 \
                       dark:text-neutral-100 mb-4">
                {"Bulk Activate Members"}
            </h2>

            <p class="text-sm text-neutral-600 dark:text-neutral-400 mb-4">
                {"Paste a list of emails or usernames separated by commas, \
                  tabs, or new lines. Each member found is set to active. \
                  Members not in the list are left unchanged."}
            </p>

            if let Some(msg) = &*error_message {
                <div class="bg-red-50 dark:bg-red-900/20 border \
                            border-red-200 dark:border-red-800 rounded p-3 \
                            text-sm text-red-800 dark:text-red-200 mb-4">
                    {msg}
                </div>
            }

            if let Some(summary) = &*result {
                <BulkActivateSummary summary={summary.clone()} />
            }

            <textarea
                rows="6"
                placeholder="alice@example.com, bob, charlie@example.com"
                value={(*input).clone()}
                oninput={on_input}
                disabled={*is_submitting}
                class="w-full px-3 py-2 border border-neutral-300 \
                       dark:border-neutral-600 rounded bg-white \
                       dark:bg-neutral-800 text-neutral-900 \
                       dark:text-neutral-100 text-sm \
                       focus:outline-none focus:ring-2 \
                       focus:ring-neutral-500 disabled:opacity-50"
            />

            if over_limit {
                <p class="mt-1 text-xs text-red-600 dark:text-red-400">
                    {format!(
                        "{} identifier(s) — over the limit of {}",
                        identifiers.len(),
                        requests::MAX_BULK_ACTIVATE_IDENTIFIERS
                    )}
                </p>
            } else {
                <p class="mt-1 text-xs text-neutral-500 \
                          dark:text-neutral-400">
                    {format!("{} identifier(s)", identifiers.len())}
                </p>
            }

            <div class="flex gap-3 mt-6">
                <button
                    onclick={on_submit}
                    disabled={!can_submit}
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
                    {if *is_submitting { "Activating..." }
                     else { "Activate" }}
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
                    {"Close"}
                </button>
            </div>
        </Modal>
    }
}

#[derive(Properties, PartialEq)]
struct SummaryProps {
    summary: BulkActivateMembersResult,
}

#[function_component]
fn BulkActivateSummary(props: &SummaryProps) -> Html {
    let summary = &props.summary;
    html! {
        <div class="bg-green-50 dark:bg-green-900/20 border \
                    border-green-200 dark:border-green-800 rounded p-3 mb-4">
            <p class="text-sm text-green-800 dark:text-green-200">
                {format!(
                    "Set {} member(s) to active.",
                    summary.activated_count
                )}
            </p>
            if !summary.unmatched.is_empty() {
                <p class="mt-2 text-sm text-green-800 dark:text-green-200">
                    {format!(
                        "{} matched no member:",
                        pluralize_entries(summary.unmatched.len())
                    )}
                </p>
                <ul class="mt-1 list-disc list-inside text-sm \
                           text-green-700 dark:text-green-300">
                    {summary.unmatched.iter().map(|id| html! {
                        <li>{id}</li>
                    }).collect::<Html>()}
                </ul>
            }
        </div>
    }
}

fn pluralize_entries(n: usize) -> String {
    if n == 1 {
        "1 entry".to_string()
    } else {
        format!("{} entries", n)
    }
}

/// Split the textarea contents on commas, tabs, and new lines, trimming each
/// entry and dropping empties. The backend re-trims and resolves these, so
/// this only needs to produce a clean token list.
fn parse_identifiers(raw: &str) -> Vec<String> {
    raw.split([',', '\t', '\n', '\r'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}
