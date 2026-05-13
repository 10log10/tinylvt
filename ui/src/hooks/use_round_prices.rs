use payloads::{AuctionRoundId, RoundSpaceResult};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{FetchHookReturn, use_fetch};

/// Hook to fetch space prices (results) for a specific round.
///
/// Returns the winning bid value for each space in the round. If `round_id`
/// is `None`, the hook returns an empty result without fetching.
///
/// Round results are immutable once written by the scheduler, so this hook
/// doesn't subscribe to SSE. Callers pass `previous_round_id` from the
/// parent's last-round fetch; when a round transition happens, the parent
/// re-renders with a new id and the hook key change drives the refetch.
#[hook]
pub fn use_round_prices(
    round_id: Option<AuctionRoundId>,
) -> FetchHookReturn<Vec<RoundSpaceResult>> {
    use_fetch(round_id, move || async move {
        // If no round_id provided, don't fetch
        let Some(round_id) = round_id else {
            return Ok(vec![]);
        };

        let api_client = get_api_client();
        api_client
            .list_round_space_results_for_round(&round_id)
            .await
            .map_err(|e| e.to_string())
    })
}
