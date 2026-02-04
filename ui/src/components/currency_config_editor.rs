use payloads::{
    CurrencyMode, CurrencyModeConfig, CurrencySettings, IOUConfig,
    PointsAllocationConfig, PrepaidCreditsConfig,
};
use rust_decimal::Decimal;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub currency: CurrencySettings,
    pub on_change: Callback<CurrencySettings>,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub can_change_mode: bool,
}

#[function_component]
pub fn CurrencyConfigEditor(props: &Props) -> Html {
    let current_mode = props.currency.mode_config.mode();

    html! {
        <div class="space-y-6">
            // Mode selector (only if can_change_mode)
            if props.can_change_mode {
                <div>
                    <label class="block text-sm font-medium mb-2 text-neutral-900 dark:text-neutral-100">
                        {"Currency Mode"}
                    </label>
                    <div class="space-y-2">
                        {render_mode_selector(props, &current_mode)}
                    </div>
                    <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                        {"Select how currency will be managed in your community"}
                    </p>
                </div>
            } else {
                // Show current mode as read-only badge
                <div>
                    <label class="block text-sm font-medium mb-2 text-neutral-900 dark:text-neutral-100">
                        {"Currency Mode"}
                    </label>
                    <div class="inline-block px-3 py-1 bg-neutral-100 dark:bg-neutral-800 text-neutral-700 dark:text-neutral-300 rounded">
                        {mode_display_name(&current_mode)}
                    </div>
                </div>
            }

            // Common fields section
            <div class="space-y-4">
                <h3 class="text-sm font-semibold text-neutral-900 dark:text-neutral-100">{"Currency Settings"}</h3>

                {render_currency_name_input(props)}
                {render_currency_symbol_input(props)}
                {render_currency_minor_units_input(props)}
                {render_balances_visible_checkbox(props)}
            </div>

            // Mode-specific fields section
            <div class="space-y-4">
                <h3 class="text-sm font-semibold text-neutral-900 dark:text-neutral-100">{"Mode-Specific Settings"}</h3>
                {render_mode_specific_fields(props, &current_mode)}
            </div>
        </div>
    }
}

fn render_mode_selector(props: &Props, current_mode: &CurrencyMode) -> Html {
    let modes = [
        (
            CurrencyMode::DistributedClearing,
            "Distributed Clearing",
            "Members issue IOUs to each other, settled among themselves",
        ),
        (
            CurrencyMode::PointsAllocation,
            "Points Allocation",
            "Treasury issues points to members on a schedule",
        ),
        (
            CurrencyMode::DeferredPayment,
            "Deferred Payment",
            "Members issue IOUs to treasury, settled later",
        ),
        (
            CurrencyMode::PrepaidCredits,
            "Prepaid Credits",
            "Members purchase credits from treasury upfront",
        ),
    ];

    html! {
        <>
            {for modes.iter().map(|(mode, name, desc)| {
                let is_selected = mode == current_mode;
                let on_select = {
                    let on_change_callback = props.on_change.clone();
                    let balances_visible = props.currency.balances_visible_to_members;
                    let currency_minor_units = props.currency.minor_units;
                    let mode = *mode;
                    Callback::from(move |_| {
                        let new_mode_config = create_default_config_for_mode(&mode);

                        // Auto-update currency name/symbol based on mode
                        let (currency_name, currency_symbol) = match mode {
                            CurrencyMode::PointsAllocation => {
                                ("points".to_string(), "P".to_string())
                            }
                            CurrencyMode::DistributedClearing
                            | CurrencyMode::DeferredPayment
                            | CurrencyMode::PrepaidCredits => {
                                ("dollars".to_string(), "$".to_string())
                            }
                        };

                        on_change_callback.emit(CurrencySettings {
                            mode_config: new_mode_config,
                            name: currency_name,
                            symbol: currency_symbol,
                            minor_units: currency_minor_units,
                            balances_visible_to_members: balances_visible,
                        });
                    })
                };

                html! {
                    <label class={classes!(
                        "flex", "items-start", "p-3", "border", "rounded", "cursor-pointer",
                        if is_selected { "border-neutral-900 dark:border-neutral-100" } else { "border-neutral-300 dark:border-neutral-600" },
                        if is_selected { "bg-neutral-50 dark:bg-neutral-800" } else { "" }
                    )}>
                        <input
                            type="radio"
                            name="currency_mode"
                            checked={is_selected}
                            onchange={on_select}
                            disabled={props.disabled}
                            class="mt-1 mr-3"
                        />
                        <div>
                            <div class="font-medium text-neutral-900 dark:text-neutral-100">{name}</div>
                            <div class="text-sm text-neutral-600 dark:text-neutral-400">{desc}</div>
                        </div>
                    </label>
                }
            })}
        </>
    }
}

