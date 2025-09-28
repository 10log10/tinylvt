use payloads::responses::Site;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use crate::{Route, State};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site: Site,
    pub active_tab: ActiveTab,
}

#[derive(PartialEq, Clone)]
pub enum ActiveTab {
    Spaces,
    Auctions,
}

#[function_component]
pub fn SiteTabHeader(props: &Props) -> Html {
    let (state, _) = use_store::<State>();

    // Get the community information for the back link
    let community =
        state.get_community_by_id(props.site.site_details.community_id);
    let community_name =
        community.map(|c| c.name.as_str()).unwrap_or("Community");

    html! {
        <div class="space-y-8">
            // Back Navigation
            <Link<Route>
                to={Route::CommunityDetail { id: props.site.site_details.community_id }}
                classes="inline-flex items-center text-sm text-neutral-600 hover:text-neutral-800 dark:text-neutral-400 dark:hover:text-neutral-200"
            >
                {format!("← Back to {}", community_name)}
            </Link<Route>>

            // Header
            <div>
                <h1 class="text-3xl font-bold text-neutral-900 dark:text-neutral-100">
                    {&props.site.site_details.name}
                </h1>
                {if let Some(description) = &props.site.site_details.description {
                    html! {
                        <p class="text-lg text-neutral-600 dark:text-neutral-400 mt-2">
                            {description}
                        </p>
                    }
                } else {
                    html! {}
                }}
            </div>

            // Tab Navigation
            <div class="border-b border-neutral-200 dark:border-neutral-700">
                <nav class="-mb-px flex space-x-8">
                    <Link<Route>
                        to={Route::SiteDetail { id: props.site.site_id }}
                        classes={classes!(format!(
                            "py-2 px-1 border-b-2 font-medium text-sm {}",
                            if props.active_tab == ActiveTab::Spaces {
                                "border-neutral-500 text-neutral-600 dark:text-neutral-400"
                            } else {
                                "border-transparent text-neutral-500 hover:text-neutral-700 hover:border-neutral-300 dark:text-neutral-400 dark:hover:text-neutral-300"
                            }
                        ))}
                    >
                        {"Spaces"}
                    </Link<Route>>
                    <Link<Route>
                        to={Route::SiteAuctions { id: props.site.site_id }}
                        classes={classes!(format!(
                            "py-2 px-1 border-b-2 font-medium text-sm {}",
                            if props.active_tab == ActiveTab::Auctions {
                                "border-neutral-500 text-neutral-600 dark:text-neutral-400"
                            } else {
                                "border-transparent text-neutral-500 hover:text-neutral-700 hover:border-neutral-300 dark:text-neutral-400 dark:hover:text-neutral-300"
                            }
                        ))}
                    >
                        {"Auctions"}
                    </Link<Route>>
                </nav>
            </div>
        </div>
    }
}
