use payloads::{AuctionId, requests, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{Fetch, use_fetch};

/// Hook return type for proxy bidding settings
///
/// `data` is `Fetch<Option<UseProxyBidding>>`: `NotFetched` while
/// loading, `Fetched(None)` if proxy bidding is disabled, `Fetched(Some)` if
/// enabled. Derefs to `Fetch` so render-only consumers can
/// take `&Fetch<Option<UseProxyBidding>>`.
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub struct ProxyBiddingSettingsHookReturn {
    pub inner: Fetch<Option<responses::UseProxyBidding>>,
    pub refetch: Callback<()>,
    pub update: Callback<i32>,
    pub delete: Callback<()>,
}

impl std::ops::Deref for ProxyBiddingSettingsHookReturn {
    type Target = Fetch<Option<responses::UseProxyBidding>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Hook to manage proxy bidding settings for an auction
///
/// Provides methods to get, update, and delete proxy bidding settings.
/// The inner Option tracks whether proxy bidding is enabled (None =
/// disabled, Some = enabled with settings). Mutation errors are merged
/// into `inner.errors` via `map_err`, so they surface through the same
/// `stale_data_banner` / `render_section` paths as fetch errors.
#[hook]
pub fn use_proxy_bidding_settings(
    auction_id: AuctionId,
) -> ProxyBiddingSettingsHookReturn {
    let fetch_hook = use_fetch(auction_id, move || async move {
        let api_client = get_api_client();
        api_client
            .get_proxy_bidding(&auction_id)
            .await
            .map_err(|e| e.to_string())
    });

    let mutation_errors = use_state(Vec::<String>::new);

    let update = {
        let refetch = fetch_hook.refetch.clone();
        let mutation_errors = mutation_errors.clone();

        use_callback(auction_id, move |max_items, auction_id| {
            let refetch = refetch.clone();
            let mutation_errors = mutation_errors.clone();
            let auction_id = *auction_id;

            yew::platform::spawn_local(async move {
                let api_client = get_api_client();
                let request = requests::UseProxyBidding {
                    auction_id,
                    max_items,
                };

                match api_client.create_or_update_proxy_bidding(&request).await
                {
                    Ok(_) => {
                        mutation_errors.set(vec![]);
                        refetch.emit(());
                    }
                    Err(e) => {
                        mutation_errors.set(vec![e.to_string()]);
                    }
                }
            });
        })
    };

    let delete = {
        let refetch = fetch_hook.refetch.clone();
        let mutation_errors = mutation_errors.clone();

        use_callback(auction_id, move |_, auction_id| {
            let refetch = refetch.clone();
            let mutation_errors = mutation_errors.clone();
            let auction_id = *auction_id;

            yew::platform::spawn_local(async move {
                let api_client = get_api_client();
                match api_client.delete_proxy_bidding(&auction_id).await {
                    Ok(_) => {
                        mutation_errors.set(vec![]);
                        refetch.emit(());
                    }
                    Err(e) => {
                        mutation_errors.set(vec![e.to_string()]);
                    }
                }
            });
        })
    };

    let mutation_errs = (*mutation_errors).clone();
    let inner = fetch_hook.inner.clone().map_err(move |mut errs| {
        errs.extend(mutation_errs);
        errs
    });

    ProxyBiddingSettingsHookReturn {
        inner,
        refetch: fetch_hook.refetch,
        update,
        delete,
    }
}
