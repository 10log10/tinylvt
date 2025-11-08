use payloads::{AuctionId, AuctionRoundId, RoundSpaceResult};
use std::collections::HashMap;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::FetchState;

/// Hook return type for auction-wide round results
///
/// Returns a map from round_id to the list of round space results for that
/// round.
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[derive(Debug)]
#[allow(dead_code)]
pub struct AuctionRoundResultsHookReturn {
    pub results_by_round:
        FetchState<HashMap<AuctionRoundId, Vec<RoundSpaceResult>>>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
}

impl AuctionRoundResultsHookReturn {
    /// Returns true if this is the initial load (no data, no error, loading)
    #[allow(dead_code)]
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading
            && !self.results_by_round.is_fetched()
            && self.error.is_none()
    }
}

/// Hook to fetch all round results across all rounds in an auction
///
/// This is useful for the rounds page where we need to show high bidder
/// information across all rounds without making separate API calls for each
/// round.
#[hook]
pub fn use_auction_round_results(
    auction_id: AuctionId,
    rounds: Option<Vec<payloads::responses::AuctionRound>>,
) -> AuctionRoundResultsHookReturn {
    let results_by_round = use_state(|| FetchState::NotFetched);
    let error = use_state(|| None);
    let is_loading = use_state(|| true);

    let refetch = {
        let results_by_round = results_by_round.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(
            (auction_id, rounds.clone()),
            move |(_, rounds_opt): (
                AuctionId,
                Option<Vec<payloads::responses::AuctionRound>>,
            ),
                  _| {
                let results_by_round = results_by_round.clone();
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
                    let mut all_results = HashMap::new();

                    // Fetch results for each round
                    for round in rounds {
                        let round_id = round.round_id;
                        match api_client
                            .list_round_space_results_for_round(&round_id)
                            .await
                        {
                            Ok(results) => {
                                all_results.insert(round_id, results);
                            }
                            Err(e) => {
                                // Log error but continue fetching other rounds
                                tracing::error!(
                                    "Failed to fetch results for round {:?}: \
                                     {}",
                                    round_id,
                                    e
                                );
                            }
                        }
                    }

                    results_by_round.set(FetchState::Fetched(all_results));
                    is_loading.set(false);
                });
            },
        )
    };

    // Auto-load results on mount or when rounds change
    {
        let refetch = refetch.clone();

        use_effect_with(
            (auction_id, rounds.clone()),
            move |(auction_id, rounds)| {
                refetch.emit((*auction_id, rounds.clone()));
            },
        );
    }

    AuctionRoundResultsHookReturn {
        results_by_round: (*results_by_round).clone(),
        error: (*error).clone(),
        is_loading: *is_loading,
        refetch: Callback::from(move |_| {
            refetch.emit((auction_id, rounds.clone()))
        }),
    }
}
