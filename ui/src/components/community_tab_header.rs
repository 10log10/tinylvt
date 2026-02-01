use payloads::{Role, responses::CommunityWithRole};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community: CommunityWithRole,
    pub active_tab: ActiveTab,
}

#[derive(PartialEq, Clone)]
pub enum ActiveTab {
    Sites,
    Members,
    Invites,
    Currency,
    Treasury,
    Settings,
}

struct TabConfig {
    label: &'static str,
    tab: ActiveTab,
    route: fn(payloads::CommunityId) -> Route,
    min_role: Option<Role>,
}

fn get_tab_configs() -> [TabConfig; 6] {
    [
        TabConfig {
            label: "Sites",
            tab: ActiveTab::Sites,
            route: |id| Route::CommunityDetail { id },
            min_role: None,
        },
        TabConfig {
            label: "Members",
            tab: ActiveTab::Members,
            route: |id| Route::CommunityMembers { id },
            min_role: None,
        },
        TabConfig {
            label: "Invites",
            tab: ActiveTab::Invites,
            route: |id| Route::CommunityInvites { id },
            min_role: Some(Role::Moderator),
        },
        TabConfig {
            label: "Currency",
            tab: ActiveTab::Currency,
            route: |id| Route::CommunityCurrency { id },
            min_role: None,
        },
        TabConfig {
            label: "Treasury",
            tab: ActiveTab::Treasury,
            route: |id| Route::CommunityTreasury { id },
            min_role: Some(Role::Coleader),
        },
        TabConfig {
            label: "Settings",
            tab: ActiveTab::Settings,
            route: |id| Route::CommunitySettings { id },
            min_role: None,
        },
    ]
}

#[function_component]
pub fn CommunityTabHeader(props: &Props) -> Html {
    html! {
        <div class="space-y-8">
            // Back Navigation
            <Link<Route>
                to={Route::Communities}
                classes="inline-flex items-center text-sm text-neutral-600 hover:text-neutral-800 dark:text-neutral-400 dark:hover:text-neutral-200"
            >
                {"← Back to Communities"}
            </Link<Route>>

            // Header
            <div>
                <h1 class="text-3xl font-bold text-neutral-900 dark:text-neutral-100">
                    {&props.community.name}
                </h1>
                <p class="text-lg text-neutral-600 dark:text-neutral-400 mt-2">
                    {"Your role: "}{format!("{:?}", props.community.user_role)}
                </p>
            </div>

            // Tab Navigation
            <div class="border-b border-neutral-200 dark:border-neutral-700">
                <nav class="-mb-px flex space-x-8">
                    {get_tab_configs().iter().filter_map(|tab_config| {
                        // Check if user has required role
                        let has_permission = match tab_config.min_role {
                            None => true,
                            Some(Role::Moderator) => {
                                props.community.user_role.is_ge_moderator()
                            }
                            Some(Role::Coleader) => {
                                props.community.user_role.is_ge_coleader()
                            }
                            Some(Role::Leader) => {
                                props.community.user_role.is_leader()
                            }
                            Some(Role::Member) => true,
                        };

                        if !has_permission {
                            return None;
                        }

                        let is_active = props.active_tab == tab_config.tab;
                        let classes = format!(
                            "py-2 px-1 border-b-2 font-medium text-sm {}",
                            if is_active {
                                "border-neutral-500 text-neutral-600 \
                                 dark:text-neutral-400"
                            } else {
                                "border-transparent text-neutral-500 \
                                 hover:text-neutral-700 hover:border-neutral-300 \
                                 dark:text-neutral-400 dark:hover:text-neutral-300"
                            }
                        );

                        Some(html! {
                            <Link<Route>
                                to={(tab_config.route)(props.community.id)}
                                classes={classes!(classes)}
                            >
                                {tab_config.label}
                            </Link<Route>>
                        })
                    }).collect::<Html>()}
                </nav>
            </div>
        </div>
    }
}
