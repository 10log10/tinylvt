use payloads::{AuctionId, AuctionRoundId, RoundSpaceResult};
use std::collections::HashMap;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{
    SubscribedEvent, SubscribedFetchHookReturn, use_subscribed_fetch,
};

/// Per-round results map. Each round maps to either its space results or
/// the error string from the failed fetch, so callers can render an inline
/// error in the corresponding card section instead of silently treating
/// missing data as "no high bidder".
pub type RoundResultsMap =
    HashMap<AuctionRoundId, Result<Vec<RoundSpaceResult>, String>>;

/// Hook to fetch all round results across all rounds in an auction.
///
/// Used by the rounds page to show high-bidder information across all rounds
/// without making separate calls. Subscribed to `RoundEnded`, since results
/// land when a round concludes (whether the auction continues or ends).
#[hook]
pub fn use_auction_round_results(
    auction_id: AuctionId,
    rounds: Vec<payloads::responses::AuctionRound>,
) -> SubscribedFetchHookReturn<RoundResultsMap> {
    use_subscribed_fetch(
        (auction_id, rounds.clone()),
        auction_id,
        &[SubscribedEvent::RoundEnded],
        move || {
            let rounds = rounds.clone();
            async move {
                let api_client = get_api_client();
                let mut all_results = HashMap::new();

                for round in rounds {
                    let round_id = round.round_id;
                    let entry = match api_client
                        .list_round_space_results_for_round(&round_id)
                        .await
                    {
                        Ok(results) => Ok(results),
                        Err(e) => {
                            let msg = e.to_string();
                            tracing::error!(
                                "Failed to fetch results for round {:?}: {}",
                                round_id,
                                msg,
                            );
                            Err(msg)
                        }
                    };
                    all_results.insert(round_id, entry);
                }

                Ok(all_results)
            }
        },
    )
}
