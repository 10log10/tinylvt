use payloads::{
    CommunityId, CurrencySettings, IdempotencyKey, UserId, requests,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::use_members;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub currency: CurrencySettings,
    pub available_credit: Option<Decimal>,
    pub on_success: Callback<()>,
}

#[function_component]
pub fn TransferForm(props: &Props) -> Html {
    let members = use_members(props.community_id);

    // Form state
    let selected_recipient = use_state(|| None::<UserId>);
    let amount_input = use_state(String::new);
    let note_input = use_state(String::new);

    // Submission state
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);
    let success_message = use_state(|| None::<String>);

    // Validation
    let amount_error = use_state(|| None::<String>);

    // Clone props values for use in closures
    let available_credit = props.available_credit;
    let currency = props.currency.clone();

    // Validate amount on change
    let validate_amount = {
        let amount_input = amount_input.clone();
        let amount_error = amount_error.clone();
        let currency = currency.clone();

        move || {
            let input = (*amount_input).clone();
            if input.is_empty() {
                amount_error.set(None);
                return;
            }

            match Decimal::from_str(&input) {
                Ok(amount) => {
                    if amount <= Decimal::ZERO {
                        amount_error.set(Some(
                            "Amount must be greater than 0".to_string(),
                        ));
                    } else if let Some(available) = available_credit {
                        if amount > available {
                            amount_error.set(Some(format!(
                                "Amount exceeds available credit ({})",
                                currency.format_amount(available)
                            )));
                        } else {
                            amount_error.set(None);
                        }
                    } else {
                        amount_error.set(None);
                    }
                }
                Err(_) => {
                    amount_error.set(Some("Invalid amount".to_string()));
                }
            }
        }
    };

    let on_amount_change = {
        let amount_input = amount_input.clone();
        let success_message = success_message.clone();
        let error_message = error_message.clone();
        let validate_amount = validate_amount.clone();

        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            amount_input.set(input.value());
            success_message.set(None);
            error_message.set(None);
            validate_amount();
        })
    };

    let on_recipient_change = {
        let selected_recipient = selected_recipient.clone();
        let success_message = success_message.clone();
        let error_message = error_message.clone();

        Callback::from(move |e: Event| {
            let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
            let value = select.value();
            if value.is_empty() || value == "none" {
                selected_recipient.set(None);
            } else if let Ok(user_id) = Uuid::parse_str(&value) {
                selected_recipient.set(Some(UserId(user_id)));
            }
            success_message.set(None);
            error_message.set(None);
        })
    };

    let on_note_change = {
        let note_input = note_input.clone();
        let success_message = success_message.clone();
        let error_message = error_message.clone();

        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            let value = input.value();
            if value.len() <= 100 {
                note_input.set(value);
            }
            success_message.set(None);
            error_message.set(None);
        })
    };

    let on_submit = {
        let community_id = props.community_id;
        let selected_recipient = selected_recipient.clone();
        let amount_input = amount_input.clone();
        let note_input = note_input.clone();
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let success_message = success_message.clone();
        let on_success = props.on_success.clone();
        let amount_error = amount_error.clone();

        Callback::from(move |_| {
            // Validate
            if selected_recipient.is_none() {
                error_message
                    .set(Some("Please select a recipient".to_string()));
                return;
            }

            if amount_input.is_empty() {
                error_message.set(Some("Please enter an amount".to_string()));
                return;
            }

            if amount_error.is_some() {
                return;
            }

            let recipient_id = selected_recipient.unwrap();
            let amount = match Decimal::from_str(&amount_input) {
                Ok(amt) => amt,
                Err(_) => {
                    error_message.set(Some("Invalid amount".to_string()));
                    return;
                }
            };

            let note = if note_input.is_empty() {
                None
            } else {
                Some((*note_input).clone())
            };

            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let success_message = success_message.clone();
            let amount_input_clear = amount_input.clone();
            let note_input_clear = note_input.clone();
            let selected_recipient_clear = selected_recipient.clone();
            let on_success = on_success.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);
                success_message.set(None);

                let api_client = get_api_client();
                let request = requests::CreateTransfer {
                    community_id,
                    to_user_id: recipient_id,
                    amount,
                    note,
                    idempotency_key: IdempotencyKey(Uuid::new_v4()),
                };

                match api_client.create_transfer(&request).await {
                    Ok(_) => {
                        success_message
                            .set(Some("Transfer successful!".to_string()));
                        // Clear form
                        amount_input_clear.set(String::new());
                        note_input_clear.set(String::new());
                        selected_recipient_clear.set(None);
                        // Notify parent to refetch balances
                        on_success.emit(());
                    }
                    Err(e) => {
                        error_message
                            .set(Some(format!("Transfer failed: {}", e)));
                    }
                }

                is_submitting.set(false);
            });
        })
    };

    let on_cancel = {
        let amount_input = amount_input.clone();
        let note_input = note_input.clone();
        let selected_recipient = selected_recipient.clone();
        let error_message = error_message.clone();
        let success_message = success_message.clone();
        let amount_error = amount_error.clone();

        Callback::from(move |_| {
            amount_input.set(String::new());
            note_input.set(String::new());
            selected_recipient.set(None);
            error_message.set(None);
            success_message.set(None);
            amount_error.set(None);
        })
    };

    let can_submit = selected_recipient.is_some()
        && !amount_input.is_empty()
        && amount_error.is_none()
        && !*is_submitting;

    html! {
        <div class="space-y-4">
            // Success message
            {
                if let Some(msg) = &*success_message {
                    html! {
                        <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded p-3 text-sm text-green-800 dark:text-green-200">
                            {msg}
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // Error message
            {
                if let Some(msg) = &*error_message {
                    html! {
                        <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded p-3 text-sm text-red-800 dark:text-red-200">
                            {msg}
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // Recipient select
            <div>
                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-1">
                    {"Recipient"}
                </label>
                {
                    if members.is_initial_loading() {
                        html! {
                            <div class="h-10 bg-neutral-200 dark:bg-neutral-700 rounded animate-pulse"></div>
                        }
                    } else if let Some(member_list) = members.data.as_ref() {
                        html! {
                            <select
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600 rounded bg-white dark:bg-neutral-800 text-neutral-900 dark:text-neutral-100"
                                value={selected_recipient.as_ref().map(|id| id.0.to_string()).unwrap_or_else(|| "none".to_string())}
                                onchange={on_recipient_change}
                                disabled={*is_submitting}
                            >
                                <option value="none" selected={selected_recipient.is_none()}>{"Select a member..."}</option>
                                {
                                    member_list.iter().map(|member| {
                                        let display_name = member.user.display_name.as_ref()
                                            .unwrap_or(&member.user.username);
                                        html! {
                                            <option
                                                value={member.user.user_id.0.to_string()}
                                            >
                                                {display_name}
                                            </option>
                                        }
                                    }).collect::<Html>()
                                }
                            </select>
                        }
                    } else {
                        html! {
                            <div class="text-red-600 dark:text-red-400 text-sm">
                                {"Error loading members"}
                            </div>
                        }
                    }
                }
            </div>

            // Amount input
            <div>
                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-1">
                    {"Amount"}
                </label>
                <div class="relative">
                    <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                        <span class="text-neutral-500 dark:text-neutral-400">
                            {&props.currency.symbol}
                        </span>
                    </div>
                    <input
                        type="text"
                        class={classes!(
                            "w-full", "pl-8", "pr-3", "py-2", "border", "rounded",
                            "bg-white", "dark:bg-neutral-800",
                            "text-neutral-900", "dark:text-neutral-100",
                            if amount_error.is_some() {
                                classes!("border-red-300", "dark:border-red-600")
                            } else {
                                classes!("border-neutral-300", "dark:border-neutral-600")
                            }
                        )}
                        placeholder="0.00"
                        value={(*amount_input).clone()}
                        onchange={on_amount_change}
                        disabled={*is_submitting}
                    />
                </div>
                {
                    if let Some(err) = &*amount_error {
                        html! {
                            <p class="mt-1 text-sm text-red-600 dark:text-red-400">
                                {err}
                            </p>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>

            // Note input
            <div>
                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-1">
                    {"Note (optional)"}
                </label>
                <input
                    type="text"
                    class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600 rounded bg-white dark:bg-neutral-800 text-neutral-900 dark:text-neutral-100"
                    placeholder="What is this transfer for?"
                    value={(*note_input).clone()}
                    onchange={on_note_change}
                    maxlength="100"
                    disabled={*is_submitting}
                />
                <p class="mt-1 text-xs text-neutral-500 dark:text-neutral-400">
                    {format!("{}/100 characters", note_input.len())}
                </p>
            </div>

            // Actions
            <div class="flex gap-3">
                <button
                    onclick={on_submit}
                    disabled={!can_submit}
                    class="flex justify-center py-2 px-4 border border-transparent
                           rounded-md shadow-sm text-sm font-medium text-white
                           bg-neutral-900 hover:bg-neutral-800
                           dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200
                           focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                           disabled:opacity-50 disabled:cursor-not-allowed
                           transition-colors duration-200"
                >
                    {if *is_submitting { "Sending..." } else { "Send Transfer" }}
                </button>
                <button
                    onclick={on_cancel}
                    disabled={*is_submitting}
                    class="py-2 px-4 border border-neutral-300 dark:border-neutral-600
                           rounded-md shadow-sm text-sm font-medium
                           text-neutral-700 dark:text-neutral-300
                           bg-white dark:bg-neutral-800
                           hover:bg-neutral-50 dark:hover:bg-neutral-700
                           focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                           disabled:opacity-50 disabled:cursor-not-allowed
                           transition-colors duration-200"
                >
                    {"Clear"}
                </button>
            </div>
        </div>
    }
}
