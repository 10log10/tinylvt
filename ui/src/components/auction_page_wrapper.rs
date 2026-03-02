use payloads::{
    AuctionId, CurrencySettings,
    responses::{Auction, CommunityWithRole, Site},
};
use yew::prelude::*;

use crate::components::RequireAuth;
use crate::hooks::{use_auction_detail, use_communities, use_site};

/// Context provided to children of AuctionPageWrapper containing all the
/// validated data needed to render an auction page.
#[derive(Clone, PartialEq)]
pub struct AuctionContext {
    pub auction: Auction,
    pub site: Site,
    pub community: CommunityWithRole,
    /// Callback to refetch the auction data
    pub refetch_auction: Callback<()>,
}

impl AuctionContext {
    /// Get the currency settings for this auction's community
    pub fn currency(&self) -> &CurrencySettings {
        &self.community.community.currency
    }

    /// Get the site timezone if set
    pub fn site_timezone(&self) -> Option<&str> {
        self.site.site_details.timezone.as_deref()
    }
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction_id: AuctionId,
    pub children: Callback<AuctionContext, Html>,
}

/// Wrapper component that loads auction, site, and community data before
/// rendering children. Shows loading/error states appropriately.
#[function_component]
pub fn AuctionPageWrapper(props: &Props) -> Html {
    html! {
        <RequireAuth>
            <AuctionLoader
                auction_id={props.auction_id}
                children={props.children.clone()}
            />
        </RequireAuth>
    }
}

// Step 1: Load auction
#[function_component]
fn AuctionLoader(props: &Props) -> Html {
    let auction_hook = use_auction_detail(props.auction_id);
    let refetch = auction_hook.refetch.clone();
    let children = props.children.clone();

    auction_hook.render("auction", move |auction, _is_loading, _error| {
        html! {
            <SiteLoader
                auction={auction.clone()}
                refetch_auction={refetch.clone()}
                children={children.clone()}
            />
        }
    })
}

// Step 2: Load site (depends on auction.site_id)
#[derive(Properties, PartialEq)]
struct SiteLoaderProps {
    auction: Auction,
    refetch_auction: Callback<()>,
    children: Callback<AuctionContext, Html>,
}

#[function_component]
fn SiteLoader(props: &SiteLoaderProps) -> Html {
    let site_hook = use_site(props.auction.auction_details.site_id);
    let communities_hook = use_communities();
    let auction = props.auction.clone();
    let refetch_auction = props.refetch_auction.clone();
    let children = props.children.clone();

    site_hook.render("site", move |site, _is_loading, _error| {
        let community_id = site.site_details.community_id;

        communities_hook.render("community membership", {
            let auction = auction.clone();
            let site = site.clone();
            let refetch_auction = refetch_auction.clone();
            let children = children.clone();

            move |communities, _is_loading, _error| {
                let community = communities
                    .iter()
                    .find(|c| c.community.id == community_id)
                    .cloned();

                match community {
                    Some(community) => {
                        let context = AuctionContext {
                            auction: auction.clone(),
                            site: site.clone(),
                            community,
                            refetch_auction: refetch_auction.clone(),
                        };
                        html! {
                            <div>
                                {children.emit(context)}
                            </div>
                        }
                    }
                    None => {
                        html! {
                            <div class="text-center py-12">
                                <p class="text-neutral-600 dark:text-neutral-400">
                                    {"Unable to verify community membership"}
                                </p>
                            </div>
                        }
                    }
                }
            }
        })
    })
}
