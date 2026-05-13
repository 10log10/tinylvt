use payloads::{AuctionId, AuctionRoundId, SpaceId};
use std::collections::HashSet;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{
    SubscribedEvent, SubscribedFetchHookReturn, use_subscribed_fetch,
};

/// Hook to fetch the current user's bids for a specific round.
///
/// Returns a set of space IDs that the user has placed bids on in the round.
/// This is used to show which spaces have active bids and disable the bid
/// button for those spaces.
///
/// Subscribed to `BidsChanged`, which is emitted on every change to this
/// user's bids — both proxy bids placed by the scheduler and manual bids
/// placed or removed via `create_bid` / `delete_bid`. The event is
/// server-side filtered to this user, so other bidders' activity doesn't
/// trigger a refetch.
#[hook]
pub fn use_user_bids(
    auction_id: AuctionId,
    round_id: AuctionRoundId,
) -> SubscribedFetchHookReturn<HashSet<SpaceId>> {
    use_subscribed_fetch(
        round_id,
        auction_id,
        &[SubscribedEvent::BidsChanged],
        move || async move {
            let api_client = get_api_client();
            let bids = api_client
                .list_bids(&round_id)
                .await
                .map_err(|e| e.to_string())?;

            let bids_set: HashSet<SpaceId> =
                bids.iter().map(|bid| bid.space_id).collect();
            Ok(bids_set)
        },
    )
}
