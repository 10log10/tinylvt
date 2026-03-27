use crate::Route;
use yew::prelude::*;
use yew_router::prelude::*;

const LINK_CLASSES: &str = "text-sm text-neutral-500 dark:text-neutral-400 \
     hover:text-neutral-900 dark:hover:text-white";

#[function_component]
pub fn Footer() -> Html {
    html! {
        <footer class="bg-white dark:bg-neutral-900 \
                        border-t border-neutral-200 \
                        dark:border-neutral-700 mt-auto">
            <div class="max-w-7xl mx-auto px-4 \
                        sm:px-6 lg:px-8 py-4 \
                        space-y-2">
                <div class="flex flex-wrap \
                            justify-center gap-x-6 \
                            gap-y-1">
                    <Link<Route>
                        to={Route::Terms}
                        classes={LINK_CLASSES}
                    >
                        {"Terms"}
                    </Link<Route>>
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
