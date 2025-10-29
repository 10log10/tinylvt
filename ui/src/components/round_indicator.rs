use crate::components::CountdownTimer;
use gloo_timers::callback::Interval;
use jiff::Timestamp;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub round_num: i32,
    pub round_end_at: Timestamp,
}

#[function_component]
pub fn RoundIndicator(props: &Props) -> Html {
    let round_concluded = use_state(|| Timestamp::now() >= props.round_end_at);

    // Update round_concluded status every second
    {
        let round_concluded = round_concluded.clone();
        let round_end_at = props.round_end_at;

        use_effect_with((), move |_| {
            let interval = Interval::new(1000, move || {
                let now = Timestamp::now();
                let is_concluded = now >= round_end_at;
                round_concluded.set(is_concluded);
            });

            move || drop(interval)
        });
    }

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-4 bg-white dark:bg-neutral-800">
            <div class="flex items-center justify-between">
                <div>
                    <h3 class="text-sm font-medium text-neutral-700 \
                               dark:text-neutral-300 uppercase tracking-wide">
                        {"Current Round"}
                    </h3>
                    <p class="text-2xl font-semibold text-neutral-900 \
                              dark:text-white mt-1">
                        {format!("Round {}", props.round_num)}
                    </p>
                </div>
                <div class="text-right">
                    <h3 class="text-sm font-medium text-neutral-700 \
                               dark:text-neutral-300 uppercase tracking-wide">
                        {if *round_concluded {
                            "Status"
                        } else {
                            "Time Remaining"
                        }}
                    </h3>
                    <p class="text-2xl font-semibold text-neutral-900 \
                              dark:text-white mt-1">
                        {if *round_concluded {
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
