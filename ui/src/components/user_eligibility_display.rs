use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub eligibility_points: Option<f64>,
    pub eligibility_threshold: f64,
    pub current_activity: Option<f64>,
}

#[function_component]
pub fn UserEligibilityDisplay(props: &Props) -> Html {
    let show_explanation = use_state(|| false);

    // Calculate values only if eligibility_points is available
    let min_required_activity = props
        .eligibility_points
        .map(|ep| ep * props.eligibility_threshold);

    // Calculate next round eligibility
    // - If we have prior eligibility: min(current, activity / threshold)
    // - If no prior eligibility (round 0): just activity / threshold
    let next_round_eligibility = props.current_activity.map(|ca| {
        let calculated = if props.eligibility_threshold > 0.0 {
            ca / props.eligibility_threshold
        } else {
            0.0
        };
        // Only apply min() if we have prior eligibility
        match props.eligibility_points {
            Some(ep) => calculated.min(ep),
            None => calculated,
        }
    });

    let toggle_explanation = {
        let show_explanation = show_explanation.clone();
        Callback::from(move |_| {
            show_explanation.set(!*show_explanation);
        })
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 bg-white dark:bg-neutral-800">
            <div class="space-y-4">
                <h3 class="text-lg font-semibold text-neutral-900 \
                           dark:text-white">
                    {"Your Eligibility"}
                </h3>

                <div class="grid grid-cols-3 gap-4">
                    <div>
                        <div class="text-sm text-neutral-600 \
                                    dark:text-neutral-400 mb-1">
                            {"Current Eligibility"}
                        </div>
                        <div class="text-2xl font-bold text-neutral-900 \
                                    dark:text-white">
                            {if let Some(ep) = props.eligibility_points {
                                format!("{:.1}", ep)
                            } else {
                                "--".to_string()
                            }}
                        </div>
                    </div>

                    <div>
                        <div class="text-sm text-neutral-600 \
                                    dark:text-neutral-400 mb-1">
                            {"Threshold"}
                        </div>
                        <div class="text-2xl font-bold text-neutral-900 \
                                    dark:text-white">
                            {format!("{:.0}%", props.eligibility_threshold * 100.0)}
                        </div>
                    </div>

                    <div>
                        <div class="text-sm text-neutral-600 \
                                    dark:text-neutral-400 mb-1">
                            {"To Maintain Eligibility"}
                        </div>
                        <div class="text-2xl font-bold text-neutral-900 \
                                    dark:text-white">
                            {if let Some(mra) = min_required_activity {
                                format!("{:.1}", mra)
                            } else {
                                "--".to_string()
                            }}
                        </div>
                    </div>
                </div>

                <div class="grid grid-cols-3 gap-4">
                    <div>
                        <div class="text-sm text-neutral-600 \
                                    dark:text-neutral-400 mb-1">
                            {"Current Activity"}
                        </div>
                        <div class={
                            match (props.current_activity, min_required_activity) {
                                (Some(ca), Some(mra)) if ca >= mra => {
                                    "text-2xl font-bold text-neutral-900 dark:text-white"
                                }
                                (Some(_), Some(_)) => {
                                    "text-2xl font-bold text-neutral-500 dark:text-neutral-400"
                                }
                                _ => "text-2xl font-bold text-neutral-900 dark:text-white"
                            }
                        }>
                            {if let Some(ca) = props.current_activity {
                                format!("{:.1}", ca)
                            } else {
                                "--".to_string()
                            }}
                        </div>
                    </div>

                    // Empty middle cell for alignment
                    <div></div>

                    <div>
                        <div class="text-sm text-neutral-600 \
                                    dark:text-neutral-400 mb-1">
                            {"Next Round Eligibility"}
                        </div>
                        <div class="text-2xl font-bold text-neutral-900 \
                                    dark:text-white">
                            {if let Some(nre) = next_round_eligibility {
                                format!("{:.1}", nre)
                            } else {
                                "--".to_string()
                            }}
                        </div>
                    </div>
                </div>

                <div>
                    <button
                        onclick={toggle_explanation}
                        class="text-sm text-neutral-600 dark:text-neutral-400 \
                               hover:text-neutral-900 dark:hover:text-white \
                               transition-colors"
                    >
                        {if *show_explanation {
                            "▼ Hide explanation"
                        } else {
                            "▶ How does eligibility work?"
                        }}
                    </button>

                    {if *show_explanation {
                        html! {
                            <div class="mt-3 text-sm text-neutral-600 \
                                        dark:text-neutral-400 space-y-2">
                                <p>
                                    <strong>{"Eligibility limits what you can bid on:"}</strong>
                                    {" Your current eligibility determines the \
                                     maximum total points you can bid for. You \
                                     cannot have more activity (sum of points for \
                                     spaces you're bidding on or winning) than \
                                     your eligibility."}
                                </p>
                                <p>
                                    <strong>{"How eligibility changes:"}</strong>
                                    {" If your activity meets the requirement \
                                     (shown above as 'To Maintain Eligibility'), \
                                     your eligibility stays the same for the next \
                                     round. If your activity falls below the \
                                     requirement, your eligibility decreases. \
                                     Specifically, it becomes your activity \
                                     divided by the threshold, which will be \
                                     lower than your current eligibility."}
                                </p>
                                <p>
                                    <strong>{"Activity"}</strong>
                                    {" is the sum of eligibility points for \
                                     spaces where you were already the high \
                                     bidder (from the previous round) or placed \
                                     a bid in this round."}
                                </p>
                                <p>
                                    <strong>{"Note:"}</strong>
                                    {" You only need to maintain your current \
                                     eligibility if you want to bid on spaces \
                                     requiring that eligibility level in future \
                                     rounds. If you only care about lower-point \
                                     spaces, you can let your eligibility \
                                     decrease naturally."}
                                </p>
                                <p>
                                    <strong>{"Important:"}</strong>
                                    {" Eligibility cannot increase once it has \
                                     decreased. This ensures price discovery by \
                                     preventing bidders from sitting out rounds \
                                     of the auction."}
                                </p>
                            </div>
                        }
                    } else {
                        html! {}
                    }}
                </div>
            </div>
        </div>
    }
}
