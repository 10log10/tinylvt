use payloads::{AuctionId, AuctionRoundId, RoundSpaceResult};
use std::collections::HashMap;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{FetchHookReturn, use_fetch};

/// Type alias for the results map returned by this hook
type RoundResultsMap = HashMap<AuctionRoundId, Vec<RoundSpaceResult>>;

/// Hook to fetch all round results across all rounds in an auction
///
/// This is useful for the rounds page where we need to show high bidder
/// information across all rounds without making separate API calls for each
/// round.
#[hook]
pub fn use_auction_round_results(
    auction_id: AuctionId,
    rounds: Vec<payloads::responses::AuctionRound>,
) -> FetchHookReturn<RoundResultsMap> {
    use_fetch((auction_id, rounds.clone()), move || {
        let rounds = rounds.clone();
        async move {
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
                            "Failed to fetch results for round {:?}: {}",
                            round_id,
                            e
                        );
                    }
                }
            }

            Ok(all_results)
        }
    })
}
