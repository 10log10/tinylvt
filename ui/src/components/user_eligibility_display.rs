use yew::prelude::*;

use crate::hooks::{Fetch, render_cell};

/// Format f64 for display, normalizing -0.0 to 0.0
fn format_value(v: f64) -> String {
    // Adding 0.0 converts -0.0 to 0.0
    format!("{:.1}", v + 0.0)
}

#[derive(Properties, PartialEq)]
pub struct Props {
    /// User's eligibility for the current round. Inner `Option<f64>` is
    /// `None` when the user has no prior eligibility (e.g., round 0); the
    /// outer `Fetch` distinguishes that from the loading window.
    pub eligibility_points: Fetch<Option<f64>>,
    pub eligibility_threshold: f64,
    /// Current activity (sum of eligibility points across spaces the user
    /// is bidding on or winning). Loading until spaces, prices, and bids
    /// have all been fetched; rendered as a skeleton until then.
    pub current_activity: Fetch<f64>,
}

#[function_component]
pub fn UserEligibilityDisplay(props: &Props) -> Html {
    let show_explanation = use_state(|| false);
    let eligibility_threshold = props.eligibility_threshold;

    // Pre-derive cells whose value depends on eligibility_points. Each is
    // `Fetch<Option<f64>>`: `Fetched(None)` means "no prior eligibility,
    // show '--'", `Fetched(Some(x))` shows the value, `NotFetched` /
    // loading shows a skeleton.
    let min_required_activity: Fetch<Option<f64>> = props
        .eligibility_points
        .map_ref(|ep_opt| ep_opt.map(|ep| ep * eligibility_threshold));

    // Next-round eligibility needs both current_activity and
    // eligibility_points. Combine via `zip_ref` so the cell renders a
    // skeleton until both are fetched.
    //   - If we have prior eligibility: min(current, activity / threshold)
    //   - If no prior eligibility (round 0): just activity / threshold
    let next_round_eligibility: Fetch<f64> = props
        .current_activity
        .zip_ref(&props.eligibility_points)
        .map(|(ca, ep_opt)| {
            let calculated = if eligibility_threshold > 0.0 {
                ca / eligibility_threshold
            } else {
                0.0
            };
            match ep_opt {
                Some(ep) => calculated.min(*ep),
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
                        {render_cell(&props.eligibility_points, |ep_opt| html! {
                            <div class="text-2xl font-bold text-neutral-900 \
                                        dark:text-white">
                                {match ep_opt {
                                    Some(ep) => format_value(*ep),
                                    None => "--".to_string(),
                                }}
                            </div>
                        })}
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
                        {render_cell(&min_required_activity, |mra_opt| html! {
                            <div class="text-2xl font-bold text-neutral-900 \
                                        dark:text-white">
                                {match mra_opt {
                                    Some(mra) => format_value(*mra),
                                    None => "--".to_string(),
                                }}
                            </div>
                        })}
                    </div>
                </div>

                <div class="grid grid-cols-3 gap-4">
                    <div>
                        <div class="text-sm text-neutral-600 \
                                    dark:text-neutral-400 mb-1">
                            {"Current Activity"}
                        </div>
                        // Activity styling depends on whether activity meets
                        // the minimum-required threshold; combine both via
                        // `zip_ref` so the cell renders a skeleton until
                        // both inputs are fetched.
                        {render_cell(
                            &props.current_activity.zip_ref(&min_required_activity),
                            |pair| {
                                let ca: f64 = *pair.0;
                                let mra_opt: Option<f64> = *pair.1;
                                let class = match mra_opt {
                                    Some(mra) if ca >= mra => {
                                        "text-2xl font-bold text-neutral-900 \
                                         dark:text-white"
                                    }
                                    Some(_) => {
                                        "text-2xl font-bold text-neutral-500 \
                                         dark:text-neutral-400"
                                    }
                                    None => {
                                        "text-2xl font-bold text-neutral-900 \
                                         dark:text-white"
                                    }
                                };
                                html! {
                                    <div class={class}>{format_value(ca)}</div>
                                }
                            },
                        )}
                    </div>

                    // Empty middle cell for alignment
                    <div></div>

                    <div>
                        <div class="text-sm text-neutral-600 \
                                    dark:text-neutral-400 mb-1">
                            {"Next Round Eligibility"}
                        </div>
                        {render_cell(&next_round_eligibility, |nre| html! {
                            <div class="text-2xl font-bold text-neutral-900 \
                                        dark:text-white">
                                {format_value(*nre)}
                            </div>
                        })}
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
