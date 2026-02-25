use payloads::responses::Auction;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use crate::hooks::use_title;
use crate::{Route, State};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction: Auction,
    pub active_tab: ActiveTab,
}

#[derive(PartialEq, Clone)]
pub enum ActiveTab {
    Current,
    Rounds,
}

impl ActiveTab {
    fn label(&self) -> &'static str {
        match self {
            ActiveTab::Current => "Current",
            ActiveTab::Rounds => "Rounds",
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

    use_title(&format!(
        "{} Auction - {} - TinyLVT",
        site_name,
        props.active_tab.label()
    ));

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
                    {"Auction"}
                </h1>
                <p class="text-lg text-neutral-600 dark:text-neutral-400 mt-2">
                    {"Possession period: "}
                    {props.auction.auction_details.possession_start_at.to_string()}
                    {" to "}
                    {props.auction.auction_details.possession_end_at.to_string()}
                </p>
            </div>

            // Tab Navigation
            <div class="border-b border-neutral-200 dark:border-neutral-700">
                <nav class="-mb-px flex space-x-8">
                    <Link<Route>
                        to={Route::AuctionDetail { id: props.auction.auction_id }}
                        classes={classes!(format!(
                            "py-2 px-1 border-b-2 font-medium text-sm {}",
                            if props.active_tab == ActiveTab::Current {
                                "border-neutral-500 text-neutral-600 dark:text-neutral-400"
                            } else {
                                "border-transparent text-neutral-500 hover:text-neutral-700 hover:border-neutral-300 dark:text-neutral-400 dark:hover:text-neutral-300"
                            }
                        ))}
                    >
                        {"Current"}
                    </Link<Route>>
                    <Link<Route>
                        to={Route::AuctionRounds { id: props.auction.auction_id }}
                        classes={classes!(format!(
                            "py-2 px-1 border-b-2 font-medium text-sm {}",
                            if props.active_tab == ActiveTab::Rounds {
                                "border-neutral-500 text-neutral-600 dark:text-neutral-400"
                            } else {
                                "border-transparent text-neutral-500 hover:text-neutral-700 hover:border-neutral-300 dark:text-neutral-400 dark:hover:text-neutral-300"
                            }
                        ))}
                    >
                        {"Rounds"}
                    </Link<Route>>
                </nav>
            </div>
        </div>
    }
}
