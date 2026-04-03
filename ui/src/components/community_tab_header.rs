use payloads::{Role, responses::CommunityWithRole};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;
use crate::components::storage_usage_display::format_bytes;
use crate::hooks::{use_storage_usage, use_title};

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
    Images,
    OrphanedAccounts,
    Billing,
    Settings,
}

struct TabConfig {
    label: &'static str,
    tab: ActiveTab,
    route: fn(payloads::CommunityId) -> Route,
    min_role: Option<Role>,
}

fn get_tab_configs() -> [TabConfig; 9] {
    [
        TabConfig {
            label: "Sites",
            tab: ActiveTab::Sites,
            route: |id| Route::CommunityDetail { id },
            min_role: None,
        },
        TabConfig {
            label: "Images",
            tab: ActiveTab::Images,
            route: |id| Route::CommunityImages { id },
            min_role: Some(Role::Coleader),
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
            label: "Orphaned Accounts",
            tab: ActiveTab::OrphanedAccounts,
            route: |id| Route::OrphanedAccounts { id },
            min_role: Some(Role::Coleader),
        },
        TabConfig {
            label: "Billing",
            tab: ActiveTab::Billing,
            route: |id| Route::CommunityBilling { id },
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

impl ActiveTab {
    fn label(&self) -> &'static str {
        match self {
            ActiveTab::Sites => "Sites",
            ActiveTab::Members => "Members",
            ActiveTab::Invites => "Invites",
            ActiveTab::Currency => "Currency",
            ActiveTab::Treasury => "Treasury",
            ActiveTab::Images => "Images",
            ActiveTab::OrphanedAccounts => "Orphaned Accounts",
            ActiveTab::Billing => "Billing",
            ActiveTab::Settings => "Settings",
        }
    }
}

/// Storage warning badge component for coleader+ users.
/// Only rendered when user has sufficient permissions.
#[derive(Properties, PartialEq)]
struct StorageWarningProps {
    community_id: payloads::CommunityId,
}

#[function_component]
fn StorageWarning(props: &StorageWarningProps) -> Html {
    let storage_hook = use_storage_usage(props.community_id);
    let navigator = use_navigator().unwrap();

    // Only show warning if we have data and usage is ≥75%
    let Some(usage) = storage_hook.data.as_ref() else {
        return html! {};
    };

    let percent = usage.usage_percentage();

    if percent < 75.0 {
        return html! {};
    }

    let total = usage.usage.total_bytes();
    let limit = usage.limits.storage_bytes;

    let (bg_class, text_class, icon) = if percent >= 90.0 {
        (
            "bg-red-100 dark:bg-red-900/30",
            "text-red-700 dark:text-red-400",
            "!",
        )
    } else {
        (
            "bg-amber-100 dark:bg-amber-900/30",
            "text-amber-700 dark:text-amber-400",
            "!",
        )
    };

    let onclick = {
        let community_id = props.community_id;
        let navigator = navigator.clone();
        Callback::from(move |_| {
            navigator.push(&Route::CommunityBilling { id: community_id });
        })
    };

    html! {
        <button
            {onclick}
            class={classes!(
                "flex", "items-center", "gap-1.5", "px-2.5", "py-1",
                "rounded-md", "text-sm", "font-medium", bg_class, text_class,
                "hover:opacity-80", "transition-opacity",
                "border-none", "cursor-pointer"
            )}
        >
            <span class="font-bold">{icon}</span>
            <span>
                {"Storage: "}
                {format_bytes(total)}
                {"/"}
                {format_bytes(limit)}
            </span>
        </button>
    }
}

#[function_component]
pub fn CommunityTabHeader(props: &Props) -> Html {
    use_title(&format!(
        "{} - {} - TinyLVT",
        props.community.name,
        props.active_tab.label()
    ));

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
            <div class="flex flex-col sm:flex-row sm:items-start sm:justify-between gap-4">
                <div>
                    <h1 class="text-3xl font-bold text-neutral-900 dark:text-neutral-100">
                        {&props.community.name}
                    </h1>
                    <p class="text-lg text-neutral-600 dark:text-neutral-400 mt-2">
                        {"Your role: "}{format!("{:?}", props.community.user_role)}
                    </p>
                </div>
                // Storage warning for coleader+ users
                {if props.community.user_role.is_ge_coleader() {
                    html! {
                        <StorageWarning community_id={props.community.id} />
                    }
                } else {
                    html! {}
                }}
            </div>

            // Tab Navigation
            <div class="border-b border-neutral-200 dark:border-neutral-700">
                <nav class="-mb-px flex flex-wrap gap-x-8 gap-y-2">
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
