use jiff::{Span, SpanRound, Unit};
use payloads::AuctionParams;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction_params: AuctionParams,
}

#[function_component]
pub fn AuctionParamsViewer(props: &Props) -> Html {
    // round to minutes-seconds as largest and smallest units
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

    html! {
        <div class="space-y-6">
            <div>
                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                    {"Round Duration"}
                </label>
                <p class="text-neutral-900 dark:text-neutral-100">
                    {format!("{} minutes, {} seconds", round_duration_minutes, round_duration_seconds)}
                </p>
            </div>

            <div>
                <label class="block text-sm font-medium text-neutral-700 dark:text-neutral-300 mb-2">
                    {"Bid Increment"}
                </label>
                <p class="text-neutral-900 dark:text-neutral-100">
                    {format!("${:.2}", props.auction_params.bid_increment)}
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
                        {props.auction_params.activity_rule_params.eligibility_progression.iter().map(|(round, fraction)| {
                            html! {
                                <div class="flex items-center gap-4 text-sm text-neutral-900 dark:text-neutral-100">
                                    <span class="font-medium">{"Round "}{round}{":"}</span>
                                    <span>{format!("{:.0}% eligibility required", fraction * 100.0)}</span>
                                </div>
                            }
                        }).collect::<Html>()}
                    </div>
                    <p class="text-xs text-neutral-500 dark:text-neutral-400 mt-3">
                        {"Activity rules determine the minimum participation required to stay eligible in each round"}
                    </p>
                </div>
            </div>
        </div>
    }
}
