use yew::prelude::*;
use crate::components::DarkModeToggle;

#[function_component]
pub fn Header() -> Html {
    html! {
        <header class="bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                <div class="flex justify-between items-center h-16">
                    <div class="flex-shrink-0">
                        <h1 class="text-xl font-semibold text-gray-900 dark:text-white">{"TinyLVT"}</h1>
                    </div>
                    <div class="flex items-center space-x-4">
                        <DarkModeToggle />
                    </div>
                </div>
            </div>
        </header>
    }
}