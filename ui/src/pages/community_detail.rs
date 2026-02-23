use payloads::{CommunityId, responses::CommunityWithRole};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::{
    ActiveTab, CommunityPageWrapper, CommunityTabHeader, MarkdownText,
};
use crate::hooks::use_sites;
use crate::{Route, get_api_client};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
}

#[function_component]
pub fn CommunityDetailPage(props: &Props) -> Html {
    let render_content = Callback::from(|community: CommunityWithRole| {
        html! {
            <div>
                <CommunityTabHeader community={community.clone()} active_tab={ActiveTab::Sites} />
                <div class="py-6">
                    <SitesTab community_id={community.id} />

                    // Community description (if present)
                    {if let Some(desc) = &community.community.description {
                        html! {
                            <div class="mt-8 bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700">
                                <div class="prose prose-sm dark:prose-invert max-w-none">
                                    <MarkdownText text={desc.clone()} />
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }}
                </div>
            </div>
        }
    });

    html! {
        <CommunityPageWrapper
            community_id={props.community_id}
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
    let show_deleted = use_state(|| false);

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

    match sites_hook.data.as_ref() {
        Some(sites) => {
            // Filter sites based on show_deleted toggle
            let displayed_sites: Vec<_> = sites
                .iter()
                .filter(|site| *show_deleted || site.deleted_at.is_none())
                .collect();
            let any_site_has_image =
                sites.iter().any(|s| s.site_details.site_image_id.is_some());

            html! {
                <div>
                    // Header - always shown
                    <div class="flex justify-between items-center mb-6">
                        <h2 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                            {"Sites"}
                        </h2>
                        <Link<Route>
                            to={Route::CreateSite { id: props.community_id }}
                            classes="bg-neutral-900 hover:bg-neutral-800 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-200 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors"
                        >
                            {"Create New Site"}
                        </Link<Route>>
                    </div>

                    // Toggle - always shown
                    <div class="mb-4 flex items-center">
                        <input
                            type="checkbox"
                            id="show-deleted-sites"
                            class="mr-2 h-4 w-4 rounded border-neutral-300 dark:border-neutral-600 text-neutral-900 dark:text-neutral-100 focus:ring-neutral-500"
                            checked={*show_deleted}
                            onclick={{
                                let show_deleted = show_deleted.clone();
                                Callback::from(move |_| show_deleted.set(!*show_deleted))
                            }}
                        />
                        <label
                            for="show-deleted-sites"
                            class="text-sm text-neutral-700 dark:text-neutral-300 select-none cursor-pointer"
                        >
                            {"Show deleted sites"}
                        </label>
                    </div>

                    // Content area
                    {if sites.is_empty() {
                        // No sites exist at all
                        html! {
                            <div class="text-center py-12">
                                <p class="text-neutral-600 dark:text-neutral-400">
                                    {"No sites have been created for this community yet."}
                                </p>
                            </div>
                        }
                    } else if displayed_sites.is_empty() {
                        // All sites are deleted and toggle is off
                        html! {
                            <div class="text-center py-12">
                                <p class="text-neutral-600 dark:text-neutral-400">
                                    {"All sites have been deleted."}
                                </p>
                            </div>
                        }
                    } else {
                        // Show the site grid
                        html! {
                            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                                {displayed_sites.iter().map(|site| {
                                let is_deleted = site.deleted_at.is_some();
                                let card_class = if is_deleted {
                                    "bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700 opacity-50 relative"
                                } else {
                                    "bg-white dark:bg-neutral-800 p-6 rounded-lg shadow-md border border-neutral-200 dark:border-neutral-700 relative"
                                };

                                // Build image URL if image exists
                                let image_url = site.site_details.site_image_id.map(|id| {
                                    get_api_client().site_image_url(&id)
                                });

                                html! {
                                    <div key={site.site_id.to_string()} class={card_class}>
                                        {if is_deleted {
                                            html! {
                                                <div class="absolute top-2 right-2 z-10">
                                                    <span class="inline-flex items-center px-2 py-1 rounded text-xs font-medium bg-red-100 dark:bg-red-900/30 text-red-800 dark:text-red-400 border border-red-200 dark:border-red-800">
                                                        {"Deleted"}
                                                    </span>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }}

                                        // Only show image section if any site has an image
                                        {if any_site_has_image {
                                            html! {
                                                <div class="aspect-video w-full overflow-hidden
                                                            rounded-t-lg -mx-6 -mt-6 mb-4"
                                                     style="width: calc(100% + 3rem);">
                                                    {if let Some(src) = &image_url {
                                                        html! {
                                                            <img
                                                                src={src.clone()}
                                                                alt={format!("{} image", site.site_details.name)}
                                                                class="w-full h-full object-cover"
                                                            />
                                                        }
                                                    } else {
                                                        html! {
                                                            <div class="w-full h-full bg-neutral-100
                                                                        dark:bg-neutral-700" />
                                                        }
                                                    }}
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }}

                                        <div class="space-y-4">
                                            <div>
                                                <h3 class="text-xl font-semibold text-neutral-900 dark:text-neutral-100">
                                                    {&site.site_details.name}
                                                </h3>
                                                <div class="h-16">
                                                    {if let Some(description) = &site.site_details.description {
                                                        // Show only the first line, but allow wrapping
                                                        let first_line = description
                                                            .lines()
                                                            .next()
                                                            .unwrap_or("");
                                                        html! {
                                                            <p class="text-sm text-neutral-600 dark:text-neutral-400 mt-1 line-clamp-3">
                                                                {first_line}
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
                                                <Link<Route>
                                                    to={Route::SiteOverview { id: site.site_id }}
                                                    classes="w-full block text-center bg-neutral-100 hover:bg-neutral-200 dark:bg-neutral-700 dark:hover:bg-neutral-600 text-neutral-900 dark:text-neutral-100 px-4 py-2 rounded-md text-sm font-medium transition-colors"
                                                >
                                                    {"View Site"}
                                                </Link<Route>>
                                            </div>
                                        </div>
                                    </div>
                                }
                            }).collect::<Html>()}
                            </div>
                        }
                    }}
                </div>
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
