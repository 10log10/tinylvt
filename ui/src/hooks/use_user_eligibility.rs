use payloads::{AuctionRoundId, Eligibility};
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{FetchHookReturn, use_fetch},
};

/// Hook to fetch the current user's eligibility for a round.
///
/// The API resolves the prior round's threshold and the user's eligibility
/// row into an `Eligibility`, so the UI doesn't need to interpret a raw
/// number against the threshold itself:
/// - `Unlimited`: round 0, or the prior round's threshold was 0%.
/// - `Finite(x)`: a budget of `x` points. `Finite(0.0)` means the user has no
///   budget (can only bid 0-point spaces).
#[hook]
pub fn use_user_eligibility(
    round_id: AuctionRoundId,
) -> FetchHookReturn<Eligibility> {
    use_fetch(round_id, move || async move {
        let api_client = get_api_client();
        api_client
            .get_eligibility(&round_id)
            .await
            .map_err(|e| e.to_string())
    })
}
