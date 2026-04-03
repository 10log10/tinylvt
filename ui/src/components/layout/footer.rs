use crate::Route;
use crate::hooks::use_push_route;
use yew::prelude::*;

const LINK_CLASSES: &str = "text-sm text-neutral-600 dark:text-neutral-400 \
     hover:text-neutral-900 dark:hover:text-white \
     cursor-pointer";

#[function_component]
pub fn Footer() -> Html {
    let push_route = use_push_route();

    let on_pricing = {
        let push_route = push_route.clone();
        Callback::from(move |_: MouseEvent| {
            push_route.emit(Route::Pricing);
        })
    };

    let on_terms = {
        Callback::from(move |_: MouseEvent| {
            push_route.emit(Route::Terms);
        })
    };

    html! {
        <footer class="bg-white dark:bg-neutral-900 \
                        border-t border-neutral-200 \
                        dark:border-neutral-700 mt-auto">
            <div class="max-w-7xl mx-auto px-4 \
                        sm:px-6 lg:px-8 py-4 \
                        space-y-2">
                <div class="flex flex-wrap justify-center gap-x-6 gap-y-1">
                    <a onclick={on_pricing}
                       class={LINK_CLASSES}>
                        {"Pricing"}
                    </a>
                    <a onclick={on_terms}
                       class={LINK_CLASSES}>
                        {"Terms"}
                    </a>
                    <a href="https://github.com/10log10/tinylvt"
                       target="_blank"
                       rel="noopener noreferrer"
                       class={LINK_CLASSES}>
                        {"Source"}
                    </a>
                    <a href="mailto:info@aperturebeam.com"
                       class={LINK_CLASSES}>
                        {"Contact"}
                    </a>
                </div>
                <p class="text-xs text-neutral-400 \
                          dark:text-neutral-500 \
                          text-center">
                    {"Aperture Beam Technologies, Inc."}
                </p>
            </div>
        </footer>
    }
}
