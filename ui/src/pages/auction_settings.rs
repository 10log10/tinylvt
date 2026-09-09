use jiff::Timestamp;
use payloads::{
    AuctionId, AuctionStatus, CommunityId, Role, responses::Auction,
};
use yew::prelude::*;

use crate::components::{
    AuctionAdminControls, AuctionContext, AuctionPageWrapper, AuctionTabHeader,
    BidderCapsEditor, CapDelegationsAdminList, ProxyBiddingParticipants,
    auction_tab_header::ActiveTab,
};
use crate::hooks::use_space_categories;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub auction_id: AuctionId,
}

/// Coleader-facing auction management: lifecycle controls, bidder caps,
/// every delegation, and who has enabled proxy bidding. Everything a
/// member needs for their own bidding stays on the Current tab.
#[function_component]
pub fn AuctionSettingsPage(props: &Props) -> Html {
    let render_content = Callback::from(|ctx: AuctionContext| {
        html! {
            <div>
                <AuctionTabHeader
                    auction={ctx.auction.clone()}
                    user_role={ctx.community.user_role}
                    active_tab={ActiveTab::Settings}
                />
                <div class="py-6">
                    <AuctionSettingsContent
                        auction={ctx.auction.clone()}
                        user_role={ctx.community.user_role}
                        community_id={ctx.community.community.id}
                    />
                </div>
            </div>
        }
    });

    html! {
        <AuctionPageWrapper
            auction_id={props.auction_id}
            children={render_content}
        />
    }
}

#[derive(Properties, PartialEq)]
struct ContentProps {
    auction: Auction,
    user_role: Role,
    community_id: CommunityId,
}

#[function_component]
fn AuctionSettingsContent(props: &ContentProps) -> Html {
    let categories_hook = use_space_categories(props.community_id);

    if !props.user_role.is_ge_coleader() {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">
                    {"Only community leaders can manage this auction."}
                </p>
            </div>
        };
    }

    let not_started = matches!(
        props.auction.status(Timestamp::now()),
        AuctionStatus::NotScheduled | AuctionStatus::Upcoming
    );

    html! {
        <div class="space-y-6">
            <AuctionAdminControls
                auction={props.auction.clone()}
                user_role={props.user_role}
            />
            <BidderCapsEditor
                auction={props.auction.clone()}
                community_id={props.community_id}
                user_role={props.user_role}
                categories={categories_hook.inner.clone()}
            />
            <CapDelegationsAdminList
                auction={props.auction.clone()}
                community_id={props.community_id}
                user_role={props.user_role}
                categories={categories_hook.inner.clone()}
            />
            // Who has opted into proxy bidding only matters while members
            // can still be nudged to set it up.
            {if not_started {
                html! {
                    <ProxyBiddingParticipants
                        auction_id={props.auction.auction_id}
                        user_role={props.user_role}
                    />
                }
            } else {
                html! {}
            }}
        </div>
    }
}
