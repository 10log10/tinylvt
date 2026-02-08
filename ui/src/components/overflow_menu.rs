use yew::prelude::*;

#[derive(Clone, PartialEq)]
pub struct MenuItem {
    pub label: AttrValue,
    pub on_click: Callback<()>,
    pub danger: bool,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub items: Vec<MenuItem>,
}

#[function_component]
pub fn OverflowMenu(props: &Props) -> Html {
    let is_open = use_state(|| false);

    // Don't render if no items
    if props.items.is_empty() {
        return html! {};
    }

    let on_toggle = {
        let is_open = is_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            is_open.set(!*is_open);
        })
    };

    let on_backdrop_click = {
        let is_open = is_open.clone();
        Callback::from(move |_: MouseEvent| {
            is_open.set(false);
        })
    };

    html! {
        <div class="relative">
            <button
                onclick={on_toggle}
                class="p-2 rounded hover:bg-neutral-100 dark:hover:bg-neutral-700 \
                       text-neutral-600 dark:text-neutral-400"
                aria-label="More options"
            >
                // Vertical ellipsis (kebab menu)
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    class="h-5 w-5"
                    viewBox="0 0 20 20"
                    fill="currentColor"
                >
                    <path d="M10 6a2 2 0 110-4 2 2 0 010 4zM10 12a2 2 0 110-4 2 2 0 010 4zM10 18a2 2 0 110-4 2 2 0 010 4z" />
                </svg>
            </button>

            {if *is_open {
                let is_open = is_open.clone();
                html! {
                    <>
                        // Backdrop to catch clicks outside menu
                        <div
                            onclick={on_backdrop_click}
                            class="fixed inset-0 z-40"
                        />

                        // Menu dropdown
                        <div class="absolute right-0 mt-1 py-1 w-48 bg-white \
                                    dark:bg-neutral-800 rounded-md shadow-lg \
                                    border border-neutral-200 dark:border-neutral-700 \
                                    z-50">
                            {props.items.iter().map(|item| {
                                let on_click = item.on_click.clone();
                                let is_open = is_open.clone();
                                let onclick = Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    is_open.set(false);
                                    on_click.emit(());
                                });

                                let text_class = if item.danger {
                                    "text-red-600 dark:text-red-400"
                                } else {
                                    "text-neutral-700 dark:text-neutral-300"
                                };

                                html! {
                                    <button
                                        onclick={onclick}
                                        class={format!(
                                            "w-full px-4 py-2 text-sm text-left \
                                             hover:bg-neutral-100 dark:hover:bg-neutral-700 \
                                             {}",
                                            text_class
                                        )}
                                    >
                                        {&item.label}
                                    </button>
                                }
                            }).collect::<Html>()}
                        </div>
                    </>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
