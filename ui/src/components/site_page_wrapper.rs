use payloads::{Role, SiteId, responses::Site};
use yew::prelude::*;

use crate::hooks::{login_form, use_communities, use_require_auth, use_site};

#[derive(Clone, PartialEq)]
pub struct SiteWithRole {
    pub site: Site,
    pub user_role: Role,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site_id: SiteId,
    pub children: Callback<SiteWithRole, Html>,
}

#[function_component]
pub fn SitePageWrapper(props: &Props) -> Html {
    // Require authentication - shows login form if not authenticated
    if use_require_auth().is_none() {
        return login_form();
    }

    let site_hook = use_site(props.site_id);
    let communities_hook = use_communities();

    if site_hook.is_loading {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">{"Loading site..."}</p>
            </div>
        };
    }

    if let Some(error) = &site_hook.error {
        return html! {
            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
            </div>
        };
    }

    let site = match &site_hook.site {
        Some(s) => s,
        None => {
            return html! {
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">{"Site not found"}</p>
                </div>
            };
        }
    };

    // Find the user's role in this site's community
    let user_role =
        communities_hook
            .communities
            .as_ref()
            .and_then(|communities| {
                communities
                    .iter()
                    .find(|c| c.id == site.site_details.community_id)
                    .map(|c| c.user_role)
            });

    let user_role = match user_role {
        Some(role) => role,
        None => {
            return html! {
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">{"Unable to verify community membership"}</p>
                </div>
            };
        }
    };

    let site_with_role = SiteWithRole {
        site: site.clone(),
        user_role,
    };

    html! {
        <div>
            {props.children.emit(site_with_role)}
        </div>
    }
}
