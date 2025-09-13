use payloads::CommunityId;
use yew::prelude::*;

use crate::components::{ActiveTab, CommunityTabHeader};
use crate::hooks::use_communities;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: String,
    pub active_tab: ActiveTab,
    pub children: Callback<CommunityId, Html>,
}

#[function_component]
pub fn CommunityPageWrapper(props: &Props) -> Html {
    let communities_hook = use_communities();

    // Parse community ID from string
    let community_id = match uuid::Uuid::parse_str(&props.community_id) {
        Ok(id) => CommunityId(id),
        Err(_) => {
            return html! {
                <div class="text-center py-12">
                    <p class="text-red-600 dark:text-red-400">{"Invalid community ID"}</p>
                </div>
            };
        }
    };

    // Find the community in the global state
    let community =
        communities_hook
            .communities
            .as_ref()
            .and_then(|communities| {
                communities.iter().find(|c| c.id == community_id)
            });

    if communities_hook.is_loading {
        return html! {
            <div class="text-center py-12">
                <p class="text-neutral-600 dark:text-neutral-400">{"Loading community..."}</p>
            </div>
        };
    }

    if let Some(error) = &communities_hook.error {
        return html! {
            <div class="p-4 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                <p class="text-sm text-red-700 dark:text-red-400">{error}</p>
            </div>
        };
    }

    let community = match community {
        Some(c) => c,
        None => {
            return html! {
                <div class="text-center py-12">
                    <p class="text-neutral-600 dark:text-neutral-400">{"Community not found"}</p>
                </div>
            };
        }
    };

    html! {
        <div>
            <CommunityTabHeader community={community.clone()} active_tab={props.active_tab.clone()} />

            <div class="py-6">
                {props.children.emit(community_id)}
            </div>
        </div>
    }
}
