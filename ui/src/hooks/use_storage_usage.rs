use payloads::{CommunityId, CommunityStorageUsage, requests};
use std::rc::Rc;
use yew::prelude::*;
use yewdux::prelude::use_store;

use crate::{
    State, get_api_client,
    hooks::{FetchHookReturn, use_fetch_with_cache},
};

/// Hook to fetch storage usage for a community (coleader+ only).
/// Uses global state cache to avoid excessive API calls and deduplicates
/// concurrent refetch requests for the same community.
#[hook]
pub fn use_storage_usage(
    community_id: CommunityId,
) -> FetchHookReturn<CommunityStorageUsage> {
    let (state, dispatch) = use_store::<State>();

    let get_cached_state = state.clone();
    let should_fetch_state = state.clone();

    // Use a callback to capture dispatch and community_id for deduplication
    let fetch_fn = {
        let dispatch = dispatch.clone();
        Rc::new(move || {
            let dispatch = dispatch.clone();
            async move {
                // Check if another fetch is already in-flight for this
                // community. If so, wait for it to complete instead of
                // starting a duplicate.
                let already_fetching = {
                    let state = dispatch.get();
                    state.storage_usage_refetching.contains(&community_id)
                };

                if already_fetching {
                    // Wait for the other fetch to complete by polling
                    // the state until data appears
                    for _ in 0..50 {
                        // 50 * 100ms = 5 second timeout
                        yew::platform::time::sleep(
                            std::time::Duration::from_millis(100),
                        )
                        .await;

                        let state = dispatch.get();
                        if let Some(usage) =
                            state.storage_usage.get(&community_id).cloned()
                        {
                            return Ok(usage);
                        }
                    }
                    // Timeout waiting for other fetch
                    return Err(
                        "Timeout waiting for storage usage fetch".to_string()
                    );
                }

                // Mark as fetching
                dispatch.reduce_mut(|s| {
                    s.storage_usage_refetching.insert(community_id);
                });

                let api_client = get_api_client();
                let request =
                    requests::GetCommunityStorageUsage { community_id };

                let result = api_client
                    .get_community_storage_usage(&request)
                    .await
                    .map_err(|e| e.to_string());

                // ALWAYS clear the refetching flag, whether success or error
                dispatch.reduce_mut(|s| {
                    s.storage_usage_refetching.remove(&community_id);
                });

                match result {
                    Ok(usage) => {
                        dispatch.reduce_mut(|s| {
                            s.storage_usage.insert(community_id, usage.clone());
                        });
                        Ok(usage)
                    }
                    Err(e) => Err(e),
                }
            }
        })
    };

    use_fetch_with_cache(
        community_id,
        move || get_cached_state.storage_usage.get(&community_id).cloned(),
        move || {
            // Only fetch if data is not cached
            !should_fetch_state.storage_usage.contains_key(&community_id)
        },
        move || fetch_fn(),
    )
}
