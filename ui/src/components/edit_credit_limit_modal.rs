use payloads::{
    CommunityId, CurrencySettings, requests, responses::UserIdentity,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use yew::prelude::*;

use crate::components::Modal;
use crate::components::user_identity_display::render_user_name;
use crate::get_api_client;
use crate::hooks::{FetchState, use_member_credit_limit_override};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub member: UserIdentity,
    pub community_id: CommunityId,
    pub currency: CurrencySettings,
    pub on_close: Callback<()>,
    pub on_success: Callback<()>,
}

#[function_component]
pub fn EditCreditLimitModal(props: &Props) -> Html {
    let credit_limit_hook = use_member_credit_limit_override(
        props.community_id,
        props.member.user_id,
    );

    let limit_input = use_state(String::new);
    let input_touched = use_state(|| false);
    let use_community_default = use_state(|| false);
    let is_submitting = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    // Initialize checkbox state when data loads
    {
        let use_community_default = use_community_default.clone();
        let credit_limit_override = credit_limit_hook
            .data
            .clone()
            .map(|cl| cl.credit_limit_override);
        use_effect_with(credit_limit_override, move |credit_limit_override| {
            if let FetchState::Fetched(cl) = credit_limit_override {
                use_community_default.set(cl.is_none());
            }
        });
    }

    // Validate and parse the input, returning the credit limit override value
    // Returns Ok(None) for community default, Ok(Some(limit)) for valid input,
    // Err(message) for invalid input
    let parse_input =
        |input: &str, use_default: bool| -> Result<Option<Decimal>, String> {
            if use_default {
                return Ok(None);
            }
            if input.is_empty() {
                return Err("Credit limit is required".into());
            }
            match Decimal::from_str(input) {
                Ok(limit) if limit < Decimal::ZERO => {
                    Err("Credit limit must be non-negative".into())
                }
                Ok(limit) => Ok(Some(limit)),
                Err(_) => Err("Invalid number".into()),
            }
        };

    let validation_error = if *use_community_default || !*input_touched {
        None
    } else {
        parse_input(&limit_input, false).err()
    };

    let on_limit_change = {
        let limit_input = limit_input.clone();
        let input_touched = input_touched.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            limit_input.set(input.value());
            input_touched.set(true);
        })
    };

    let on_checkbox_change = {
        let use_community_default = use_community_default.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            use_community_default.set(input.checked());
        })
    };

    let on_submit = {
        let community_id = props.community_id;
        let member_user_id = props.member.user_id;
        let limit_input = limit_input.clone();
        let use_community_default = use_community_default.clone();
        let is_submitting = is_submitting.clone();
        let error_message = error_message.clone();
        let on_success = props.on_success.clone();

        Callback::from(move |_| {
            let credit_limit_override =
                match parse_input(&limit_input, *use_community_default) {
                    Ok(value) => value,
                    Err(msg) => {
                        error_message.set(Some(msg));
                        return;
                    }
                };

            let is_submitting = is_submitting.clone();
            let error_message = error_message.clone();
            let on_success = on_success.clone();

            yew::platform::spawn_local(async move {
                is_submitting.set(true);
                error_message.set(None);

                let request = requests::UpdateCreditLimitOverride {
                    community_id,
                    member_user_id,
                    credit_limit_override,
                };

                match get_api_client()
                    .update_credit_limit_override(&request)
                    .await
                {
                    Ok(_) => on_success.emit(()),
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

    let can_submit = (*use_community_default || !limit_input.is_empty())
        && validation_error.is_none()
        && !*is_submitting;

    let member_display = render_user_name(&props.member);

    let currency = props.currency.clone();
    let on_close = props.on_close.clone();

    html! {
        <Modal on_close={on_close.clone()} max_width="max-w-md">
            <h2 class="text-xl font-semibold text-neutral-900 \
                        dark:text-neutral-100 mb-4">
                {"Edit Credit Limit"}
            </h2>
            {credit_limit_hook.render("credit limit", move |data, _, _| {
                let current_credit_limit_override = data.credit_limit_override;
                let default_limit = currency.default_credit_limit()
                    .map(|d| currency.format_amount(d))
                    .unwrap_or_else(|| "unlimited".into());

                let current_display = match current_credit_limit_override {
                    Some(limit) => currency.format_amount(limit),
                    None => format!("Community default ({})", default_limit),
                };

                let checkbox_label =
                    format!("Use community default ({})", default_limit);

                html! {
                    <>
                        <p class="text-sm text-neutral-600 \
                                  dark:text-neutral-400 mb-4">
                            {"Editing credit limit for "}
                            <span class="font-medium">{member_display.clone()}</span>
                        </p>

                        if let Some(msg) = &*error_message {
                            <div class="bg-red-50 dark:bg-red-900/20 \
                                        border border-red-200 \
                                        dark:border-red-800 rounded p-3 \
                                        text-sm text-red-800 \
                                        dark:text-red-200 mb-4">
                                {msg}
                            </div>
                        }

                        <div class="space-y-4">
                            <div class="text-sm">
                                <span class="text-neutral-600 \
                                             dark:text-neutral-400">
                                    {"Current: "}
                                </span>
                                <span class="font-medium text-neutral-900 \
                                             dark:text-neutral-100">
                                    {current_display}
                                </span>
                            </div>

                            <div class="flex items-center">
                                <input
                                    type="checkbox"
                                    id="no-override"
                                    checked={*use_community_default}
                                    onchange={on_checkbox_change.clone()}
                                    disabled={*is_submitting}
                                    class="h-4 w-4 text-neutral-900 \
                                           dark:text-neutral-100 \
                                           border-neutral-300 \
                                           dark:border-neutral-600 rounded \
                                           focus:ring-neutral-500"
                                />
                                <label for="no-override"
                                       class="ml-2 text-sm text-neutral-700 \
                                              dark:text-neutral-300">
                                    {checkbox_label}
                                </label>
                            </div>

                            if !*use_community_default {
                                <div>
                                    <label class="block text-sm font-medium \
                                                  text-neutral-700 \
                                                  dark:text-neutral-300 mb-1">
                                        {"Custom Credit Limit"}
                                    </label>
                                    <div class="relative">
                                        <div class="absolute inset-y-0 left-0 \
                                                    pl-3 flex items-center \
                                                    pointer-events-none">
                                            <span class="text-neutral-500 \
                                                         dark:text-neutral-400">
                                                {&currency.symbol}
                                            </span>
                                        </div>
                                        <input
                                            type="text"
                                            class={classes!(
                                                "w-full", "pl-8", "pr-3", "py-2",
                                                "border", "rounded", "bg-white",
                                                "dark:bg-neutral-800",
                                                "text-neutral-900",
                                                "dark:text-neutral-100",
                                                if validation_error.is_some() {
                                                    "border-red-300 \
                                                     dark:border-red-600"
                                                } else {
                                                    "border-neutral-300 \
                                                     dark:border-neutral-600"
                                                }
                                            )}
                                            placeholder="0.00"
                                            value={(*limit_input).clone()}
                                            oninput={on_limit_change.clone()}
                                            disabled={*is_submitting}
                                        />
                                    </div>
                                    if let Some(err) = &validation_error {
                                        <p class="mt-1 text-sm text-red-600 \
                                                  dark:text-red-400">
                                            {err}
                                        </p>
                                    }
                                </div>
                            }
                        </div>

                        <div class="flex gap-3 mt-6">
                            <button
                                onclick={on_submit.clone()}
                                disabled={!can_submit}
                                class="flex-1 justify-center py-2 px-4 border \
                                       border-transparent rounded-md shadow-sm \
                                       text-sm font-medium text-white \
                                       bg-neutral-900 hover:bg-neutral-800 \
                                       dark:bg-neutral-100 \
                                       dark:text-neutral-900 \
                                       dark:hover:bg-neutral-200 \
                                       focus:outline-none focus:ring-2 \
                                       focus:ring-offset-2 \
                                       focus:ring-neutral-500 \
                                       disabled:opacity-50 \
                                       disabled:cursor-not-allowed \
                                       transition-colors duration-200"
                            >
                                {if *is_submitting { "Updating..." }
                                 else { "Update Credit Limit" }}
                            </button>
                            <button
                                onclick={on_close.reform(|_| ())}
                                disabled={*is_submitting}
                                class="flex-1 py-2 px-4 border \
                                       border-neutral-300 \
                                       dark:border-neutral-600 rounded-md \
                                       shadow-sm text-sm font-medium \
                                       text-neutral-700 dark:text-neutral-300 \
                                       bg-white dark:bg-neutral-800 \
                                       hover:bg-neutral-50 \
                                       dark:hover:bg-neutral-700 \
                                       focus:outline-none focus:ring-2 \
                                       focus:ring-offset-2 \
                                       focus:ring-neutral-500 \
                                       disabled:opacity-50 \
                                       disabled:cursor-not-allowed \
                                       transition-colors duration-200"
                            >
                                {"Cancel"}
                            </button>
                        </div>
                    </>
                }
            })}
        </Modal>
    }
}
