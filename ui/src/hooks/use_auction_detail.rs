use payloads::{AuctionId, responses};
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{SubscribedEvent, SubscribedFetchHookReturn, use_subscribed_fetch},
};

/// Hook to fetch and manage a single auction by ID.
///
/// Subscribed to `AuctionEnded` so the auction's `end_at` becomes visible
/// the moment the auction concludes (or is cancelled). Always fetches fresh
/// from the API; doesn't read from yewdux. The yewdux `individual_auctions`
/// cache is still populated by `use_auctions` (the per-site list) but isn't
/// consulted here — that cache isn't SSE-subscribed and could be stale,
/// which conflicts with this hook's "always reflects current state"
/// contract.
#[hook]
pub fn use_auction_detail(
    auction_id: AuctionId,
) -> SubscribedFetchHookReturn<responses::Auction> {
    use_subscribed_fetch(
        auction_id,
        auction_id,
        &[SubscribedEvent::AuctionEnded],
        move || async move {
            let api_client = get_api_client();
            api_client
                .get_auction(&auction_id)
                .await
                .map_err(|e| e.to_string())
        },
    )
}
