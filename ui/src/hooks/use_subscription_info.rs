use payloads::{CommunityId, SubscriptionInfo, requests};
use yew::prelude::*;

use crate::{get_api_client, hooks::use_fetch};

use super::FetchHookReturn;

/// Hook to fetch subscription info for a community
/// (coleader+ only). Lightweight — does not trigger storage
/// recalculation.
#[hook]
pub fn use_subscription_info(
    community_id: CommunityId,
) -> FetchHookReturn<Option<SubscriptionInfo>> {
    use_fetch(community_id, move || async move {
        let client = get_api_client();
        let request = requests::GetSubscriptionInfo { community_id };
        client
            .get_subscription_info(&request)
            .await
            .map_err(|e| e.to_string())
    })
}
