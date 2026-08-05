use payloads::{AuctionId, CommunityId};
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{SubscribedEvent, SubscribedFetchHookReturn, use_subscribed_fetch},
};

/// Fetches whether the current user has granted a community permission
/// to charge their saved card.
///
/// The grant is community-scoped, but the control lives on auction
/// pages, so the fetch rides that page's SSE stream and refetches on
/// `CardChargeGrantChanged`, keeping it in sync with the funding
/// section when either control changes the grant.
#[hook]
pub fn use_card_charge_grant(
    auction_id: AuctionId,
    community_id: CommunityId,
) -> SubscribedFetchHookReturn<bool> {
    use_subscribed_fetch(
        community_id,
        auction_id,
        &[SubscribedEvent::CardChargeGrantChanged],
        move || async move {
            let api_client = get_api_client();
            api_client
                .get_card_charge_grant(&community_id)
                .await
                .map_err(|e| e.to_string())
        },
    )
}
