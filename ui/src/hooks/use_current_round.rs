use payloads::{AuctionId, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{FetchHookReturn, use_fetch};

/// Hook to fetch the current (or most recent) round for an auction
///
/// This fetches all rounds and returns the latest one (highest round_num).
/// If the auction has not started yet (no rounds exist), data will be
/// `FetchState::Fetched(None)` with no error.
///
/// The data field uses `FetchState<Option<AuctionRound>>` to distinguish:
/// - `NotFetched`: Data not fetched yet / still loading
/// - `Fetched(None)`: Successfully fetched, but no current round exists
/// - `Fetched(Some(round))`: Successfully fetched with a current round
#[hook]
pub fn use_current_round(
    auction_id: AuctionId,
) -> FetchHookReturn<Option<responses::AuctionRound>> {
    use_fetch(auction_id, move || async move {
        let api_client = get_api_client();
        let rounds = api_client
            .list_auction_rounds(&auction_id)
            .await
            .map_err(|e| e.to_string())?;

        // Get the latest round (highest round_num)
        let latest =
            rounds.into_iter().max_by_key(|r| r.round_details.round_num);
        Ok(latest)
    })
}
