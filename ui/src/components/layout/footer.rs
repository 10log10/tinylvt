use crate::Route;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component]
pub fn Footer() -> Html {
    html! {
        <footer class="bg-white dark:bg-neutral-900 border-t border-neutral-200 dark:border-neutral-700 mt-auto">
            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4">
                <div class="flex justify-center gap-6">
                    <Link<Route> to={Route::Help} classes="text-sm text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white">
                        {"Help"}
                    </Link<Route>>
                    <a href="https://github.com/10log10/tinylvt" target="_blank" rel="noopener noreferrer" class="text-sm text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white">
                        {"Source"}
                    </a>
                </div>
            </div>
        </footer>
    }
}
