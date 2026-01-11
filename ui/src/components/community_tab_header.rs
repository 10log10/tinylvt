use payloads::Role;
use payloads::responses::CommunityWithRole;
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
                <div class="flex items-center justify-between">
                    <h1 class="text-3xl font-bold text-neutral-900 dark:text-neutral-100">
                        {&props.community.name}
                    </h1>
                    {if props.community.user_role == Role::Leader {
                        html! {
                            <Link<Route>
                                to={Route::CommunitySettings { id: props.community.id }}
                                classes="text-neutral-500 hover:text-neutral-700 dark:text-neutral-400 dark:hover:text-neutral-200 transition-colors"
                            >
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    class="h-6 w-6"
                                    fill="none"
                                    viewBox="0 0 24 24"
                                    stroke="currentColor"
                                    stroke-width="2"
                                >
                                    <path
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
                                    />
                                    <path
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                                    />
                                </svg>
                            </Link<Route>>
                        }
                    } else {
                        html! {}
                    }}
                </div>
                <p class="text-lg text-neutral-600 dark:text-neutral-400 mt-2">
                    {"Your role: "}{format!("{:?}", props.community.user_role)}
                </p>
            </div>

            // Tab Navigation
            <div class="border-b border-neutral-200 dark:border-neutral-700">
                <nav class="-mb-px flex space-x-8">
                    <Link<Route>
                        to={Route::CommunityDetail { id: props.community.id }}
                        classes={classes!(format!(
                            "py-2 px-1 border-b-2 font-medium text-sm {}",
                            if props.active_tab == ActiveTab::Sites {
                                "border-neutral-500 text-neutral-600 dark:text-neutral-400"
                            } else {
                                "border-transparent text-neutral-500 hover:text-neutral-700 hover:border-neutral-300 dark:text-neutral-400 dark:hover:text-neutral-300"
                            }
                        ))}
                    >
                        {"Sites"}
                    </Link<Route>>
                    <Link<Route>
                        to={Route::CommunityMembers { id: props.community.id }}
                        classes={classes!(format!(
                            "py-2 px-1 border-b-2 font-medium text-sm {}",
                            if props.active_tab == ActiveTab::Members {
                                "border-neutral-500 text-neutral-600 dark:text-neutral-400"
                            } else {
                                "border-transparent text-neutral-500 hover:text-neutral-700 hover:border-neutral-300 dark:text-neutral-400 dark:hover:text-neutral-300"
                            }
                        ))}
                    >
                        {"Members"}
                    </Link<Route>>

                    {if props.community.user_role.is_ge_moderator() {
                        html! {
                            <Link<Route>
                                to={Route::CommunityInvites { id: props.community.id }}
                                classes={classes!(format!(
                                    "py-2 px-1 border-b-2 font-medium text-sm {}",
                                    if props.active_tab == ActiveTab::Invites {
                                        "border-neutral-500 text-neutral-600 dark:text-neutral-400"
                                    } else {
                                        "border-transparent text-neutral-500 hover:text-neutral-700 hover:border-neutral-300 dark:text-neutral-400 dark:hover:text-neutral-300"
                                    }
                                ))}
                            >
                                {"Invites"}
                            </Link<Route>>
                        }
                    } else {
                        html! {}
                    }}
                </nav>
            </div>
        </div>
    }
}
