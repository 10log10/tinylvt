use payloads::{
    AuctionId, CurrencySettings,
    responses::{Auction, CommunityWithRole, Site, UserProfile},
};
use yew::prelude::*;

use crate::components::RequireAuth;
use crate::hooks::{
    render_section, use_auction_detail, use_communities, use_site,
};

/// Context provided to children of AuctionPageWrapper containing all the
/// validated data needed to render an auction page.
#[derive(Clone, PartialEq)]
pub struct AuctionContext {
    pub auction: Auction,
    pub site: Site,
    pub community: CommunityWithRole,
    /// The signed-in user. Threaded through `RequireAuth`'s render prop so
    /// children have access to the resolved profile (id, username, display
    /// name) without needing to read auth state separately. Use `user_id`
    /// for identity comparisons.
    pub current_user: UserProfile,
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
    let auction_id = props.auction_id;
    let children = props.children.clone();
    let render = Callback::from(move |current_user: UserProfile| {
        html! {
            <AuctionLoader
                auction_id={auction_id}
                current_user={current_user}
                children={children.clone()}
            />
        }
    });
    html! { <RequireAuth render={render} /> }
}

#[derive(Properties, PartialEq)]
struct LoaderProps {
    auction_id: AuctionId,
    current_user: UserProfile,
    children: Callback<AuctionContext, Html>,
}

// Step 1: Load auction
#[function_component]
fn AuctionLoader(props: &LoaderProps) -> Html {
    let auction_hook = use_auction_detail(props.auction_id);
    let current_user = props.current_user.clone();
    let children = props.children.clone();

    render_section(
        &auction_hook.inner,
        "auction",
        move |auction, _is_loading, _errors| {
            html! {
                <SiteLoader
                    auction={auction.clone()}
                    current_user={current_user.clone()}
                    children={children.clone()}
                />
            }
        },
    )
}

// Step 2: Load site (depends on auction.site_id)
#[derive(Properties, PartialEq)]
struct SiteLoaderProps {
    auction: Auction,
    current_user: UserProfile,
    children: Callback<AuctionContext, Html>,
}

#[function_component]
fn SiteLoader(props: &SiteLoaderProps) -> Html {
    let site_hook = use_site(props.auction.auction_details.site_id);
    let communities_hook = use_communities();
    let auction = props.auction.clone();
    let current_user = props.current_user.clone();
    let children = props.children.clone();

    render_section(
        &site_hook.inner,
        "site",
        move |site, _is_loading, _errors| {
            let community_id = site.site_details.community_id;

            render_section(&communities_hook.inner, "community membership", {
                let auction = auction.clone();
                let site = site.clone();
                let current_user = current_user.clone();
                let children = children.clone();

                move |communities, _is_loading, _errors| {
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
                                current_user: current_user.clone(),
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
        },
    )
}
