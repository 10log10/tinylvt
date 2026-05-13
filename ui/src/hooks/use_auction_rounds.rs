use payloads::{AuctionId, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{
    SubscribedEvent, SubscribedFetchHookReturn, use_subscribed_fetch,
};

/// Hook to fetch and manage rounds for a specific auction.
///
/// Subscribed to `RoundCreated`. Round rows themselves don't change after
/// creation — `round_space_results` are tracked separately — so RoundEnded
/// isn't needed here. This hook does not cache in global state since rounds
/// are only used in specific views.
#[hook]
pub fn use_auction_rounds(
    auction_id: AuctionId,
) -> SubscribedFetchHookReturn<Vec<responses::AuctionRound>> {
    use_subscribed_fetch(
        auction_id,
        auction_id,
        &[SubscribedEvent::RoundCreated],
        move || async move {
            let api_client = get_api_client();
            api_client
                .list_auction_rounds(&auction_id)
                .await
                .map_err(|e| e.to_string())
        },
    )
}
