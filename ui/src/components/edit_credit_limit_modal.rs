use payloads::{
    CommunityId, CurrencySettings, requests, responses::UserIdentity,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use yew::prelude::*;

use crate::get_api_client;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub member: UserIdentity,
    pub community_id: CommunityId,
    pub current_credit_limit: Option<Decimal>,
    pub currency: CurrencySettings,
    pub on_close: Callback<()>,
    pub on_success: Callback<()>,
}

#[function_component]
pub fn EditCreditLimitModal(props: &Props) -> Html {
    // Form state
    let limit_input = use_state(|| {
        props
            .current_credit_limit
            .map(|l| l.to_string())
            .unwrap_or_default()
    });
    let is_unlimited = use_state(|| props.current_credit_limit.is_none());

    // Submission state
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);
    let success_message = use_state(|| None::<String>);

    // Validation
    let limit_error = use_state(|| None::<String>);

    // Validate limit on change
    let validate_limit = {
        let limit_input = limit_input.clone();
        let limit_error = limit_error.clone();
        let is_unlimited = is_unlimited.clone();

        move || {
            if *is_unlimited {
                limit_error.set(None);
                return;
            }

            let input = (*limit_input).clone();
            if input.is_empty() {
                limit_error.set(Some(
                    "Credit limit is required (or check Unlimited)".to_string(),
                ));
                return;
            }

            match Decimal::from_str(&input) {
                Ok(limit) => {
                    if limit < Decimal::ZERO {
                        limit_error.set(Some(
                            "Credit limit must be non-negative".to_string(),
                        ));
                    } else {
                        limit_error.set(None);
                    }
                }
                Err(_) => {
                    limit_error.set(Some("Invalid credit limit".to_string()));
                }
            }
        }
    };

    let on_limit_change = {
        let limit_input = limit_input.clone();
        let success_message = success_message.clone();
        let error_message = error_message.clone();
        let validate_limit = validate_limit.clone();

        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            limit_input.set(input.value());
            success_message.set(None);
            error_message.set(None);
            validate_limit();
        })
    };

    let on_unlimited_change = {
        let is_unlimited = is_unlimited.clone();
        let success_message = success_message.clone();
        let error_message = error_message.clone();
        let limit_error = limit_error.clone();

        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            is_unlimited.set(input.checked());
            success_message.set(None);
            error_message.set(None);
            limit_error.set(None);
        })
    };

    let on_submit = {
        let community_id = props.community_id;
        let member_user_id = props.member.user_id;
        let limit_input = limit_input.clone();
        let is_unlimited = is_unlimited.clone();
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let success_message = success_message.clone();
        let on_success = props.on_success.clone();
        let limit_error = limit_error.clone();

        Callback::from(move |_| {
            // Validate
            if !*is_unlimited && limit_input.is_empty() {
                error_message.set(Some(
                    "Please enter a credit limit or check Unlimited"
                        .to_string(),
                ));
                return;
            }

            if limit_error.is_some() {
                return;
            }

            let credit_limit = if *is_unlimited {
                None
            } else {
                match Decimal::from_str(&limit_input) {
                    Ok(limit) => Some(limit),
                    Err(_) => {
                        error_message
                            .set(Some("Invalid credit limit".to_string()));
                        return;
                    }
                }
            };

            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let success_message = success_message.clone();
            let on_success = on_success.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);
                success_message.set(None);

                let api_client = get_api_client();
                let request = requests::UpdateCreditLimit {
                    community_id,
                    member_user_id,
                    credit_limit,
                };

                match api_client.update_credit_limit(&request).await {
                    Ok(_) => {
                        success_message.set(Some(
                            "Credit limit updated successfully!".to_string(),
                        ));
                        // Notify parent to refetch and close modal
                        on_success.emit(());
                    }
                    Err(e) => {
                        error_message.set(Some(format!(
                            "Failed to update credit limit: {}",
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
        Callback::from(move |_| {
            on_close.emit(());
        })
    };

    let can_submit = (*is_unlimited || !limit_input.is_empty())
        && limit_error.is_none()
        && !*is_submitting;

    let member_display_name = props
        .member
        .display_name
        .as_ref()
        .unwrap_or(&props.member.username);

    html! {
        // Modal overlay
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
            // Modal content
            <div class="bg-white dark:bg-neutral-800 rounded-lg shadow-xl max-w-md w-full mx-4 p-6">
                <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                    {"Edit Credit Limit"}
                </h2>

                <p class="text-sm text-neutral-600 dark:text-neutral-400 mb-4">
                    {"Editing credit limit for "}
                    <span class="font-medium">{member_display_name}</span>
                </p>

                // Success message
                {
                    if let Some(msg) = &*success_message {
                        html! {
                            <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded p-3 text-sm text-green-800 dark:text-green-200 mb-4">
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
                            <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded p-3 text-sm text-red-800 dark:text-red-200 mb-4">
                                {msg}
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                <div class="space-y-4">
                    // Current credit limit display
                    <div class="text-sm">
                        <span class="text-neutral-600 dark:text-neutral-400">{"Current: "}</span>
                        <span class="font-medium text-neutral-900 dark:text-neutral-100">
                            {
                                if let Some(limit) = props.current_credit_limit {
                                    props.currency.format_amount(limit)
                                } else {
                                    "Unlimited".to_string()
                                }
                            }
                        </span>
                    </div>

                    // Unlimited checkbox
                    <div class="flex items-center">
                        <input
                            type="checkbox"
                            id="unlimited"
                            checked={*is_unlimited}
                            onchange={on_unlimited_change}
                            disabled={*is_submitting}
                            class="h-4 w-4 text-neutral-900 dark:text-neutral-100 border-neutral-300 dark:border-neutral-600 rounded focus:ring-neutral-500"
                        />
                        <label
                            for="unlimited"
                            class="ml-2 text-sm text-neutral-700 dark:text-neutral-300"
                        >
                            {"Set to Unlimited"}
                        </label>
                    </div>

                    // Credit limit input
                    {
                        if !*is_unlimited {
                            html! {
                                <div>
                                    <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-1">
                                        {"New Credit Limit"}
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
                                                if limit_error.is_some() {
                                                    classes!("border-red-300", "dark:border-red-600")
                                                } else {
                                                    classes!("border-neutral-300", "dark:border-neutral-600")
                                                }
                                            )}
                                            placeholder="0.00"
                                            value={(*limit_input).clone()}
                                            onchange={on_limit_change}
                                            disabled={*is_submitting}
                                        />
                                    </div>
                                    {
                                        if let Some(err) = &*limit_error {
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
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                // Actions
                <div class="flex gap-3 mt-6">
                    <button
                        onclick={on_submit}
                        disabled={!can_submit}
                        class="flex-1 justify-center py-2 px-4 border border-transparent
                               rounded-md shadow-sm text-sm font-medium text-white
                               bg-neutral-900 hover:bg-neutral-800
                               dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200
                               focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                               disabled:opacity-50 disabled:cursor-not-allowed
                               transition-colors duration-200"
                    >
                        {if *is_submitting { "Updating..." } else { "Update Credit Limit" }}
                    </button>
                    <button
                        onclick={on_cancel}
                        disabled={*is_submitting}
                        class="flex-1 py-2 px-4 border border-neutral-300 dark:border-neutral-600
                               rounded-md shadow-sm text-sm font-medium
                               text-neutral-700 dark:text-neutral-300
                               bg-white dark:bg-neutral-800
                               hover:bg-neutral-50 dark:hover:bg-neutral-700
                               focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-neutral-500
                               disabled:opacity-50 disabled:cursor-not-allowed
                               transition-colors duration-200"
                    >
                        {"Cancel"}
                    </button>
                </div>
            </div>
        </div>
    }
}
