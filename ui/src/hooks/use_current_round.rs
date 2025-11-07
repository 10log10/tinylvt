use payloads::{AuctionId, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::FetchState;

/// Hook return type for current round data
///
/// The `current_round` field uses FetchState to distinguish fetch state:
/// - `FetchState::NotFetched`: Data not fetched yet / still loading
/// - `FetchState::Fetched(None)`: Successfully fetched, but no current round
///   exists
/// - `FetchState::Fetched(Some(round))`: Successfully fetched with a current
///   round
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[derive(Debug)]
#[allow(dead_code)]
pub struct CurrentRoundHookReturn {
    pub current_round: FetchState<Option<responses::AuctionRound>>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
}

impl CurrentRoundHookReturn {
    /// Returns true if this is the initial load (no data, no error, loading)
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading
            && !self.current_round.is_fetched()
            && self.error.is_none()
    }
}

/// Hook to fetch the current (or most recent) round for an auction
///
/// This fetches all rounds and returns the latest one (highest round_num).
/// If the auction has not started yet (no rounds exist), `current_round` will
/// be `FetchState::Fetched(None)` with no error.
#[hook]
pub fn use_current_round(auction_id: AuctionId) -> CurrentRoundHookReturn {
    let current_round = use_state(|| FetchState::NotFetched);
    let error = use_state(|| None);
    let is_loading = use_state(|| true);

    let refetch = {
        let current_round = current_round.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(auction_id, move |auction_id, _| {
            let current_round = current_round.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.list_auction_rounds(&auction_id).await {
                    Ok(rounds) => {
                        // Get the latest round (highest round_num)
                        let latest = rounds
                            .into_iter()
                            .max_by_key(|r| r.round_details.round_num);
                        current_round.set(FetchState::Fetched(latest));
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

    // Auto-load current round on mount and when auction_id changes
    {
        let refetch = refetch.clone();

        use_effect_with(auction_id, move |auction_id| {
            refetch.emit(*auction_id);
        });
    }

    CurrentRoundHookReturn {
        current_round: (*current_round).clone(),
        error: (*error).clone(),
        is_loading: *is_loading,
        refetch: Callback::from(move |_| refetch.emit(auction_id)),
    }
}
