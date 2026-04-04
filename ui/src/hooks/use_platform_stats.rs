use payloads::responses;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{FetchHookReturn, use_fetch};

#[hook]
pub fn use_platform_stats() -> FetchHookReturn<responses::PlatformStats> {
    use_fetch((), move || async move {
        let api_client = get_api_client();
        api_client.platform_stats().await.map_err(|e| e.to_string())
    })
}
