use payloads::{AuctionId, SpaceCategoryId, responses};
use std::collections::HashMap;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{
    FetchHookReturn, SubscribedEvent, SubscribedFetchHookReturn, use_fetch,
    use_subscribed_fetch,
};

/// The current user's effective cap points per category bucket in a
/// capped auction (None = the uncategorized bucket), with backed
/// delegations already applied. A missing bucket means 0: the user cannot
/// bid on its spaces at all.
pub type MyCapsMap = HashMap<Option<SpaceCategoryId>, f64>;

/// Hook to fetch the current user's own effective caps in an auction.
/// Subscribed to `BidderCapsChanged`, which is server-side filtered to
/// this user, so coleader edits, seeding applies, and delegation changes
/// show up live.
#[hook]
pub fn use_my_bidder_caps(
    auction_id: AuctionId,
) -> SubscribedFetchHookReturn<MyCapsMap> {
    use_subscribed_fetch(
        auction_id,
        auction_id,
        &[SubscribedEvent::BidderCapsChanged],
        move || async move {
            let api_client = get_api_client();
            api_client
                .my_bidder_caps(&auction_id)
                .await
                .map(|caps| {
                    caps.into_iter()
                        .map(|c| (c.category_id, c.points))
                        .collect::<MyCapsMap>()
                })
                .map_err(|e| e.to_string())
        },
    )
}

/// Hook to fetch all cap rows of an auction (coleader+), for the cap
/// editor. Not SSE-subscribed: the cap-change event is scoped to the
/// affected bidder, and the editing coleader refetches after their own
/// mutations instead.
#[hook]
pub fn use_bidder_caps(
    auction_id: AuctionId,
) -> FetchHookReturn<Vec<responses::BidderCap>> {
    use_fetch(auction_id, move || async move {
        let api_client = get_api_client();
        api_client
            .list_bidder_caps(&auction_id)
            .await
            .map_err(|e| e.to_string())
    })
}

/// Hook to fetch the delegations the current user gave or received in an
/// auction. Subscribed to `BidderCapsChanged` like the caps themselves:
/// the other party's edits and cap changes that shift backing both emit
/// it for this user.
#[hook]
pub fn use_my_cap_delegations(
    auction_id: AuctionId,
) -> SubscribedFetchHookReturn<Vec<responses::CapDelegation>> {
    use_subscribed_fetch(
        auction_id,
        auction_id,
        &[SubscribedEvent::BidderCapsChanged],
        move || async move {
            let api_client = get_api_client();
            api_client
                .my_cap_delegations(&auction_id)
                .await
                .map_err(|e| e.to_string())
        },
    )
}

/// Hook to fetch all delegations of an auction (coleader+), for the
/// read-only admin list. Not SSE-subscribed, like [`use_bidder_caps`].
#[hook]
pub fn use_cap_delegations(
    auction_id: AuctionId,
) -> FetchHookReturn<Vec<responses::CapDelegation>> {
    use_fetch(auction_id, move || async move {
        let api_client = get_api_client();
        api_client
            .list_cap_delegations(&auction_id)
            .await
            .map_err(|e| e.to_string())
    })
}
