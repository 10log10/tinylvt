use payloads::{AuctionRoundId, RoundSpaceResult};
use yew::prelude::*;

use crate::get_api_client;

/// Hook return type for round space results (prices)
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[allow(dead_code)]
pub struct RoundPricesHookReturn {
    pub prices: Option<Vec<RoundSpaceResult>>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
}

/// Hook to fetch space prices (results) for a specific round
///
/// Returns the winning bid value for each space in the round.
/// If round_id is None, the hook will not fetch and return empty state.
#[hook]
pub fn use_round_prices(
    round_id: Option<AuctionRoundId>,
) -> RoundPricesHookReturn {
    let prices = use_state(|| None);
    let error = use_state(|| None);
    let is_loading = use_state(|| false);

    let refetch = {
        let prices = prices.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(round_id, move |round_id_opt, _| {
            let prices = prices.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();

            // If no round_id provided, don't fetch
            let Some(round_id) = round_id_opt else {
                return;
            };

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client
                    .list_round_space_results_for_round(&round_id)
                    .await
                {
                    Ok(results) => {
                        prices.set(Some(results));
                        error.set(None);
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    // Auto-load prices on mount and whenever round_id changes
    {
        let refetch = refetch.clone();

        use_effect_with(round_id, move |round_id| {
            refetch.emit(*round_id);
        });
    }

    let current_prices = (*prices).clone();
    let current_error = (*error).clone();
    let current_is_loading =
        *is_loading || (current_prices.is_none() && current_error.is_none());

    RoundPricesHookReturn {
        prices: current_prices,
        error: current_error,
        is_loading: current_is_loading,
        refetch: Callback::from(move |_| refetch.emit(round_id)),
    }
}
