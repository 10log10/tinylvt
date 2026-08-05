use payloads::AuctionId;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{
    SubscribedEvent, SubscribedFetchHookReturn, use_subscribed_fetch,
};

/// Fetches the current user's funding state in one auction
/// (backed_credits mode): balance allocation, live card authorization,
/// and card availability.
///
/// Refetches on `FundingChanged` (allocation and authorization writes),
/// `BidsChanged` (the commitment derives from bids), and
/// `CardChargeGrantChanged` (card availability derives from the grant).
#[hook]
pub fn use_auction_funding(
    auction_id: AuctionId,
) -> SubscribedFetchHookReturn<payloads::responses::AuctionFunding> {
    use_subscribed_fetch(
        auction_id,
        auction_id,
        &[
            SubscribedEvent::FundingChanged,
            SubscribedEvent::BidsChanged,
            SubscribedEvent::CardChargeGrantChanged,
        ],
        move || async move {
            get_api_client()
                .get_auction_funding(&auction_id)
                .await
                .map_err(|e| e.to_string())
        },
    )
}
