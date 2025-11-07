use payloads::AuctionRoundId;
use yew::prelude::*;

use crate::{get_api_client, hooks::FetchState};

/// Hook return type for user eligibility data
///
/// The `eligibility` field uses `FetchState<Option<f64>>` to distinguish:
/// - `NotFetched`: Haven't called the API yet
/// - `Fetched(None)`: API returned None (e.g., round 0 has no eligibility)
/// - `Fetched(Some(0.5))`: API returned Some(0.5)
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[allow(dead_code)]
pub struct UserEligibilityHookReturn {
    pub eligibility: FetchState<Option<f64>>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
}

impl UserEligibilityHookReturn {
    /// Returns true if this is the initial load (no data, no error, loading)
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading
            && !self.eligibility.is_fetched()
            && self.error.is_none()
    }
}

/// Hook to fetch the current user's eligibility points for a round
///
/// Returns the eligibility score which determines which spaces the user
/// can bid on based on the round's eligibility threshold.
#[hook]
pub fn use_user_eligibility(
    round_id: AuctionRoundId,
) -> UserEligibilityHookReturn {
    // Use FetchState to distinguish "not fetched" from "fetched None"
    let eligibility = use_state(|| FetchState::NotFetched);
    let error = use_state(|| None);
    let is_loading = use_state(|| true);

    let refetch = {
        let eligibility = eligibility.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(round_id, move |round_id, _| {
            let eligibility = eligibility.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.get_eligibility(&round_id).await {
                    Ok(points) => {
                        // API returns Option<f64>, store as Fetched
                        eligibility.set(FetchState::Fetched(points));
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

    // Auto-load eligibility on mount and whenever round_id changes
    {
        let refetch = refetch.clone();

        use_effect_with(round_id, move |round_id| {
            refetch.emit(*round_id);
        });
    }

    UserEligibilityHookReturn {
        // Return FetchState directly - don't flatten!
        eligibility: (*eligibility).clone(),
        error: (*error).clone(),
        is_loading: *is_loading,
        refetch: Callback::from(move |_| refetch.emit(round_id)),
    }
}
