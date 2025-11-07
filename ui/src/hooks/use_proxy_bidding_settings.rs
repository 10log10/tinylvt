use payloads::{AuctionId, requests, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::FetchState;

/// Hook return type for proxy bidding settings
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ProxyBiddingSettingsHookReturn {
    pub settings: FetchState<Option<responses::UseProxyBidding>>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
    pub update: Callback<i32>,
    pub delete: Callback<()>,
}

impl ProxyBiddingSettingsHookReturn {
    /// Returns true if this is the initial load (no data, no error, loading)
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading && !self.settings.is_fetched() && self.error.is_none()
    }
}

/// Hook to manage proxy bidding settings for an auction
///
/// Provides methods to get, update, and delete proxy bidding settings.
/// FetchState tracks loading state, inner Option tracks whether
/// proxy bidding is enabled (None = disabled, Some = enabled with settings).
#[hook]
pub fn use_proxy_bidding_settings(
    auction_id: AuctionId,
) -> ProxyBiddingSettingsHookReturn {
    let settings = use_state(|| FetchState::NotFetched);
    let error = use_state(|| None);
    let is_loading = use_state(|| true);

    let refetch = {
        let settings = settings.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(auction_id, move |auction_id, _| {
            let settings = settings.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.get_proxy_bidding(&auction_id).await {
                    Ok(proxy_settings) => {
                        settings.set(FetchState::Fetched(proxy_settings));
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

    let update = {
        let settings = settings.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(auction_id, move |max_items, _| {
            let settings = settings.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                let request = requests::UseProxyBidding {
                    auction_id,
                    max_items,
                };

                match api_client.create_or_update_proxy_bidding(&request).await
                {
                    Ok(_) => {
                        // Refetch to get updated settings with timestamp
                        match api_client.get_proxy_bidding(&auction_id).await {
                            Ok(proxy_settings) => {
                                settings
                                    .set(FetchState::Fetched(proxy_settings));
                                error.set(None);
                            }
                            Err(e) => {
                                error.set(Some(e.to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    let delete = {
        let settings = settings.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(auction_id, move |_, _| {
            let settings = settings.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.delete_proxy_bidding(&auction_id).await {
                    Ok(_) => {
                        settings.set(FetchState::Fetched(None));
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

    // Auto-load settings on mount and when auction_id changes
    {
        let refetch = refetch.clone();

        use_effect_with(auction_id, move |auction_id| {
            refetch.emit(*auction_id);
        });
    }

    ProxyBiddingSettingsHookReturn {
        settings: (*settings).clone(),
        error: (*error).clone(),
        is_loading: *is_loading,
        refetch: Callback::from(move |_| refetch.emit(auction_id)),
        update,
        delete,
    }
}
