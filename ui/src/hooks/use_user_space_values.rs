use payloads::{SiteId, SpaceId, requests};
use rust_decimal::Decimal;
use std::collections::HashMap;
use yew::prelude::*;

use crate::get_api_client;
use crate::hooks::{Fetch, use_fetch};

/// Hook return type for user space values
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details. Derefs to `Fetch` so render-only consumers can
/// take `&Fetch<HashMap<SpaceId, Decimal>>` and avoid
/// rerendering on callback identity changes.
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub struct UserSpaceValuesHookReturn {
    pub inner: Fetch<HashMap<SpaceId, Decimal>>,
    pub refetch: Callback<()>,
    pub update_value: Callback<(SpaceId, Decimal)>,
    pub delete_value: Callback<SpaceId>,
}

impl std::ops::Deref for UserSpaceValuesHookReturn {
    type Target = Fetch<HashMap<SpaceId, Decimal>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Hook to manage user's max values for spaces at a site
///
/// Fetches all user values for a site and provides methods to update
/// and delete individual values. Values are stored as a HashMap for
/// quick lookup. Mutation errors are merged into `inner.errors` via
/// `map_err`, so they surface through the same `stale_data_banner` /
/// `render_section` paths as fetch errors.
#[hook]
pub fn use_user_space_values(site_id: SiteId) -> UserSpaceValuesHookReturn {
    let fetch_hook = use_fetch(site_id, move || async move {
        let api_client = get_api_client();
        api_client
            .list_user_values(&site_id)
            .await
            .map(|user_values| {
                user_values
                    .into_iter()
                    .map(|uv| (uv.space_id, uv.value))
                    .collect::<HashMap<SpaceId, Decimal>>()
            })
            .map_err(|e| e.to_string())
    });

    let mutation_errors = use_state(Vec::<String>::new);

    let update_value = {
        let refetch = fetch_hook.refetch.clone();
        let mutation_errors = mutation_errors.clone();

        use_callback((), move |(space_id, value), _| {
            let refetch = refetch.clone();
            let mutation_errors = mutation_errors.clone();

            yew::platform::spawn_local(async move {
                let api_client = get_api_client();
                let request = requests::UserValue { space_id, value };

                match api_client.create_or_update_user_value(&request).await {
                    Ok(_) => {
                        mutation_errors.set(vec![]);
                        refetch.emit(());
                    }
                    Err(e) => {
                        mutation_errors.set(vec![e.to_string()]);
                    }
                }
            });
        })
    };

    let delete_value = {
        let refetch = fetch_hook.refetch.clone();
        let mutation_errors = mutation_errors.clone();

        use_callback((), move |space_id, _| {
            let refetch = refetch.clone();
            let mutation_errors = mutation_errors.clone();

            yew::platform::spawn_local(async move {
                let api_client = get_api_client();
                match api_client.delete_user_value(&space_id).await {
                    Ok(_) => {
                        mutation_errors.set(vec![]);
                        refetch.emit(());
                    }
                    Err(e) => {
                        mutation_errors.set(vec![e.to_string()]);
                    }
                }
            });
        })
    };

    let mutation_errs = (*mutation_errors).clone();
    let inner = fetch_hook.inner.clone().map_err(move |mut errs| {
        errs.extend(mutation_errs);
        errs
    });

    UserSpaceValuesHookReturn {
        inner,
        refetch: fetch_hook.refetch,
        update_value,
        delete_value,
    }
}
