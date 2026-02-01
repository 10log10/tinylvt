use payloads::{AuctionId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{
    State, get_api_client,
    hooks::{FetchHookReturn, use_fetch_with_cache},
};

/// Hook to fetch and manage a single auction by ID
///
/// This hook checks if the auction exists in global state first, and only
/// fetches from the API if it's not found or explicitly refetched.
#[hook]
pub fn use_auction_detail(
    auction_id: AuctionId,
) -> FetchHookReturn<responses::Auction> {
    let (state, dispatch) = use_store::<State>();

    let get_cached_state = state.clone();
    let should_fetch_state = state.clone();
    let fetch_dispatch = dispatch.clone();

    use_fetch_with_cache(
        auction_id,
        move || {
            get_cached_state
                .individual_auctions
                .get(&auction_id)
                .cloned()
        },
        move || {
            !should_fetch_state
                .individual_auctions
                .contains_key(&auction_id)
        },
        move || {
            let dispatch = fetch_dispatch.clone();
            async move {
                let api_client = get_api_client();
                let auction = api_client
                    .get_auction(&auction_id)
                    .await
                    .map_err(|e| e.to_string())?;
                dispatch.reduce_mut(|s| {
                    s.individual_auctions.insert(auction_id, auction.clone());
                });
                Ok(auction)
            }
        },
    )
}
