use payloads::{AuctionId, responses};
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{FetchHookReturn, use_fetch};

/// Hook to fetch the members who have enabled proxy bidding for an auction.
///
/// Restricted to coleaders+ on the backend (a plain member's fetch fails
/// with a permission error). Used before an auction starts to let leaders
/// nudge interested members who haven't opted in yet.
#[hook]
pub fn use_proxy_bidding_participants(
    auction_id: AuctionId,
) -> FetchHookReturn<Vec<responses::UserIdentity>> {
    use_fetch(auction_id, move || async move {
        let api_client = get_api_client();
        api_client
            .list_proxy_bidding_participants(&auction_id)
            .await
            .map_err(|e| e.to_string())
    })
}
