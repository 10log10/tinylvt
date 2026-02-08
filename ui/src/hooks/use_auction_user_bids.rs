use payloads::{AuctionId, AuctionRoundId, SpaceId};
use std::collections::HashMap;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{FetchHookReturn, use_fetch};

/// Type alias for the bids map returned by this hook
type UserBidsMap = HashMap<AuctionRoundId, Vec<SpaceId>>;

/// Hook to fetch all user bids across all rounds in an auction
///
/// This is useful for the rounds page where we need to show bidding activity
/// across all rounds without making separate API calls for each round.
#[hook]
pub fn use_auction_user_bids(
    auction_id: AuctionId,
    rounds: Vec<payloads::responses::AuctionRound>,
) -> FetchHookReturn<UserBidsMap> {
    use_fetch((auction_id, rounds.clone()), move || {
        let rounds = rounds.clone();
        async move {
            let api_client = get_api_client();
            let mut all_bids = HashMap::new();

            // Fetch bids for each round
            for round in rounds {
                let round_id = round.round_id;
                match api_client.list_bids(&round_id).await {
                    Ok(bids) => {
                        let space_ids: Vec<SpaceId> =
                            bids.iter().map(|bid| bid.space_id).collect();
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

            Ok(all_bids)
        }
    })
}
