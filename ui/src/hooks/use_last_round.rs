use payloads::{AuctionId, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{
    SubscribedEvent, SubscribedFetchHookReturn, use_subscribed_fetch,
};

/// The two most recent rounds for an auction.
///
/// `last_round` is the round with the highest `round_num` — the one
/// currently being bid on (if the auction is active) or the one that ended
/// the auction (if `auction.end_at.is_some()`). `previous_round` is the one
/// before it, present only when `last_round.round_num > 0`.
///
/// Returning both means consumers don't need to call `use_auction_rounds`
/// just to look up the previous round (which is needed to read prices —
/// prices for round N are stored in round N-1's results).
#[derive(Clone, Debug, PartialEq)]
pub struct LastRoundInfo {
    pub last_round: responses::AuctionRound,
    pub previous_round: Option<responses::AuctionRound>,
}

/// Hook to fetch the most recent round (and the round before it) for an
/// auction.
///
/// "Most recent" means the round with the highest `round_num`. While the
/// auction is active this is the round currently being bid on. Once the
/// auction has ended (`auction.end_at.is_some()`) it remains the last round
/// that ever existed — there is no "current" round at that point. Callers
/// should consult `auction.end_at` to decide which mode they're in. This
/// hook also returns `Fetched(None)` if the auction has not started yet
/// (no rounds exist).
///
/// Subscribed to `RoundCreated` so a new round transition is reflected
/// without polling. `connection_status` on the return value indicates
/// freshness: `Some(Failed)` means live updates aren't working and the user
/// should refresh.
///
/// The data field uses `FetchData<Option<LastRoundInfo>>` to distinguish:
/// - `NotFetched`: Data not fetched yet / still loading
/// - `Fetched(None)`: Successfully fetched, but no round exists yet
/// - `Fetched(Some(info))`: Successfully fetched
#[hook]
pub fn use_last_round(
    auction_id: AuctionId,
) -> SubscribedFetchHookReturn<Option<LastRoundInfo>> {
    use_subscribed_fetch(
        auction_id,
        auction_id,
        &[SubscribedEvent::RoundCreated],
        move || async move {
            let api_client = get_api_client();
            let mut rounds = api_client
                .list_auction_rounds(&auction_id)
                .await
                .map_err(|e| e.to_string())?;

            // Sort descending by round_num, then take the first two.
            rounds.sort_by(|a, b| {
                b.round_details.round_num.cmp(&a.round_details.round_num)
            });
            let mut iter = rounds.into_iter();
            let Some(last_round) = iter.next() else {
                return Ok(None);
            };
            let previous_round = iter.next();
            Ok(Some(LastRoundInfo {
                last_round,
                previous_round,
            }))
        },
    )
}
