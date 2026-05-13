use payloads::{
    Role, SiteId,
    responses::{CommunityWithRole, Site},
};
use yew::prelude::*;

use crate::components::RequireAuth;
use crate::hooks::{render_section, use_communities, use_site};

#[derive(Clone, PartialEq)]
pub struct SiteWithRole {
    pub site: Site,
    pub community: CommunityWithRole,
}

impl SiteWithRole {
    /// Get the user's role in the site's community
    pub fn user_role(&self) -> Role {
        self.community.user_role
    }
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub site_id: SiteId,
    pub children: Callback<SiteWithRole, Html>,
}

#[function_component]
pub fn SitePageWrapper(props: &Props) -> Html {
    html! {
        <RequireAuth>
            <SitePageWrapperInner
                site_id={props.site_id}
                children={props.children.clone()}
            />
        </RequireAuth>
    }
}

#[function_component]
fn SitePageWrapperInner(props: &Props) -> Html {
    let site_hook = use_site(props.site_id);
    let communities_hook = use_communities();
    let children = props.children.clone();

    render_section(
        &site_hook.inner,
        "site",
        move |site, _is_loading, _errors| {
            let community_id = site.site_details.community_id;

            render_section(&communities_hook.inner, "community membership", {
                let site = site.clone();
                let children = children.clone();

                move |communities, _is_loading, _errors| {
                    let community = communities
                        .iter()
                        .find(|c| c.community.id == community_id)
                        .cloned();

                    match community {
                        Some(community) => {
                            let site_with_role = SiteWithRole {
                                site: site.clone(),
                                community,
                            };
                            html! {
                                <div>
                                    {children.emit(site_with_role)}
                                </div>
                            }
                        }
                        None => {
                            html! {
                                <div class="text-center py-12">
                                    <p class="text-neutral-600 dark:text-neutral-400">
                                        {"Unable to verify community membership"}
                                    </p>
                                </div>
                            }
                        }
                    }
                }
            })
        },
    )
}
