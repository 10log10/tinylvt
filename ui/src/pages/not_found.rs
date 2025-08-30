use yew::prelude::*;

#[function_component]
pub fn NotFoundPage() -> Html {
    html! {
        <div class="text-center">
            <h1 class="text-4xl font-bold text-gray-900 dark:text-white">{"404"}</h1>
            <p class="text-gray-600 dark:text-gray-300">{"Page not found"}</p>
        </div>
    }
}