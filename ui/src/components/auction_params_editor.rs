use jiff::{Span, SpanRound, Unit};
use payloads::AuctionParams;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction_params: AuctionParams,
    pub on_change: Callback<AuctionParams>,
    #[prop_or_default]
    pub disabled: bool,
}

#[function_component]
pub fn AuctionParamsEditor(props: &Props) -> Html {
    // round to minutes-seconds as largets and smallest units
    let rounded_duration = match props.auction_params.round_duration.round(
        SpanRound::new()
            .largest(Unit::Minute)
            .smallest(Unit::Second),
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(
                "Failed to round span {:?} with err: {:#}",
                props.auction_params.round_duration,
                e
            );
            Span::new().minutes(5).seconds(0)
        }
    };
    let round_duration_minutes = rounded_duration.get_minutes() as u32;
    let round_duration_seconds = rounded_duration.get_seconds() as u32;

    let on_round_duration_minutes_change = {
        let on_change = props.on_change.clone();
        let auction_params = props.auction_params.clone();

        Callback::from(move |e: Event| {
            let target = e.target().unwrap();
            let input = target.dyn_into::<HtmlInputElement>().unwrap();
            if let Ok(minutes) = input.value().parse::<u32>() {
                let mut updated = auction_params.clone();
                updated.round_duration = jiff::Span::new()
                    .minutes(minutes)
                    .seconds(round_duration_seconds);
                on_change.emit(updated);
            }
        })
    };

    let on_round_duration_seconds_change = {
        let on_change = props.on_change.clone();
        let auction_params = props.auction_params.clone();

        Callback::from(move |e: Event| {
            let target = e.target().unwrap();
            let input = target.dyn_into::<HtmlInputElement>().unwrap();
            if let Ok(seconds) = input.value().parse::<u32>() {
                // Limit seconds to 0-59
                if seconds < 60 {
                    let mut updated = auction_params.clone();
                    updated.round_duration = jiff::Span::new()
                        .minutes(round_duration_minutes)
                        .seconds(seconds);
                    on_change.emit(updated);
                }
            }
        })
    };

    let on_bid_increment_change = {
        let on_change = props.on_change.clone();
        let auction_params = props.auction_params.clone();

        Callback::from(move |e: Event| {
            let target = e.target().unwrap();
            let input = target.dyn_into::<HtmlInputElement>().unwrap();
            let value = input.value();

            if let Ok(parsed_decimal) = value.parse::<rust_decimal::Decimal>() {
                let mut updated = auction_params.clone();
                updated.bid_increment = parsed_decimal;
                on_change.emit(updated);
                return;
            }
            // Reset by triggering re-render with unchanged params
            on_change.emit(auction_params.clone());
        })
    };

    html! {
        <div class="space-y-4">
                <div>
                    <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                        {"Round Duration"}
                    </label>
                    <div class="flex space-x-2">
                        <div class="flex-1">
                            <input
                                type="number"
                                id="round-duration-minutes"
                                name="round_duration_minutes"
                                value={round_duration_minutes.to_string()}
                                onchange={on_round_duration_minutes_change}
                                disabled={props.disabled}
                                min="0"
                                step="1"
                                placeholder="5"
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                       rounded-md shadow-sm bg-white dark:bg-neutral-700
                                       text-neutral-900 dark:text-neutral-100
                                       focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                       dark:focus:ring-neutral-400 dark:focus:border-neutral-400
                                       disabled:opacity-50 disabled:cursor-not-allowed"
                            />
                            <p class="text-xs text-neutral-500 dark:text-neutral-400 mt-1 text-center">
                                {"minutes"}
                            </p>
                        </div>
                        <div class="flex-1">
                            <input
                                type="number"
                                id="round-duration-seconds"
                                name="round_duration_seconds"
                                value={round_duration_seconds.to_string()}
                                onchange={on_round_duration_seconds_change}
                                disabled={props.disabled}
                                min="0"
                                max="59"
                                step="1"
                                placeholder="0"
                                class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                                       rounded-md shadow-sm bg-white dark:bg-neutral-700
                                       text-neutral-900 dark:text-neutral-100
                                       focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                                       dark:focus:ring-neutral-400 dark:focus:border-neutral-400
                                       disabled:opacity-50 disabled:cursor-not-allowed"
                            />
                            <p class="text-xs text-neutral-500 dark:text-neutral-400 mt-1 text-center">
                                {"seconds"}
                            </p>
                        </div>
                    </div>
                    <p class="text-xs text-neutral-500 dark:text-neutral-400 mt-1">
                        {"Duration of each bidding round (e.g., 5 minutes 0 seconds, 0 minutes 30 seconds)"}
                    </p>
                </div>

                <div>
                    <label for="bid-increment" class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                        {"Bid Increment (dollars)"}
                    </label>
                    <input
                        type="text"
                        id="bid-increment"
                        name="bid_increment"
                        value={format!("{:.2}", props.auction_params.bid_increment)}
                        onchange={on_bid_increment_change}
                        disabled={props.disabled}
                        placeholder="1.00"
                        class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600
                               rounded-md shadow-sm bg-white dark:bg-neutral-700
                               text-neutral-900 dark:text-neutral-100
                               focus:outline-none focus:ring-2 focus:ring-neutral-500 focus:border-neutral-500
                               dark:focus:ring-neutral-400 dark:focus:border-neutral-400
                               disabled:opacity-50 disabled:cursor-not-allowed"
                    />
                    <p class="text-xs text-neutral-500 dark:text-neutral-400 mt-1">
                        {"Minimum bid increment in dollars (e.g., 1.00, 0.25, 5.50)"}
                    </p>
                </div>

                <div>
                    <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                        {"Activity Rules"}
                    </label>
                    <div class="bg-neutral-50 dark:bg-neutral-800 p-4 rounded-md">
                        <p class="text-sm text-neutral-600 dark:text-neutral-400 mb-3">
                            {"Eligibility progression by round:"}
                        </p>
                        <div class="space-y-2">
                            {props.auction_params.activity_rule_params.eligibility_progression.iter().enumerate().map(|(idx, (round, fraction))| {
                                let on_round_change = {
                                    let on_change = props.on_change.clone();
                                    let auction_params = props.auction_params.clone();
                                    Callback::from(move |e: Event| {
                                        let target = e.target().unwrap();
                                        let input = target.dyn_into::<HtmlInputElement>().unwrap();
                                        let value = input.value();

                                        if let Ok(new_round) = value.parse::<i32>()
                                            && new_round >= 0 {
                                                let mut updated = auction_params.clone();
                                                updated.activity_rule_params.eligibility_progression[idx].0 = new_round;
                                                on_change.emit(updated);
                                                return;
                                            }
                                        // Reset by triggering re-render with unchanged params
                                        on_change.emit(auction_params.clone());
                                    })
                                };

                                let on_eligibility_change = {
                                    let on_change = props.on_change.clone();
                                    let auction_params = props.auction_params.clone();
                                    Callback::from(move |e: Event| {
                                        let target = e.target().unwrap();
                                        let input = target.dyn_into::<HtmlInputElement>().unwrap();
                                        let value = input.value();

                                        if let Ok(percentage) = value.parse::<f64>()
                                            && (0.0..=100.0).contains(&percentage) {
                                                let fraction = percentage / 100.0;
                                                let mut updated = auction_params.clone();
                                                updated.activity_rule_params.eligibility_progression[idx].1 = fraction;
                                                on_change.emit(updated);
                                                return;
                                            }
                                        // Reset by triggering re-render with unchanged params
                                        on_change.emit(auction_params.clone());
                                    })
                                };

                                let on_remove = {
                                    let on_change = props.on_change.clone();
                                    let auction_params = props.auction_params.clone();
                                    Callback::from(move |_: MouseEvent| {
                                        if auction_params.activity_rule_params.eligibility_progression.len() > 1 {
                                            let mut updated = auction_params.clone();
                                            updated.activity_rule_params.eligibility_progression.remove(idx);
                                            on_change.emit(updated);
                                        }
                                    })
                                };

                                html! {
                                    <div key={format!("round-{}-{}", idx, round)} class="flex items-center gap-2">
                                        <div class="flex-1">
                                            <label class="text-xs text-neutral-500 dark:text-neutral-400">{"Round"}</label>
                                            <input
                                                type="number"
                                                value={round.to_string()}
                                                onchange={on_round_change}
                                                disabled={props.disabled}
                                                min="0"
                                                step="1"
                                                class="w-full px-2 py-1 border border-neutral-300 dark:border-neutral-600
                                                       rounded-md text-sm bg-white dark:bg-neutral-700
                                                       text-neutral-900 dark:text-neutral-100
                                                       focus:outline-none focus:ring-1 focus:ring-neutral-500
                                                       disabled:opacity-50 disabled:cursor-not-allowed"
                                            />
                                        </div>
                                        <div class="flex-1">
                                            <label class="text-xs text-neutral-500 dark:text-neutral-400">{"Eligibility %"}</label>
                                            <input
                                                type="number"
                                                value={format!("{:.0}", fraction * 100.0)}
                                                onchange={on_eligibility_change}
                                                disabled={props.disabled}
                                                min="0"
                                                max="100"
                                                step="1"
                                                class="w-full px-2 py-1 border border-neutral-300 dark:border-neutral-600
                                                       rounded-md text-sm bg-white dark:bg-neutral-700
                                                       text-neutral-900 dark:text-neutral-100
                                                       focus:outline-none focus:ring-1 focus:ring-neutral-500
                                                       disabled:opacity-50 disabled:cursor-not-allowed"
                                            />
                                        </div>
                                        <button
                                            type="button"
                                            onclick={on_remove}
                                            disabled={props.disabled || props.auction_params.activity_rule_params.eligibility_progression.len() <= 1}
                                            class="mt-5 px-2 py-1 text-sm text-red-600 dark:text-red-400
                                                   hover:text-red-800 dark:hover:text-red-300
                                                   disabled:opacity-30 disabled:cursor-not-allowed"
                                        >
                                            {"✕"}
                                        </button>
                                    </div>
                                }
                            }).collect::<Html>()}
                        </div>
                        <button
                            type="button"
                            onclick={{
                                let on_change = props.on_change.clone();
                                let auction_params = props.auction_params.clone();
                                Callback::from(move |_: MouseEvent| {
                                    let mut updated = auction_params.clone();
                                    let next_round = updated.activity_rule_params.eligibility_progression
                                        .iter()
                                        .map(|(r, _)| r)
                                        .max()
                                        .unwrap_or(&0) + 1;
                                    updated.activity_rule_params.eligibility_progression.push((next_round, 0.5));
                                    on_change.emit(updated);
                                })
                            }}
                            disabled={props.disabled}
                            class="mt-3 px-3 py-1 text-sm border border-neutral-300 dark:border-neutral-600
                                   rounded-md text-neutral-700 dark:text-neutral-300
                                   bg-white dark:bg-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-600
                                   focus:outline-none focus:ring-1 focus:ring-neutral-500
                                   disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            {"+ Add Breakpoint"}
                        </button>
                        <p class="text-xs text-neutral-500 dark:text-neutral-400 mt-3">
                            {"Activity rules determine the minimum participation required to stay eligible in each round"}
                        </p>
                    </div>
                </div>
        </div>
    }
}