fn render_currency_name_input(props: &Props) -> Html {
    let on_change = {
        let on_change_callback = props.on_change.clone();
        let currency = props.currency.clone();

        Callback::from(move |e: Event| {
            let input: HtmlInputElement =
                e.target().unwrap().dyn_into().unwrap();
            let value = input.value();
            on_change_callback.emit(CurrencySettings {
                name: value,
                ..currency.clone()
            });
        })
    };

    html! {
        <div>
            <label class="block text-sm font-medium mb-1 text-neutral-900 dark:text-neutral-100">
                {"Currency Name"}
            </label>
            <input
                type="text"
                value={props.currency.name.clone()}
                onchange={on_change}
                disabled={props.disabled}
                placeholder="dollars"
                class="w-full border border-neutral-300 dark:border-neutral-600 rounded px-3 py-2 bg-white dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100"
            />
            <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                {"The name of your community's currency (e.g., dollars, credits, points)"}
            </p>
        </div>
    }
}

fn render_currency_symbol_input(props: &Props) -> Html {
    let on_change = {
        let on_change_callback = props.on_change.clone();
        let currency = props.currency.clone();

        Callback::from(move |e: Event| {
            let input: HtmlInputElement =
                e.target().unwrap().dyn_into().unwrap();
            let value = input.value();
            // Validate max 5 characters (matches DB VARCHAR(5))
            if value.chars().count() <= 5 {
                on_change_callback.emit(CurrencySettings {
                    symbol: value,
                    ..currency.clone()
                });
            }
        })
    };

    html! {
        <div>
            <label class="block text-sm font-medium mb-1 text-neutral-900 dark:text-neutral-100">
                {"Currency Symbol"}
            </label>
            <input
                type="text"
                value={props.currency.symbol.clone()}
                onchange={on_change}
                disabled={props.disabled}
                placeholder="$"
                maxlength="5"
                class="w-20 border border-neutral-300 dark:border-neutral-600 rounded px-3 py-2 bg-white dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100"
            />
            <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                {"Symbol to display with amounts (1-5 characters)"}
            </p>
        </div>
    }
}

fn render_currency_minor_units_input(props: &Props) -> Html {
    let on_change = {
        let on_change_callback = props.on_change.clone();
        let currency = props.currency.clone();

        Callback::from(move |e: Event| {
            let input: HtmlInputElement =
                e.target().unwrap().dyn_into().unwrap();
            let value = input.value();

            if let Ok(minor_units) = value.parse::<i16>() {
                // Validate range 0-6 (matches DB constraint)
                if (0..=6).contains(&minor_units) {
                    on_change_callback.emit(CurrencySettings {
                        minor_units,
                        ..currency.clone()
                    });
                }
            }
        })
    };

    html! {
        <div>
            <label class="block text-sm font-medium mb-1 text-neutral-900 dark:text-neutral-100">
                {"Decimal Places"}
            </label>
            <input
                type="number"
                min="0"
                max="6"
                value={props.currency.minor_units.to_string()}
                onchange={on_change}
                disabled={props.disabled}
                class="w-20 border border-neutral-300 dark:border-neutral-600 rounded px-3 py-2 bg-white dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100"
            />
            <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                {"Number of decimal places to display (0-6). E.g., 2 for cents, 0 for whole units"}
            </p>
        </div>
    }
}

fn render_balances_visible_checkbox(props: &Props) -> Html {
    let on_change = {
        let on_change_callback = props.on_change.clone();
        let currency = props.currency.clone();

        Callback::from(move |e: Event| {
            let input: HtmlInputElement =
                e.target().unwrap().dyn_into().unwrap();
            let checked = input.checked();
            on_change_callback.emit(CurrencySettings {
                balances_visible_to_members: checked,
                ..currency.clone()
            });
        })
    };

    html! {
        <div class="flex items-start">
            <input
                type="checkbox"
                checked={props.currency.balances_visible_to_members}
                onchange={on_change}
                disabled={props.disabled}
                class="mt-1 mr-2"
            />
            <div>
                <label class="text-sm font-medium">
                    {"Balances visible to all members"}
                </label>
                <p class="text-sm text-neutral-600 dark:text-neutral-400">
                    {"If enabled, all members can see each other's balances"}
                </p>
            </div>
        </div>
    }
}

