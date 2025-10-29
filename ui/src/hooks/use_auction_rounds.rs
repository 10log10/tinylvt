use payloads::{AuctionId, responses};
use yew::prelude::*;

use crate::get_api_client;

/// Hook return type for auction rounds data
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
pub struct AuctionRoundsHookReturn {
    pub rounds: Option<Vec<responses::AuctionRound>>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
}

/// Hook to fetch and manage rounds for a specific auction
///
/// This hook does not cache in global state since rounds are
/// only used in specific views and can change frequently during
/// an active auction.
#[hook]
pub fn use_auction_rounds(auction_id: AuctionId) -> AuctionRoundsHookReturn {
    let rounds = use_state(|| None);
    let error = use_state(|| None);
    let is_loading = use_state(|| false);

    let refetch = {
        let rounds = rounds.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(auction_id, move |auction_id, _| {
            let rounds = rounds.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.list_auction_rounds(&auction_id).await {
                    Ok(fetched_rounds) => {
                        rounds.set(Some(fetched_rounds));
                        error.set(None);
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    // Auto-load rounds on mount
    {
        let refetch = refetch.clone();
        let rounds = rounds.clone();
        let is_loading = is_loading.clone();

        use_effect_with(auction_id, move |auction_id| {
            if rounds.is_none() && !*is_loading {
                refetch.emit(*auction_id);
            }
        });
    }

    let current_rounds = (*rounds).clone();
    let current_error = (*error).clone();
    let current_is_loading =
        *is_loading || (current_rounds.is_none() && current_error.is_none());

    AuctionRoundsHookReturn {
        rounds: current_rounds,
        error: current_error,
        is_loading: current_is_loading,
        refetch: Callback::from(move |_| refetch.emit(auction_id)),
    }
}
