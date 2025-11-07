use payloads::{AuctionId, AuctionRoundId, SpaceId};
use std::collections::HashMap;
use yew::prelude::*;

use crate::get_api_client;

/// Hook return type for auction-wide user bids data
///
/// Returns a map from round_id to the set of space IDs that the user has bid
/// on in that round.
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[derive(Debug)]
#[allow(dead_code)]
pub struct AuctionUserBidsHookReturn {
    pub bids_by_round: Option<HashMap<AuctionRoundId, Vec<SpaceId>>>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
}

impl AuctionUserBidsHookReturn {
    /// Returns true if this is the initial load (no data, no error, loading)
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading && self.bids_by_round.is_none() && self.error.is_none()
    }
}

/// Hook to fetch all user bids across all rounds in an auction
///
/// This is useful for the rounds page where we need to show bidding activity
/// across all rounds without making separate API calls for each round.
#[hook]
pub fn use_auction_user_bids(
    auction_id: AuctionId,
    rounds: Option<Vec<payloads::responses::AuctionRound>>,
) -> AuctionUserBidsHookReturn {
    let bids_by_round = use_state(|| None);
    let error = use_state(|| None);
    let is_loading = use_state(|| false);

    let refetch = {
        let bids_by_round = bids_by_round.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(
            (auction_id, rounds.clone()),
            move |(_, rounds_opt): (
                AuctionId,
                Option<Vec<payloads::responses::AuctionRound>>,
            ),
                  _| {
                let bids_by_round = bids_by_round.clone();
                let error = error.clone();
                let is_loading = is_loading.clone();

                // If no rounds provided, don't fetch
                let Some(rounds) = rounds_opt else {
                    return;
                };

                yew::platform::spawn_local(async move {
                    is_loading.set(true);
                    error.set(None);

                    let api_client = get_api_client();
                    let mut all_bids = HashMap::new();

                    // Fetch bids for each round
                    for round in rounds {
                        let round_id = round.round_id;
                        match api_client.list_bids(&round_id).await {
                            Ok(bids) => {
                                let space_ids: Vec<SpaceId> = bids
                                    .iter()
                                    .map(|bid| bid.space_id)
                                    .collect();
                                all_bids.insert(round_id, space_ids);
                            }
                            Err(e) => {
                                // Log error but continue fetching other rounds
                                tracing::error!(
                                    "Failed to fetch bids for round {:?}: {}",
                                    round_id,
                                    e
                                );
                            }
                        }
                    }

                    bids_by_round.set(Some(all_bids));
                    is_loading.set(false);
                });
            },
        )
    };

    // Auto-load bids on mount or when rounds change
    {
        let refetch = refetch.clone();

        use_effect_with(
            (auction_id, rounds.clone()),
            move |(auction_id, rounds)| {
                refetch.emit((*auction_id, rounds.clone()));
            },
        );
    }

    let current_bids = (*bids_by_round).clone();
    let current_error = (*error).clone();
    let current_is_loading =
        *is_loading || (current_bids.is_none() && current_error.is_none());

    AuctionUserBidsHookReturn {
        bids_by_round: current_bids,
        error: current_error,
        is_loading: current_is_loading,
        refetch: Callback::from(move |_| {
            refetch.emit((auction_id, rounds.clone()))
        }),
    }
}