fn render_mode_specific_fields(props: &Props, _mode: &CurrencyMode) -> Html {
    match props.currency.mode_config {
        CurrencyModeConfig::PointsAllocation(ref config) => {
            render_points_allocation_fields(props, config)
        }
        CurrencyModeConfig::DistributedClearing(ref config) => {
            render_iou_fields(
                props,
                config,
                "In Distributed Clearing mode, members issue IOUs to each other which are settled among themselves.",
            )
        }
        CurrencyModeConfig::DeferredPayment(ref config) => render_iou_fields(
            props,
            config,
            "In Deferred Payment mode, members issue IOUs to the treasury which are settled later.",
        ),
        CurrencyModeConfig::PrepaidCredits(ref config) => {
            render_prepaid_credits_fields(props, config)
        }
    }
}

fn render_points_allocation_fields(
    props: &Props,
    _config: &PointsAllocationConfig,
) -> Html {
    let CurrencyModeConfig::PointsAllocation(boxed_config) =
        &props.currency.mode_config
    else {
        return html! {};
    };

    html! {
        <div class="space-y-4">
            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                {"In Points Allocation mode, the treasury issues currency to members on a regular schedule."}
            </p>

            {render_allowance_amount_input(props, boxed_config)}
            {render_allowance_period_input(props, boxed_config)}
            {render_allowance_start_input(props, boxed_config)}
        </div>
    }
}

fn render_iou_fields(
    props: &Props,
    config: &IOUConfig,
    description: &str,
) -> Html {
    // Check if configuration is invalid
    let is_invalid =
        !config.debts_callable && config.default_credit_limit.is_none();

    html! {
        <div class="space-y-4">
            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                {description}
            </p>

            {render_credit_limit_input(props, config.default_credit_limit)}
            {render_debts_callable_checkbox(props, config.debts_callable)}

            if is_invalid {
                <div class="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded">
                    <p class="text-sm text-red-700 dark:text-red-400">
                        {"⚠️ Invalid configuration: When debts are not callable, you must set a credit limit to prevent unlimited debt."}
                    </p>
                </div>
            }
        </div>
    }
}

fn render_prepaid_credits_fields(
    props: &Props,
    config: &PrepaidCreditsConfig,
) -> Html {
    html! {
        <div class="space-y-4">
            <p class="text-sm text-neutral-600 dark:text-neutral-400">
                {"In Prepaid Credits mode, members purchase credits from the treasury upfront."}
            </p>

            {render_debts_callable_checkbox(props, config.debts_callable)}
        </div>
    }
}

// Reusable input components

fn render_credit_limit_input(
    props: &Props,
    current_limit: Option<Decimal>,
) -> Html {
    let is_unlimited = current_limit.is_none();

    let on_unlimited_change = {
        let on_change_callback = props.on_change.clone();
        let currency = props.currency.clone();

        Callback::from(move |e: Event| {
            let input: HtmlInputElement =
                e.target().unwrap().dyn_into().unwrap();
            let checked = input.checked();

            let Some(new_mode_config) =
                currency.mode_config.set_default_credit_limit(if checked {
                    None
                } else {
                    Some(Decimal::new(100, 0))
                })
            else {
                return;
            };

            on_change_callback.emit(CurrencySettings {
                mode_config: new_mode_config,
                ..currency.clone()
            });
        })
    };

    let on_amount_change = {
        let on_change_callback = props.on_change.clone();
        let currency = props.currency.clone();

        Callback::from(move |e: Event| {
            let input: HtmlInputElement =
                e.target().unwrap().dyn_into().unwrap();
            let value = input.value();

            if let Ok(amount) = value.parse::<Decimal>()
                && amount >= Decimal::ZERO
            {
                let Some(new_mode_config) =
                    currency.mode_config.set_default_credit_limit(Some(amount))
                else {
                    return;
                };

                on_change_callback.emit(CurrencySettings {
                    mode_config: new_mode_config,
                    ..currency.clone()
                });
            }
        })
    };

    html! {
        <div>
            <label class="block text-sm font-medium mb-1 text-neutral-900 dark:text-neutral-100">
                {"Default Credit Limit"}
            </label>
            <div class="flex items-start space-x-3">
                <input
                    type="checkbox"
                    checked={is_unlimited}
                    onchange={on_unlimited_change}
                    disabled={props.disabled}
                    class="mt-2"
                />
                <div class="flex-1">
                    <label class="text-sm">{"Unlimited"}</label>
                </div>
            </div>
            if !is_unlimited {
                <input
                    type="number"
                    step="0.01"
                    min="0"
                    value={current_limit.map(|d| d.to_string()).unwrap_or_default()}
                    onchange={on_amount_change}
                    disabled={props.disabled}
                    class="mt-2 w-full border border-neutral-300 dark:border-neutral-600 rounded px-3 py-2 bg-white dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100"
                    placeholder="100"
                />
            }
            <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                {"Maximum debt members can accumulate. Required if debts are not callable."}
            </p>
        </div>
    }
}

