use payloads::{AuctionId, requests, responses};
use yew::prelude::*;

use crate::get_api_client;

/// Hook return type for proxy bidding settings
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ProxyBiddingSettingsHookReturn {
    pub settings: Option<Option<responses::UseProxyBidding>>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
    pub update: Callback<i32>,
    pub delete: Callback<()>,
}

/// Hook to manage proxy bidding settings for an auction
///
/// Provides methods to get, update, and delete proxy bidding settings.
/// The outer Option tracks loading state, inner Option tracks whether
/// proxy bidding is enabled (None = disabled, Some = enabled with settings).
#[hook]
pub fn use_proxy_bidding_settings(
    auction_id: AuctionId,
) -> ProxyBiddingSettingsHookReturn {
    let settings = use_state(|| None);
    let error = use_state(|| None);
    let is_loading = use_state(|| false);

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
                        settings.set(Some(proxy_settings));
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
                                settings.set(Some(proxy_settings));
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
                        settings.set(Some(None));
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

    // Auto-load settings on mount
    {
        let refetch = refetch.clone();
        let settings = settings.clone();
        let is_loading = is_loading.clone();

        use_effect_with(auction_id, move |auction_id| {
            if settings.is_none() && !*is_loading {
                refetch.emit(*auction_id);
            }
        });
    }

    let current_settings = (*settings).clone();
    let current_error = (*error).clone();
    let current_is_loading =
        *is_loading || (current_settings.is_none() && current_error.is_none());

    ProxyBiddingSettingsHookReturn {
        settings: current_settings,
        error: current_error,
        is_loading: current_is_loading,
        refetch: Callback::from(move |_| refetch.emit(auction_id)),
        update,
        delete,
    }
}
