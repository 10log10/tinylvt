use payloads::{AuctionRoundId, SpaceId};
use std::collections::HashSet;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::FetchState;

/// Hook return type for user bids data
///
/// Returns a set of space IDs that the user has bid on in the specified round.
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[derive(Debug)]
#[allow(dead_code)]
pub struct UserBidsHookReturn {
    pub bid_space_ids: FetchState<HashSet<SpaceId>>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
}

impl UserBidsHookReturn {
    /// Returns true if this is the initial load (no data, no error, loading)
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading
            && !self.bid_space_ids.is_fetched()
            && self.error.is_none()
    }
}

/// Hook to fetch the user's bids for a specific round
///
/// Returns a set of space IDs that the user has placed bids on in the round.
/// This is used to show which spaces have active bids and disable the bid
/// button for those spaces.
#[hook]
pub fn use_user_bids(round_id: AuctionRoundId) -> UserBidsHookReturn {
    let bid_space_ids = use_state(|| FetchState::NotFetched);
    let error = use_state(|| None);
    let is_loading = use_state(|| true);

    let refetch = {
        let bid_space_ids = bid_space_ids.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(round_id, move |round_id, _| {
            let bid_space_ids = bid_space_ids.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.list_bids(&round_id).await {
                    Ok(bids) => {
                        // Extract space IDs from the bids
                        let bids_set: HashSet<SpaceId> =
                            bids.iter().map(|bid| bid.space_id).collect();
                        bid_space_ids.set(FetchState::Fetched(bids_set));
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

    // Auto-load bids on mount or when round_id changes
    {
        let refetch = refetch.clone();

        use_effect_with(round_id, move |round_id| {
            refetch.emit(*round_id);
        });
    }

    UserBidsHookReturn {
        bid_space_ids: (*bid_space_ids).clone(),
        error: (*error).clone(),
        is_loading: *is_loading,
        refetch: Callback::from(move |_| refetch.emit(round_id)),
    }
}
