use payloads::CommunityId;
use yew::prelude::*;

use crate::components::{ActiveTab, CommunityPageWrapper};
use crate::hooks::use_sites;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: String,
}

#[function_component]
pub fn CommunityDetailPage(props: &Props) -> Html {
    let render_content = Callback::from(|community_id: CommunityId| {
        html! { <SitesTab community_id={community_id} /> }
    });

    html! {
        <CommunityPageWrapper
            community_id={props.community_id.clone()}
            active_tab={ActiveTab::Sites}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
pub struct SitesTabProps {
    pub community_id: CommunityId,
}

#[function_component]
fn SitesTab(props: &SitesTabProps) -> Html {
    let sites_hook = use_sites(props.community_id);

    if sites_hook.is_loading {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">{"Loading sites..."}</p>
            </div>
        };
    }

    if let Some(error) = &sites_hook.error {
        return html! {
            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
            </div>
        };
    }

    match &sites_hook.sites {
        Some(sites) => {
            if sites.is_empty() {
                html! {
                    <div class="text-center py-12">
                        <p class="text-neutral-600 dark:text-neutral-400 mb-4">
                            {"No sites have been created for this community yet."}
                        </p>
                        <button class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors">
                            {"Create First Site"}
                        </button>
                    </div>
                }
            } else {
                html! {
                    <div>
                        <div class="flex justify-between items-center mb-6">
                            <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                                {"Sites"}
                            </h2>
                            <button class="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors">
                                {"Create New Site"}
                            </button>
                        </div>

                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                            {sites.iter().map(|site| {
                                html! {
                                    <div key={site.site_id.to_string()} class="bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700">
                                        <div class="space-y-4">
                                            <div>
                                                <h3 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                                                    {&site.site_details.name}
                                                </h3>
                                                <div class="h-12">
                                                    {if let Some(description) = &site.site_details.description {
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

                                            <div class="text-sm text-neutral-600 dark:text-neutral-400">
                                                <p>{"Created: "}{site.created_at.to_zoned(jiff::tz::TimeZone::system()).strftime("%B %d, %Y").to_string()}</p>
                                            </div>

                                            <div class="pt-2">
                                                <button class="w-full bg-neutral-100 hover:bg-neutral-200 dark:bg-neutral-700 dark:hover:bg-neutral-600 text-neutral-900 dark:text-neutral-100 px-4 py-2 rounded-md text-sm font-medium transition-colors">
                                                    {"View Site"}
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
                    <p class="text-neutral-600 dark:text-neutral-400">{"No sites data available"}</p>
                </div>
            }
        }
    }
}
