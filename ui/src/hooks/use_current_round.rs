use payloads::{AuctionId, responses};
use yew::prelude::*;

use crate::get_api_client;

/// Hook return type for current round data
///
/// The `current_round` field uses a nested Option to distinguish fetch state:
/// - `None`: Data not fetched yet / still loading
/// - `Some(None)`: Successfully fetched, but no current round exists
/// - `Some(Some(round))`: Successfully fetched with a current round
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[derive(Debug)]
#[allow(dead_code)]
pub struct CurrentRoundHookReturn {
    pub current_round: Option<Option<responses::AuctionRound>>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
}

/// Hook to fetch the current (or most recent) round for an auction
///
/// This fetches all rounds and returns the latest one (highest round_num).
/// If the auction has not started yet (no rounds exist), `current_round` will
/// be `Some(None)` with no error.
#[hook]
pub fn use_current_round(auction_id: AuctionId) -> CurrentRoundHookReturn {
    let current_round = use_state(|| None);
    let error = use_state(|| None);
    let is_loading = use_state(|| false);

    let refetch = {
        let current_round = current_round.clone();
        let error = error.clone();

        use_callback(auction_id, move |auction_id, _| {
            let current_round = current_round.clone();
            let error = error.clone();

            yew::platform::spawn_local(async move {
                error.set(None);

                let api_client = get_api_client();
                match api_client.list_auction_rounds(&auction_id).await {
                    Ok(rounds) => {
                        // Get the latest round (highest round_num)
                        let latest = rounds
                            .into_iter()
                            .max_by_key(|r| r.round_details.round_num);
                        // Wrap in Some to indicate we've fetched the data
                        current_round.set(Some(latest));
                        error.set(None);
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }
            });
        })
    };

    // Auto-load current round on mount
    {
        let refetch = refetch.clone();
        let current_round = current_round.clone();
        let is_loading = is_loading.clone();

        use_effect_with(auction_id, move |auction_id| {
            if current_round.is_none() && !*is_loading {
                refetch.emit(*auction_id);
            }
        });
    }

    let current_round_value = (*current_round).clone();
    let current_error = (*error).clone();
    // If outer Option is None and there's no error, we haven't fetched yet
    let current_is_loading = *is_loading
        || (current_round_value.is_none() && current_error.is_none());

    CurrentRoundHookReturn {
        current_round: current_round_value,
        error: current_error,
        is_loading: current_is_loading,
        refetch: Callback::from(move |_| refetch.emit(auction_id)),
    }
}
