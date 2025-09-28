use payloads::{SiteId, responses::Site};
use yew::prelude::*;

use crate::hooks::use_site;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site_id: SiteId,
    pub children: Callback<Site, Html>,
}

#[function_component]
pub fn SitePageWrapper(props: &Props) -> Html {
    let site_hook = use_site(props.site_id);

    if site_hook.is_loading {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">{"Loading site..."}</p>
            </div>
        };
    }

    if let Some(error) = &site_hook.error {
        return html! {
            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
            </div>
        };
    }

    let site = match &site_hook.site {
        Some(s) => s,
        None => {
            return html! {
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">{"Site not found"}</p>
                </div>
            };
        }
    };

    html! {
        <div>
            {props.children.emit(site.clone())}
        </div>
    }
}
