use payloads::{SiteId, SpaceId, requests};
use rust_decimal::Decimal;
use std::collections::HashMap;
use yew::prelude::*;

use crate::get_api_client;

/// Hook return type for user space values
///
/// See module-level documentation in `hooks/mod.rs` for state combination
/// details.
#[derive(Debug)]
#[allow(dead_code)]
pub struct UserSpaceValuesHookReturn {
    pub values: Option<HashMap<SpaceId, Decimal>>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub refetch: Callback<()>,
    pub update_value: Callback<(SpaceId, Decimal)>,
    pub delete_value: Callback<SpaceId>,
}

impl UserSpaceValuesHookReturn {
    /// Returns true if this is the initial load (no data, no error, loading)
    pub fn is_initial_loading(&self) -> bool {
        self.is_loading && self.values.is_none() && self.error.is_none()
    }
}

/// Hook to manage user's max values for spaces at a site
///
/// Fetches all user values for a site and provides methods to update
/// and delete individual values. Values are stored as a HashMap for
/// quick lookup.
#[hook]
pub fn use_user_space_values(site_id: SiteId) -> UserSpaceValuesHookReturn {
    let values = use_state(|| None);
    let error = use_state(|| None);
    let is_loading = use_state(|| false);

    let refetch = {
        let values = values.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(site_id, move |site_id, _| {
            let values = values.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.list_user_values(&site_id).await {
                    Ok(user_values) => {
                        // Convert Vec to HashMap for quick lookup
                        let value_map: HashMap<SpaceId, Decimal> = user_values
                            .into_iter()
                            .map(|uv| (uv.space_id, uv.value))
                            .collect();
                        values.set(Some(value_map));
                        error.set(None);
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }

                is_loading.set(false);
            });
        })
    };

    let update_value = {
        let refetch = refetch.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(site_id, move |(space_id, value), site_id| {
            let refetch = refetch.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();
            let site_id = *site_id;

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                let request = requests::UserValue { space_id, value };

                match api_client.create_or_update_user_value(&request).await {
                    Ok(_) => {
                        error.set(None);
                        // Refetch to ensure UI is updated with latest data
                        refetch.emit(site_id);
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                        is_loading.set(false);
                    }
                }
            });
        })
    };

    let delete_value = {
        let refetch = refetch.clone();
        let error = error.clone();
        let is_loading = is_loading.clone();

        use_callback(site_id, move |space_id, site_id| {
            let refetch = refetch.clone();
            let error = error.clone();
            let is_loading = is_loading.clone();
            let site_id = *site_id;

            yew::platform::spawn_local(async move {
                is_loading.set(true);
                error.set(None);

                let api_client = get_api_client();
                match api_client.delete_user_value(&space_id).await {
                    Ok(_) => {
                        error.set(None);
                        // Refetch to ensure UI is updated with latest data
                        refetch.emit(site_id);
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                        is_loading.set(false);
                    }
                }
            });
        })
    };

    // Auto-load values on mount
    {
        let refetch = refetch.clone();
        let values = values.clone();
        let is_loading = is_loading.clone();

        use_effect_with(site_id, move |site_id| {
            if values.is_none() && !*is_loading {
                refetch.emit(*site_id);
            }
        });
    }

    let current_values = (*values).clone();
    let current_error = (*error).clone();
    let current_is_loading =
        *is_loading || (current_values.is_none() && current_error.is_none());

    UserSpaceValuesHookReturn {
        values: current_values,
        error: current_error,
        is_loading: current_is_loading,
        refetch: Callback::from(move |_| refetch.emit(site_id)),
        update_value,
        delete_value,
    }
}
