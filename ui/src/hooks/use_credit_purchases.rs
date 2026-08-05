use payloads::{CommunityId, responses};
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{FetchHookReturn, use_fetch},
};

/// Hook to fetch the current user's credit purchases in a community
/// (pending display and history).
#[hook]
pub fn use_credit_purchases(
    community_id: CommunityId,
) -> FetchHookReturn<Vec<responses::CreditPurchase>> {
    use_fetch(community_id, move || async move {
        let api_client = get_api_client();
        api_client
            .list_credit_purchases(&community_id)
            .await
            .map_err(|e| e.to_string())
    })
}
