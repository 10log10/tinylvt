use payloads::{AuctionRoundId, SpaceId};
use std::collections::HashSet;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{FetchHookReturn, use_fetch};

/// Hook to fetch the user's bids for a specific round
///
/// Returns a set of space IDs that the user has placed bids on in the round.
/// This is used to show which spaces have active bids and disable the bid
/// button for those spaces.
#[hook]
pub fn use_user_bids(
    round_id: AuctionRoundId,
) -> FetchHookReturn<HashSet<SpaceId>> {
    use_fetch(round_id, move || async move {
        let api_client = get_api_client();
        let bids = api_client
            .list_bids(&round_id)
            .await
            .map_err(|e| e.to_string())?;

        // Extract space IDs from the bids
        let bids_set: HashSet<SpaceId> =
            bids.iter().map(|bid| bid.space_id).collect();
        Ok(bids_set)
    })
}
