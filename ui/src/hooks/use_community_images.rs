use payloads::{CommunityId, responses};
use yew::prelude::*;

use crate::{get_api_client, hooks::use_fetch};

use super::FetchHookReturn;

/// Hook to fetch all site images for a community.
/// Returns lightweight image info (without the actual image data).
#[hook]
pub fn use_community_images(
    community_id: CommunityId,
) -> FetchHookReturn<Vec<responses::SiteImageInfo>> {
    use_fetch(community_id, move || async move {
        let api_client = get_api_client();
        api_client
            .list_site_images(&community_id)
            .await
            .map_err(|e| e.to_string())
    })
}
