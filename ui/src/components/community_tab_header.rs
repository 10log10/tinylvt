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
