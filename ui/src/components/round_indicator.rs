use crate::components::CountdownTimer;
use gloo_timers::callback::Interval;
use jiff::Timestamp;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub round_num: i32,
    pub round_end_at: Timestamp,
    #[prop_or_default]
    pub auction_end_at: Option<Timestamp>,
    #[prop_or_default]
    pub on_round_end: Option<Callback<()>>,
}

#[function_component]
pub fn RoundIndicator(props: &Props) -> Html {
    let round_concluded = use_state(|| Timestamp::now() >= props.round_end_at);

    // Update round_concluded status every second
    {
        let round_concluded = round_concluded.clone();
        let round_end_at = props.round_end_at;
        let on_round_end = props.on_round_end.clone();

        use_effect_with((), move |_| {
            let callback_called = std::cell::Cell::new(false);

            let interval = Interval::new(1000, move || {
                let now = Timestamp::now();
                let was_concluded = *round_concluded;
                let is_concluded = now >= round_end_at;

                // If round just concluded (and we haven't called callback yet)
                if is_concluded && !was_concluded && !callback_called.get() {
                    if let Some(callback) = &on_round_end {
                        tracing::info!(
                            "RoundIndicator: round just concluded, triggering \
                             on_round_end callback"
                        );
                        callback.emit(());
                        callback_called.set(true);
                    }
                }

                round_concluded.set(is_concluded);
            });

            move || drop(interval)
        });
    }

    // Check if auction has ended
    let auction_has_ended = props.auction_end_at.is_some();

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-4 bg-white dark:bg-neutral-800">
            <div class="flex items-center justify-between">
                <div>
                    <h3 class="text-sm font-medium text-neutral-700 \
                               dark:text-neutral-300 uppercase tracking-wide">
                        {if auction_has_ended {
                            "Final Round"
                        } else {
                            "Current Round"
                        }}
                    </h3>
                    <p class="text-2xl font-semibold text-neutral-900 \
                              dark:text-white mt-1">
                        {format!("Round {}", props.round_num)}
                    </p>
                </div>
                <div class="text-right">
                    <h3 class="text-sm font-medium text-neutral-700 \
                               dark:text-neutral-300 uppercase tracking-wide">
                        {if auction_has_ended {
                            "Status"
                        } else if *round_concluded {
                            "Status"
                        } else {
                            "Time Remaining"
                        }}
                    </h3>
                    <p class="text-2xl font-semibold text-neutral-900 \
                              dark:text-white mt-1">
                        {if auction_has_ended {
                            html! {
                                <span class="text-neutral-600 dark:text-neutral-400">
                                    {"Auction Concluded"}
                                </span>
                            }
                        } else if *round_concluded {
                            html! {
                                <span class="text-neutral-600 dark:text-neutral-400">
                                    {"Processing..."}
                                </span>
                            }
                        } else {
                            html! {
                                <CountdownTimer target_time={props.round_end_at} />
                            }
                        }}
                    </p>
                </div>
            </div>
        </div>
    }
}
