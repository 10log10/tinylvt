use payloads::{
    CommunityId, CurrencyMode, IdempotencyKey, TreasuryRecipient, UserId,
    requests, responses::CommunityWithRole,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;
use yew::prelude::*;

use crate::components::transfer_form::MemberSelect;
use crate::get_api_client;

#[derive(PartialEq, Clone, Copy)]
enum RecipientType {
    AllActiveMembers,
    SingleMember,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
    pub community: CommunityWithRole,
    pub on_success: Callback<()>,
}

#[function_component]
pub fn TreasuryCreditForm(props: &Props) -> Html {
    // Determine default recipient type based on currency mode
    // Default to AllActiveMembers unless it's prepaid or deferred payment
    let currency_mode = props.community.community.currency.mode_config.mode();
    let default_recipient_type = match currency_mode {
        CurrencyMode::PrepaidCredits | CurrencyMode::DeferredPayment => {
            RecipientType::SingleMember
        }
        CurrencyMode::PointsAllocation | CurrencyMode::DistributedClearing => {
            RecipientType::AllActiveMembers
        }
    };

    // Form state
    let recipient_type = use_state(|| default_recipient_type);
    let selected_member = use_state(|| None::<UserId>);
    let amount_input = use_state(String::new);
    let note_input = use_state(String::new);

    // Submission state
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);
    let success_message = use_state(|| None::<String>);

    // Validation
    let amount_error = use_state(|| None::<String>);

    // Determine entry type label based on currency mode
    let entry_type_label = match currency_mode {
        CurrencyMode::PointsAllocation => "Allowance",
        CurrencyMode::DistributedClearing => "Distribution Correction",
        CurrencyMode::DeferredPayment => "Debt Settlement",
        CurrencyMode::PrepaidCredits => "Credit Purchase",
    };

    // Validate amount on change
    let validate_amount = {
        let amount_input = amount_input.clone();
        let amount_error = amount_error.clone();

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

    let on_recipient_type_change = {
        let recipient_type = recipient_type.clone();
        let success_message = success_message.clone();
        let error_message = error_message.clone();

        Callback::from(move |e: Event| {
            let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
            let value = select.value();
            match value.as_str() {
                "single" => recipient_type.set(RecipientType::SingleMember),
                "all" => recipient_type.set(RecipientType::AllActiveMembers),
                _ => {}
            }
            success_message.set(None);
            error_message.set(None);
        })
    };

    let on_member_change = {
        let selected_member = selected_member.clone();
        let success_message = success_message.clone();
        let error_message = error_message.clone();

        Callback::from(move |user_id: Option<UserId>| {
            selected_member.set(user_id);
            success_message.set(None);
            error_message.set(None);
        })
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
        let recipient_type = recipient_type.clone();
        let selected_member = selected_member.clone();
        let amount_input = amount_input.clone();
        let note_input = note_input.clone();
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let success_message = success_message.clone();
        let on_success = props.on_success.clone();
        let amount_error = amount_error.clone();

        Callback::from(move |_: web_sys::MouseEvent| {
            // Validate
            if matches!(*recipient_type, RecipientType::SingleMember)
                && selected_member.is_none()
            {
                error_message.set(Some("Please select a member".to_string()));
                return;
            }

            if amount_input.is_empty() {
                error_message.set(Some("Please enter an amount".to_string()));
                return;
            }

            if amount_error.is_some() {
                return;
            }

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

            let recipient = match *recipient_type {
                RecipientType::SingleMember => {
                    TreasuryRecipient::SingleMember(selected_member.unwrap())
                }
                RecipientType::AllActiveMembers => {
                    TreasuryRecipient::AllActiveMembers
                }
            };

            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let success_message = success_message.clone();
            let amount_input_clear = amount_input.clone();
            let note_input_clear = note_input.clone();
            let selected_member_clear = selected_member.clone();
            let on_success = on_success.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);
                success_message.set(None);

                let api_client = get_api_client();
                let request = requests::TreasuryCreditOperation {
                    community_id,
                    recipient,
                    amount_per_recipient: amount,
                    note,
                    idempotency_key: IdempotencyKey(Uuid::new_v4()),
                };

                match api_client.treasury_credit_operation(&request).await {
                    Ok(_) => {
                        success_message.set(Some(
                            "Treasury operation successful!".to_string(),
                        ));
                        // Clear form
                        amount_input_clear.set(String::new());
                        note_input_clear.set(String::new());
                        selected_member_clear.set(None);
                        // Notify parent to refetch balances
                        on_success.emit(());
                    }
                    Err(e) => {
                        error_message.set(Some(format!(
                            "Treasury operation failed: {}",
                            e
                        )));
                    }
                }

                is_submitting.set(false);
            });
        })
    };

    let on_cancel = {
        let amount_input = amount_input.clone();
        let note_input = note_input.clone();
        let selected_member = selected_member.clone();
        let error_message = error_message.clone();
        let success_message = success_message.clone();
        let amount_error = amount_error.clone();

        Callback::from(move |_: web_sys::MouseEvent| {
            amount_input.set(String::new());
            note_input.set(String::new());
            selected_member.set(None);
            error_message.set(None);
            success_message.set(None);
            amount_error.set(None);
        })
    };

    let can_submit = (!matches!(*recipient_type, RecipientType::SingleMember)
        || selected_member.is_some())
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

            // Recipient type selector
            <div>
                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-1">
                    {"Recipient"}
                </label>
                <select
                    class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600 rounded bg-white dark:bg-neutral-800 text-neutral-900 dark:text-neutral-100"
                    onchange={on_recipient_type_change}
                    disabled={*is_submitting}
                >
                    <option value="all" selected={matches!(*recipient_type, RecipientType::AllActiveMembers)}>
                        {"All Active Members"}
                    </option>
                    <option value="single" selected={matches!(*recipient_type, RecipientType::SingleMember)}>
                        {"Single Member"}
                    </option>
                </select>
            </div>

            // Member selection (only for SingleMember)
            {
                if matches!(*recipient_type, RecipientType::SingleMember) {
                    html! {
                        <div>
                            <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-1">
                                {"Member"}
                            </label>
                            <MemberSelect
                                community_id={props.community_id}
                                selected={*selected_member}
                                on_change={on_member_change.clone()}
                                disabled={*is_submitting}
                            />
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // Amount input
            <div>
                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-1">
                    {
                        if matches!(*recipient_type, RecipientType::AllActiveMembers) {
                            "Amount per Member"
                        } else {
                            "Amount"
                        }
                    }
                </label>
                <div class="relative">
                    <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                        <span class="text-neutral-500 dark:text-neutral-400">
                            {&props.community.community.currency.symbol}
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

            // Entry type display
            <div class="text-sm text-neutral-600 dark:text-neutral-400">
                {"Entry type: "}
                <span class="font-medium">{entry_type_label}</span>
            </div>

            // Note input
            <div>
                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-1">
                    {"Note (optional)"}
                </label>
                <input
                    type="text"
                    class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600 rounded bg-white dark:bg-neutral-800 text-neutral-900 dark:text-neutral-100"
                    placeholder="What is this operation for?"
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
                    {if *is_submitting { "Processing..." } else { "Issue Credits" }}
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
