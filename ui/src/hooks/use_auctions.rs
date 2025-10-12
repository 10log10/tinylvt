use payloads::{SiteId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{State, get_api_client};

/// Hook return type for auctions data
pub struct AuctionsHookReturn {
    pub auctions: Option<Vec<responses::Auction>>,
    pub is_loading: bool,
    pub error: Option<String>,
    #[allow(dead_code)]
    pub refetch: Callback<()>,
}

/// Hook to manage auctions data with lazy loading and global state caching
///
/// This follows the same pattern as use_spaces and use_sites, providing
/// efficient data loading and caching for auctions at the site level.
#[hook]
pub fn use_auctions(site_id: SiteId) -> AuctionsHookReturn {
    let (state, dispatch) = use_store::<State>();
    let is_loading = use_state(|| false);
    let error = use_state(|| None::<String>);

    let refetch = {
        let dispatch = dispatch.clone();
        let is_loading = is_loading.clone();
        let error = error.clone();

        use_callback(site_id, move |site_id, _| {
            let dispatch = dispatch.clone();
            let is_loading = is_loading.clone();
            let error = error.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.list_auctions(&site_id).await {
                    Ok(auctions) => {
                        dispatch.reduce_mut(|state| {
                            state.set_auctions_for_site(site_id, auctions);
                        });
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

    // Auto-load auctions if not already loaded and user is authenticated
    {
        let refetch = refetch.clone();
        let state = state.clone();
        let is_loading = is_loading.clone();

        use_effect_with(
            (state.auth_state.clone(), site_id),
            move |(_, site_id)| {
                if state.is_authenticated()
                    && !state.has_auctions_loaded_for_site(*site_id)
                    && !*is_loading
                {
                    refetch.emit(*site_id);
                }
            },
        );
    }

    // Consider it "loading" if actively loading OR if we're in initial state
    // (no data, no error yet)
    let auctions = state
        .get_auctions_for_site(site_id)
        .map(|auction_refs| auction_refs.into_iter().cloned().collect());
    let current_error = (*error).clone();
    let effective_is_loading =
        *is_loading || (auctions.is_none() && current_error.is_none());

    AuctionsHookReturn {
        auctions,
        is_loading: effective_is_loading,
        error: current_error,
        refetch: Callback::from(move |_| refetch.emit(site_id)),
    }
}
