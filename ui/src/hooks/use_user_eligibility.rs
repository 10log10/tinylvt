use payloads::AuctionRoundId;
use yew::prelude::*;

use crate::get_api_client;

/// Hook return type for user eligibility data
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[allow(dead_code)]
pub struct UserEligibilityHookReturn {
    pub eligibility: Option<f64>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
}

/// Hook to fetch the current user's eligibility points for a round
///
/// Returns the eligibility score which determines which spaces the user
/// can bid on based on the round's eligibility threshold.
#[hook]
pub fn use_user_eligibility(
    round_id: AuctionRoundId,
) -> UserEligibilityHookReturn {
    let eligibility = use_state(|| None);
    let error = use_state(|| None);
    let is_loading = use_state(|| false);

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
                        eligibility.set(Some(points));
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

    // Auto-load eligibility on mount
    {
        let refetch = refetch.clone();
        let eligibility = eligibility.clone();
        let is_loading = is_loading.clone();

        use_effect_with(round_id, move |round_id| {
            if eligibility.is_none() && !*is_loading {
                refetch.emit(*round_id);
            }
        });
    }

    let current_eligibility = *eligibility;
    let current_error = (*error).clone();
    let current_is_loading = *is_loading
        || (current_eligibility.is_none() && current_error.is_none());

    UserEligibilityHookReturn {
        eligibility: current_eligibility,
        error: current_error,
        is_loading: current_is_loading,
        refetch: Callback::from(move |_| refetch.emit(round_id)),
    }
}
