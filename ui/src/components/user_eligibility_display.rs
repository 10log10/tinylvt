use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub eligibility_points: f64,
    pub eligibility_threshold: f64,
}

#[function_component]
pub fn UserEligibilityDisplay(props: &Props) -> Html {
    let is_eligible = props.eligibility_points >= props.eligibility_threshold;

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 bg-white dark:bg-neutral-800">
            <div class="space-y-4">
                <h3 class="text-lg font-semibold text-neutral-900 \
                           dark:text-white">
                    {"Your Eligibility"}
                </h3>

                <div class="grid grid-cols-2 gap-4">
                    <div>
                        <div class="text-sm text-neutral-600 \
                                    dark:text-neutral-400 mb-1">
                            {"Your Points"}
                        </div>
                        <div class="text-2xl font-bold text-neutral-900 \
                                    dark:text-white">
                            {format!("{:.2}", props.eligibility_points)}
                        </div>
                    </div>

                    <div>
                        <div class="text-sm text-neutral-600 \
                                    dark:text-neutral-400 mb-1">
                            {"Threshold"}
                        </div>
                        <div class="text-2xl font-bold text-neutral-900 \
                                    dark:text-white">
                            {format!("{:.2}", props.eligibility_threshold)}
                        </div>
                    </div>
                </div>

                <div class={format!(
                    "px-4 py-3 rounded-md {}",
                    if is_eligible {
                        "bg-neutral-100 dark:bg-neutral-700 \
                         border border-neutral-300 dark:border-neutral-600"
                    } else {
                        "bg-neutral-200 dark:bg-neutral-600 \
                         border border-neutral-400 dark:border-neutral-500"
                    }
                )}>
                    <div class="flex items-center gap-2">
                        <span class="text-sm font-medium text-neutral-900 \
                                     dark:text-white">
                            {if is_eligible {
                                "✓ You are eligible to bid on all spaces"
                            } else {
                                "⚠ You can only bid on spaces where you \
                                 currently hold the high bid"
                            }}
                        </span>
                    </div>
                </div>
            </div>
        </div>
    }
}
