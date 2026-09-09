use payloads::{Role, responses::Auction};
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use crate::hooks::use_title;
use crate::{Route, State};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction: Auction,
    pub user_role: Role,
    pub active_tab: ActiveTab,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ActiveTab {
    Current,
    Rounds,
    /// Coleader-only auction management; the tab is hidden for other
    /// roles.
    Settings,
}

impl ActiveTab {
    fn label(&self) -> &'static str {
        match self {
            ActiveTab::Current => "Current",
            ActiveTab::Rounds => "Rounds",
            ActiveTab::Settings => "Settings",
        }
    }

    fn route(&self, auction_id: payloads::AuctionId) -> Route {
        match self {
            ActiveTab::Current => Route::AuctionDetail { id: auction_id },
            ActiveTab::Rounds => Route::AuctionRounds { id: auction_id },
            ActiveTab::Settings => Route::AuctionSettings { id: auction_id },
        }
    }
}

#[function_component]
pub fn AuctionTabHeader(props: &Props) -> Html {
    let (state, _) = use_store::<State>();

    // Get the site information for the back link and title
    let site_id = props.auction.auction_details.site_id;
    let site = state.get_site(site_id);
    let site_name =
        site.map(|s| s.site_details.name.as_str()).unwrap_or("Site");

    // The auction's own name takes over the heading and title when set;
    // unnamed auctions fall back to the generic site-based heading.
    let auction_name = props.auction.auction_details.name.as_deref();
    let heading = auction_name.unwrap_or("Auction").to_string();

    use_title(&format!(
        "{} - {} - TinyLVT",
        auction_name
            .map(String::from)
            .unwrap_or_else(|| format!("{} Auction", site_name)),
        props.active_tab.label()
    ));

    let tabs = [ActiveTab::Current, ActiveTab::Rounds, ActiveTab::Settings]
        .into_iter()
        .filter(|tab| {
            *tab != ActiveTab::Settings || props.user_role.is_ge_coleader()
        });

    html! {
        <div class="space-y-8">
            // Back Navigation
            <Link<Route>
                to={Route::SiteAuctions { id: site_id }}
                classes="inline-flex items-center text-sm text-neutral-600 hover:text-neutral-800 dark:text-neutral-400 dark:hover:text-neutral-200"
            >
                {format!("← Back to {} Auctions", site_name)}
            </Link<Route>>

            // Header
            <div>
                <h1 class="text-3xl font-bold text-neutral-900 dark:text-neutral-100">
                    {heading}
                </h1>
            </div>

            // Tab Navigation
            <div class="border-b border-neutral-200 dark:border-neutral-700">
                <nav class="-mb-px flex space-x-8">
                    {tabs.map(|tab| html! {
                        <Link<Route>
                            key={tab.label()}
                            to={tab.route(props.auction.auction_id)}
                            classes={classes!(format!(
                                "py-2 px-1 border-b-2 font-medium text-sm {}",
                                if props.active_tab == tab {
                                    "border-neutral-500 text-neutral-600 dark:text-neutral-400"
                                } else {
                                    "border-transparent text-neutral-500 hover:text-neutral-700 hover:border-neutral-300 dark:text-neutral-400 dark:hover:text-neutral-300"
                                }
                            ))}
                        >
                            {tab.label()}
                        </Link<Route>>
                    }).collect::<Html>()}
                </nav>
            </div>
        </div>
    }
}
