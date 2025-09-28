use payloads::{SiteId, responses::Site};
use yew::prelude::*;

use crate::components::{
    SitePageWrapper, SiteTabHeader, site_tab_header::ActiveTab,
};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site_id: SiteId,
}

#[function_component]
pub fn SiteDetailPage(props: &Props) -> Html {
    let render_content = Callback::from(|site: Site| {
        html! {
            <div>
                <SiteTabHeader site={site.clone()} active_tab={ActiveTab::Spaces} />
                <div class="py-6">
                    <SpacesTab site_id={site.site_id} />
                </div>
            </div>
        }
    });

    html! {
        <SitePageWrapper
            site_id={props.site_id}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
pub struct SpacesTabProps {
    pub site_id: SiteId,
}

#[function_component]
fn SpacesTab(_props: &SpacesTabProps) -> Html {
    html! {
        <div>
            <div class="flex justify-between items-center mb-6">
                <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                    {"Spaces"}
                </h2>
                <button class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors">
                    {"Create New Space"}
                </button>
            </div>

            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400 mb-4">
                    {"Spaces functionality will be implemented here."}
                </p>
                <p class="text-sm text-neutral-500 dark:text-neutral-500">
                    {"This will show all spaces for this site with options to create, edit, and manage them."}
                </p>
            </div>
        </div>
    }
}
