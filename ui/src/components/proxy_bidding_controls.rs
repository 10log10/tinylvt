use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub is_enabled: bool,
    pub max_items: i32,
    pub on_toggle: Callback<bool>,
    pub on_update: Callback<i32>,
    pub is_loading: bool,
}

#[function_component]
pub fn ProxyBiddingControls(props: &Props) -> Html {
    let max_items_input = use_state(|| props.max_items.to_string());
    let is_editing = use_state(|| false);

    // Reset input when props change
    {
        let max_items_input = max_items_input.clone();
        let max_items = props.max_items;
        use_effect_with(max_items, move |max_items| {
            max_items_input.set(max_items.to_string());
        });
    }

    let on_input_change = {
        let max_items_input = max_items_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            max_items_input.set(input.value());
        })
    };

    let on_toggle_click = {
        let on_toggle = props.on_toggle.clone();
        let is_enabled = props.is_enabled;
        Callback::from(move |_| {
            on_toggle.emit(!is_enabled);
        })
    };

    let on_save_click = {
        let max_items_input = max_items_input.clone();
        let on_update = props.on_update.clone();
        let is_editing = is_editing.clone();
        Callback::from(move |_| {
            if let Ok(value) = (*max_items_input).parse::<i32>()
                && value > 0
            {
                on_update.emit(value);
                is_editing.set(false);
            }
        })
    };

    let on_edit_click = {
        let is_editing = is_editing.clone();
        Callback::from(move |_| {
            is_editing.set(true);
        })
    };

    let on_cancel_click = {
        let is_editing = is_editing.clone();
        let max_items_input = max_items_input.clone();
        let max_items = props.max_items;
        Callback::from(move |_| {
            max_items_input.set(max_items.to_string());
            is_editing.set(false);
        })
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
                    rounded-lg p-6 bg-white dark:bg-neutral-800">
            <div class="space-y-4">
                <div class="flex items-center justify-between">
                    <h3 class="text-lg font-semibold text-neutral-900 \
                               dark:text-white">
                        {"Proxy Bidding"}
                    </h3>
                    <button
                        onclick={on_toggle_click}
                        disabled={props.is_loading}
                        class={format!(
                            "relative inline-flex h-6 w-11 items-center \
                             rounded-full transition-colors {}",
                            if props.is_enabled {
                                "bg-neutral-900 dark:bg-neutral-400"
                            } else {
                                "bg-neutral-300 dark:bg-neutral-600"
                            }
                        )}
                    >
                        <span class={format!(
                            "inline-block h-4 w-4 transform rounded-full \
                             bg-white transition-transform {}",
                            if props.is_enabled {
                                "translate-x-6"
                            } else {
                                "translate-x-1"
                            }
                        )} />
                    </button>
                </div>

                {if props.is_enabled {
                    html! {
                        <div class="space-y-3">
                            <p class="text-sm text-neutral-600 \
                                      dark:text-neutral-400">
                                {"Proxy bidding will automatically bid on your \
                                 behalf based on the maximum values you set for \
                                 each space."}
                            </p>

                            <div class="space-y-2">
                                <label class="block text-sm font-medium \
                                              text-neutral-700 \
                                              dark:text-neutral-300">
                                    {"Maximum Spaces to Win"}
                                </label>
                                {if *is_editing {
                                    html! {
                                        <div class="space-y-2">
                                            <input
                                                type="number"
                                                min="1"
                                                value={(*max_items_input).clone()}
                                                oninput={on_input_change}
                                                class="block w-full rounded-md \
                                                       border-neutral-300 \
                                                       dark:border-neutral-600 \
                                                       dark:bg-neutral-700 \
                                                       dark:text-white px-3 py-2 \
                                                       text-sm"
                                                placeholder="Enter max spaces"
                                            />
                                            <div class="flex gap-2">
                                                <button
                                                    onclick={on_save_click}
                                                    disabled={props.is_loading}
                                                    class="bg-neutral-900 \
                                                           hover:bg-neutral-800 \
                                                           dark:bg-neutral-100 \
                                                           dark:text-neutral-900 \
                                                           dark:hover:bg-neutral-200 \
                                                           text-white px-3 py-1.5 \
                                                           rounded-md text-sm \
                                                           font-medium \
                                                           transition-colors \
                                                           disabled:opacity-50"
                                                >
                                                    {"Save"}
                                                </button>
                                                <button
                                                    onclick={on_cancel_click}
                                                    disabled={props.is_loading}
                                                    class="border border-neutral-300 \
                                                           dark:border-neutral-600 \
                                                           hover:bg-neutral-100 \
                                                           dark:hover:bg-neutral-700 \
                                                           px-3 py-1.5 rounded-md \
                                                           text-sm font-medium \
                                                           transition-colors \
                                                           disabled:opacity-50"
                                                >
                                                    {"Cancel"}
                                                </button>
                                            </div>
                                        </div>
                                    }
                                } else {
                                    html! {
                                        <div class="flex items-center \
                                                    justify-between">
                                            <span class="text-2xl font-bold \
                                                         text-neutral-900 \
                                                         dark:text-white">
                                                {props.max_items}
                                            </span>
                                            <button
                                                onclick={on_edit_click}
                                                disabled={props.is_loading}
                                                class="text-sm text-neutral-600 \
                                                       hover:text-neutral-900 \
                                                       dark:text-neutral-400 \
                                                       dark:hover:text-neutral-200 \
                                                       underline"
                                            >
                                                {"Edit"}
                                            </button>
                                        </div>
                                    }
                                }}
                                <p class="text-xs text-neutral-500 \
                                          dark:text-neutral-400">
                                    {"The proxy bidder will try to win up to this \
                                     many spaces, prioritizing those with the \
                                     highest surplus (value - price)."}
                                </p>
                            </div>
                        </div>
                    }
                } else {
                    html! {
                        <p class="text-sm text-neutral-600 \
                                  dark:text-neutral-400">
                            {"Enable proxy bidding to automatically bid on \
                             spaces based on your maximum values."}
                        </p>
                    }
                }}
            </div>
        </div>
    }
}
