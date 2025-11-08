use payloads::{SiteId, responses};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::{State, get_api_client};

/// Hook return type for single site data
pub struct SiteHookReturn {
    pub site: Option<responses::Site>,
    pub is_loading: bool,
    pub error: Option<String>,
    #[allow(dead_code)]
    pub refetch: Callback<()>,
}

impl SiteHookReturn {
    /// Returns true if this is the initial load (no data, no error, loading)
    #[allow(dead_code)]
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading && self.site.is_none() && self.error.is_none()
    }
}

/// Hook to manage single site data with lazy loading and global state caching
///
/// ## Hook Architecture Rationale
///
/// This implements a consistent 3-tier hook hierarchy:
/// 1. `use_communities` - Fetches all communities for the user
/// 2. `use_sites(community_id)` - Fetches all sites for a specific community
/// 3. `use_site(site_id)` - Fetches a single site by ID
///
/// This enables flatter routes (`/sites/:id`) while maintaining efficient data
/// fetching at each granularity level. No `use_community` hook is needed since
/// `use_communities` already loads all user communities.
#[hook]
pub fn use_site(site_id: SiteId) -> SiteHookReturn {
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
                match api_client.get_site(&site_id).await {
                    Ok(site) => {
                        dispatch.reduce_mut(|state| {
                            state.set_site(site_id, site);
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

    // Auto-load site if not already loaded and user is authenticated
    {
        let refetch = refetch.clone();
        let state = state.clone();
        let is_loading = is_loading.clone();

        use_effect_with(
            (state.auth_state.clone(), site_id),
            move |(_, site_id)| {
                if state.is_authenticated()
                    && !state.has_site_loaded(*site_id)
                    && !*is_loading
                {
                    refetch.emit(*site_id);
                }
            },
        );
    }

    // Consider it "loading" if actively loading OR if we're in initial state
    // (no data, no error yet)
    let site = state.get_site(site_id).cloned();
    let current_error = (*error).clone();
    let effective_is_loading =
        *is_loading || (site.is_none() && current_error.is_none());

    SiteHookReturn {
        site,
        is_loading: effective_is_loading,
        error: current_error,
        refetch: Callback::from(move |_| refetch.emit(site_id)),
    }
}
