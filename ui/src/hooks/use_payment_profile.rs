use payloads::responses;
use yew::prelude::*;

use crate::{
    get_api_client,
    hooks::{FetchHookReturn, use_fetch},
};

/// Hook to fetch the current user's payment settings (saved card +
/// hold strategy).
#[hook]
pub fn use_payment_profile() -> FetchHookReturn<responses::UserPaymentProfile> {
    use_fetch((), move || async move {
        let api_client = get_api_client();
        api_client
            .get_payment_profile()
            .await
            .map_err(|e| e.to_string())
    })
}
