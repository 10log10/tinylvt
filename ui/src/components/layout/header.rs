use crate::{components::DarkModeToggle, Route};
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component]
pub fn Header() -> Html {
    html! {
        <header class="bg-white dark:bg-neutral-900 border-b border-neutral-200 dark:border-neutral-700">
            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                <div class="flex justify-between items-center h-16">
                    <div class="flex items-center space-x-8">
                        <Link<Route> to={Route::Home} classes="text-xl font-semibold text-neutral-900 dark:text-white hover:text-neutral-700 dark:hover:text-neutral-300">
                            {"TinyLVT"}
                        </Link<Route>>
                        <nav class="hidden md:flex space-x-6">
                            <Link<Route> to={Route::Test} classes="text-sm text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white">
                                {"Test"}
                            </Link<Route>>
                        </nav>
                    </div>
                    <div class="flex items-center space-x-4">
                        <DarkModeToggle />
                    </div>
                </div>
            </div>
        </header>
    }
}

