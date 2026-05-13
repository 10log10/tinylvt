use payloads::{AuctionId, AuctionRoundId, SpaceId};
use std::collections::HashMap;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{
    SubscribedEvent, SubscribedFetchHookReturn, use_subscribed_fetch,
};

/// Per-round bids map. Each round maps to either the user's bid space ids
/// for that round or the error string from the failed fetch, so callers can
/// render an inline error in the corresponding card section instead of
/// silently treating missing data as "no bids".
pub type UserBidsMap = HashMap<AuctionRoundId, Result<Vec<SpaceId>, String>>;

/// Hook to fetch all of the current user's bids across all rounds in an
/// auction.
///
/// Used by the rounds page to show bidding activity across all rounds.
/// Subscribed to `BidsChanged`, which fires on every change to this user's
/// bids (both scheduler proxy bids and manual create/delete). The event is
/// server-side filtered to this user.
#[hook]
pub fn use_auction_user_bids(
    auction_id: AuctionId,
    rounds: Vec<payloads::responses::AuctionRound>,
) -> SubscribedFetchHookReturn<UserBidsMap> {
    use_subscribed_fetch(
        (auction_id, rounds.clone()),
        auction_id,
        &[SubscribedEvent::BidsChanged],
        move || {
            let rounds = rounds.clone();
            async move {
                let api_client = get_api_client();
                let mut all_bids = HashMap::new();

                for round in rounds {
                    let round_id = round.round_id;
                    let entry = match api_client.list_bids(&round_id).await {
                        Ok(bids) => Ok(bids
                            .iter()
                            .map(|bid| bid.space_id)
                            .collect::<Vec<_>>()),
                        Err(e) => {
                            let msg = e.to_string();
                            tracing::error!(
                                "Failed to fetch bids for round {:?}: {}",
                                round_id,
                                msg,
                            );
                            Err(msg)
                        }
                    };
                    all_bids.insert(round_id, entry);
                }

                Ok(all_bids)
            }
        },
    )
}
