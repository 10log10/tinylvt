use payloads::{SiteId, responses::Site};
use yew::prelude::*;

use crate::components::{
    SitePageWrapper, SiteTabHeader, site_tab_header::ActiveTab,
};
use crate::hooks::use_spaces;

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
fn SpacesTab(props: &SpacesTabProps) -> Html {
    let spaces_hook = use_spaces(props.site_id);

    if spaces_hook.is_loading {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">{"Loading spaces..."}</p>
            </div>
        };
    }

    if let Some(error) = &spaces_hook.error {
        return html! {
            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
            </div>
        };
    }

    match &spaces_hook.spaces {
        Some(spaces) => {
            if spaces.is_empty() {
                html! {
                    <div class="text-center py-12">
                        <p class="text-neutral-600 dark:text-neutral-400 mb-4">
                            {"No spaces have been created for this site yet."}
                        </p>
                        <button class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors">
                            {"Create First Space"}
                        </button>
                    </div>
                }
            } else {
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

                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                            {spaces.iter().map(|space| {
                                html! {
                                    <div key={space.space_id.to_string()} class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700">
                                        <div class="space-y-4">
                                            <div>
                                                <h3 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                                                    {&space.space_details.name}
                                                </h3>
                                                <div class="h-12">
                                                    {if let Some(description) = &space.space_details.description {
                                                        html! {
                                                            <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1 line-clamp-3">
                                                                {description}
                                                            </p>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }}
                                                </div>
                                            </div>

                                            <div class="text-sm text-neutral-600 dark:text-neutral-400 space-y-1">
                                                <p>{"Eligibility Points: "}{space.space_details.eligibility_points}</p>
                                                <p>{"Status: "}{if space.space_details.is_available { "Available" } else { "Unavailable" }}</p>
                                                <p>{"Created: "}{space.created_at.to_zoned(jiff::tz::TimeZone::system()).strftime("%B %d, %Y").to_string()}</p>
                                            </div>

                                            <div class="pt-2">
                                                <button class="w-full block text-center bg-neutral-100 hover:bg-neutral-200 dark:bg-neutral-700 dark:hover:bg-neutral-600 text-neutral-900 dark:text-neutral-100 px-4 py-2 rounded-md text-sm font-medium transition-colors">
                                                    {"View Space"}
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                }
                            }).collect::<Html>()}
                        </div>
                    </div>
                }
            }
        }
        None => {
            html! {
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">{"No spaces data available"}</p>
                </div>
            }
        }
    }
}
