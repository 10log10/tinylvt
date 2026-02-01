use payloads::{AuctionId, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{FetchHookReturn, use_fetch};

/// Hook to fetch and manage rounds for a specific auction
///
/// This hook does not cache in global state since rounds are
/// only used in specific views and can change frequently during
/// an active auction.
#[hook]
pub fn use_auction_rounds(
    auction_id: AuctionId,
) -> FetchHookReturn<Vec<responses::AuctionRound>> {
    use_fetch(auction_id, move || async move {
        let api_client = get_api_client();
        api_client
            .list_auction_rounds(&auction_id)
            .await
            .map_err(|e| e.to_string())
    })
}
