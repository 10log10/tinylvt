use payloads::{CommunityId, responses};
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{FetchHookReturn, use_fetch},
};

/// Hook to fetch a community's Stripe Connect standing (coleader+ only).
#[hook]
pub fn use_community_stripe_status(
    community_id: CommunityId,
) -> FetchHookReturn<responses::CommunityStripeStatus> {
    use_fetch(community_id, move || async move {
        get_api_client()
            .get_community_stripe_status(&community_id)
            .await
            .map_err(|e| e.to_string())
    })
}