fn render_debts_callable_checkbox(props: &Props, current_value: bool) -> Html {
    let on_change = {
        let on_change_callback = props.on_change.clone();
        let currency = props.currency.clone();

        Callback::from(move |e: Event| {
            let input: HtmlInputElement =
                e.target().unwrap().dyn_into().unwrap();
            let checked = input.checked();

            let Some(new_mode_config) =
                currency.mode_config.set_debts_callable(checked)
            else {
                return;
            };

            on_change_callback.emit(CurrencySettings {
                mode_config: new_mode_config,
                ..currency.clone()
            });
        })
    };

    html! {
        <div class="flex items-start">
            <input
                type="checkbox"
                checked={current_value}
                onchange={on_change}
                disabled={props.disabled}
                class="mt-1 mr-2"
            />
            <div>
                <label class="text-sm font-medium">
                    {"Debts are callable"}
                </label>
                <p class="text-sm text-neutral-600 dark:text-neutral-400">
                    {"If enabled, debts carry a promise that they can be settled in the denominated unit"}
                </p>
            </div>
        </div>
    }
}

fn render_allowance_amount_input(
    props: &Props,
    config: &PointsAllocationConfig,
) -> Html {
    let on_change = {
        let on_change_callback = props.on_change.clone();
        let currency = props.currency.clone();
        let allowance_period = config.allowance_period;
        let allowance_start = config.allowance_start;

        Callback::from(move |e: Event| {
            let input: HtmlInputElement =
                e.target().unwrap().dyn_into().unwrap();
            let value = input.value();

            if let Ok(amount) = value.parse::<Decimal>()
                && amount > Decimal::ZERO
            {
                let new_mode_config = CurrencyModeConfig::PointsAllocation(
                    Box::new(PointsAllocationConfig {
                        allowance_amount: amount,
                        allowance_period,
                        allowance_start,
                    }),
                );

                on_change_callback.emit(CurrencySettings {
                    mode_config: new_mode_config,
                    ..currency.clone()
                });
            }
        })
    };

    html! {
        <div>
            <label class="block text-sm font-medium mb-1 text-neutral-900 dark:text-neutral-100">
                {"Allowance Amount"}
            </label>
            <input
                type="number"
                step="0.01"
                min="0.01"
                value={config.allowance_amount.to_string()}
                onchange={on_change}
                disabled={props.disabled}
                class="w-full border border-neutral-300 dark:border-neutral-600 rounded px-3 py-2 bg-white dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100"
            />
            <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                {"Amount of currency issued to each member per period"}
            </p>
        </div>
    }
}

fn render_allowance_period_input(
    props: &Props,
    config: &PointsAllocationConfig,
) -> Html {
    // Determine if period is in months or days
    let (period_value, period_unit) = {
        let months = config.allowance_period.get_months();
        let days = config.allowance_period.get_days();
        if months > 0 && days == 0 {
            (months, "months")
        } else {
            (days, "days")
        }
    };

    let on_value_change = {
        let on_change_callback = props.on_change.clone();
        let currency = props.currency.clone();
        let allowance_amount = config.allowance_amount;
        let allowance_start = config.allowance_start;
        let current_unit = period_unit;

        Callback::from(move |e: Event| {
            let input: HtmlInputElement =
                e.target().unwrap().dyn_into().unwrap();
            let value = input.value();

            if let Ok(amount) = value.parse::<i64>()
                && amount > 0
            {
                let period = if current_unit == "months" {
                    jiff::Span::new().months(amount)
                } else {
                    jiff::Span::new().days(amount)
                };

                let new_mode_config = CurrencyModeConfig::PointsAllocation(
                    Box::new(PointsAllocationConfig {
                        allowance_amount,
                        allowance_period: period,
                        allowance_start,
                    }),
                );

                on_change_callback.emit(CurrencySettings {
                    mode_config: new_mode_config,
                    ..currency.clone()
                });
            }
        })
    };

    let on_unit_change = {
        let on_change_callback = props.on_change.clone();
        let currency = props.currency.clone();
        let allowance_amount = config.allowance_amount;
        let allowance_start = config.allowance_start;

        Callback::from(move |e: Event| {
            let select: web_sys::HtmlSelectElement =
                e.target().unwrap().dyn_into().unwrap();
            let unit = select.value();

            // Convert current value to new unit
            let period = if unit == "months" {
                jiff::Span::new().months(1)
            } else {
                jiff::Span::new().days(7)
            };

            let new_mode_config = CurrencyModeConfig::PointsAllocation(
                Box::new(PointsAllocationConfig {
                    allowance_amount,
                    allowance_period: period,
                    allowance_start,
                }),
            );

            on_change_callback.emit(CurrencySettings {
                mode_config: new_mode_config,
                ..currency.clone()
            });
        })
    };

    html! {
        <div>
            <label class="block text-sm font-medium mb-1 text-neutral-900 dark:text-neutral-100">
                {"Allowance Period"}
            </label>
            <div class="flex gap-2">
                <input
                    type="number"
                    min="1"
                    value={period_value.to_string()}
                    onchange={on_value_change}
                    disabled={props.disabled}
                    class="flex-1 border border-neutral-300 dark:border-neutral-600 rounded px-3 py-2 bg-white dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100"
                />
                <select
                    value={period_unit}
                    onchange={on_unit_change}
                    disabled={props.disabled}
                    class="border border-neutral-300 dark:border-neutral-600 rounded px-3 py-2 bg-white dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100"
                >
                    <option value="days">{"Days"}</option>
                    <option value="months">{"Months"}</option>
                </select>
            </div>
            <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                {"How often to issue allowances (e.g., 1 month, 7 days)"}
            </p>
        </div>
    }
}

