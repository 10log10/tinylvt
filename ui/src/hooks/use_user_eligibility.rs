use payloads::AuctionRoundId;
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{FetchHookReturn, use_fetch},
};

/// Hook to fetch the current user's eligibility points for a round
///
/// Returns the eligibility score which determines which spaces the user
/// can bid on based on the round's eligibility threshold.
///
/// The data field uses `FetchState<Option<f64>>` to distinguish:
/// - `NotFetched`: Haven't called the API yet
/// - `Fetched(None)`: API returned None (e.g., round 0 has no eligibility)
/// - `Fetched(Some(0.5))`: API returned Some(0.5)
#[hook]
pub fn use_user_eligibility(
    round_id: AuctionRoundId,
) -> FetchHookReturn<Option<f64>> {
    use_fetch(round_id, move || async move {
        let api_client = get_api_client();
        // API returns Option<f64>
        api_client
            .get_eligibility(&round_id)
            .await
            .map_err(|e| e.to_string())
    })
}
