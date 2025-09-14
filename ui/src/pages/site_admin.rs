use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site_id: String,
}

#[function_component]
pub fn SiteAdminPage(props: &Props) -> Html {
    html! {
        <div class="max-w-4xl mx-auto py-8 px-4">
            <div class="bg-white dark:bg-neutral-800 p-8 rounded-lg shadow-md">
                <div class="mb-8 text-center">
                    <h1 class="text-2xl font-bold text-neutral-900 dark:text-neutral-100 mb-2">
                        {"Site Administration"}
                    </h1>
                    <p class="text-neutral-600 dark:text-neutral-400">
                        {"Configure auction parameters and manage site settings"}
                    </p>
                    <p class="text-sm text-neutral-500 dark:text-neutral-500 mt-2">
                        {"Site ID: "}{&props.site_id}
                    </p>
                </div>

                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400 mb-4">
                        {"Site administration features coming soon!"}
                    </p>
                    <p class="text-sm text-neutral-500 dark:text-neutral-500">
                        {"This page will include auction parameter editing, space management, and more."}
                    </p>
                </div>
            </div>
        </div>
    }
}