fn render_allowance_start_input(
    props: &Props,
    config: &PointsAllocationConfig,
) -> Html {
    // Convert timestamp to datetime-local format
    let datetime_str = config
        .allowance_start
        .to_zoned(jiff::tz::TimeZone::system())
        .strftime("%Y-%m-%dT%H:%M")
        .to_string();

    let on_change = {
        let on_change_callback = props.on_change.clone();
        let currency = props.currency.clone();
        let allowance_amount = config.allowance_amount;
        let allowance_period = config.allowance_period;

        Callback::from(move |e: Event| {
            let input: HtmlInputElement =
                e.target().unwrap().dyn_into().unwrap();
            let value = input.value();

            // Parse datetime-local format (YYYY-MM-DDTHH:MM)
            // datetime-local is a civil datetime, we convert to system timezone
            if let Ok(civil) = value.parse::<jiff::civil::DateTime>()
                && let Ok(zoned) = civil.to_zoned(jiff::tz::TimeZone::system())
            {
                let new_mode_config = CurrencyModeConfig::PointsAllocation(
                    Box::new(PointsAllocationConfig {
                        allowance_amount,
                        allowance_period,
                        allowance_start: zoned.timestamp(),
                    }),
                );

                on_change_callback.emit(CurrencySettings {
                    mode_config: new_mode_config,
                    ..currency.clone()
                });
            }
        })
    };

    html! {
        <div>
            <label class="block text-sm font-medium mb-1 text-neutral-900 dark:text-neutral-100">
                {"Allowance Start"}
            </label>
            <input
                type="datetime-local"
                value={datetime_str}
                onchange={on_change}
                disabled={props.disabled}
                class="w-full border border-neutral-300 dark:border-neutral-600 rounded px-3 py-2 bg-white dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100"
            />
            <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                {"When to begin issuing allowances"}
            </p>
        </div>
    }
}

fn create_default_config_for_mode(mode: &CurrencyMode) -> CurrencyModeConfig {
    match mode {
        CurrencyMode::DistributedClearing => {
            CurrencyModeConfig::DistributedClearing(IOUConfig {
                default_credit_limit: None,
                debts_callable: true,
            })
        }
        CurrencyMode::PointsAllocation => {
            // Default to monthly allowance starting now
            CurrencyModeConfig::PointsAllocation(Box::new(
                PointsAllocationConfig {
                    allowance_amount: Decimal::new(100, 0),
                    allowance_period: jiff::Span::new().months(1),
                    allowance_start: jiff::Zoned::now().timestamp(),
                },
            ))
        }
        CurrencyMode::DeferredPayment => {
            CurrencyModeConfig::DeferredPayment(IOUConfig {
                default_credit_limit: None,
                debts_callable: true,
            })
        }
        CurrencyMode::PrepaidCredits => {
            CurrencyModeConfig::PrepaidCredits(PrepaidCreditsConfig {
                debts_callable: false,
            })
        }
    }
}

fn mode_display_name(mode: &CurrencyMode) -> &'static str {
    match mode {
        CurrencyMode::DistributedClearing => "Distributed Clearing",
        CurrencyMode::PointsAllocation => "Points Allocation",
        CurrencyMode::DeferredPayment => "Deferred Payment",
        CurrencyMode::PrepaidCredits => "Prepaid Credits",
    }
}
