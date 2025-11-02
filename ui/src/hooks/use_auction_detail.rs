use payloads::{AuctionId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{State, get_api_client};

/// Hook return type for single auction data
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[allow(dead_code)]
pub struct AuctionDetailHookReturn {
    pub auction: Option<responses::Auction>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
}

/// Hook to fetch and manage a single auction by ID
///
/// This hook checks if the auction exists in global state first, and only
/// fetches from the API if it's not found or explicitly refetched.
#[hook]
pub fn use_auction_detail(auction_id: AuctionId) -> AuctionDetailHookReturn {
    let (state, dispatch) = use_store::<State>();
    let auction = use_state(|| None);
    let error = use_state(|| None);
    let is_loading = use_state(|| false);

    let refetch = {
        let dispatch = dispatch.clone();
        let auction = auction.clone();
        let error = error.clone();

        use_callback(auction_id, move |auction_id, _| {
            let dispatch = dispatch.clone();
            let auction = auction.clone();
            let error = error.clone();

            yew::platform::spawn_local(async move {
                error.set(None);

                let api_client = get_api_client();
                match api_client.get_auction(&auction_id).await {
                    Ok(fetched_auction) => {
                        dispatch.reduce_mut(|state| {
                            state
                                .individual_auctions
                                .insert(auction_id, fetched_auction.clone());
                        });
                        auction.set(Some(fetched_auction));
                        error.set(None);
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }
            });
        })
    };

    // Auto-load auction if not already loaded and user is authenticated
    {
        let refetch = refetch.clone();
        let state = state.clone();
        let auction = auction.clone();
        let is_loading = is_loading.clone();

        use_effect_with(
            (state.auth_state.clone(), auction_id),
            move |(_, auction_id)| {
                if state.is_authenticated()
                    && !state.individual_auctions.contains_key(auction_id)
                    && auction.is_none()
                    && !*is_loading
                {
                    refetch.emit(*auction_id);
                }
            },
        );
    }

    // Get auction from state if available
    let current_auction = if let Some(auction) =
        state.individual_auctions.get(&auction_id).cloned()
    {
        Some(auction)
    } else {
        (*auction).clone()
    };

    let current_error = (*error).clone();
    let current_is_loading =
        *is_loading || (current_auction.is_none() && current_error.is_none());

    AuctionDetailHookReturn {
        auction: current_auction,
        error: current_error,
        is_loading: current_is_loading,
        refetch: Callback::from(move |_| refetch.emit(auction_id)),
    }
}
