use payloads::{
    CommunityId, responses::CommunityWithRole, responses::UserProfile,
};
use yew::prelude::*;

use crate::components::{
    ActiveTab, CommunityPageWrapper, CommunityTabHeader, RequireAuth,
    TransactionList, TreasuryCreditForm,
};
use crate::hooks::{use_treasury_account, use_treasury_transactions};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub community_id: CommunityId,
}

#[function_component]
pub fn CommunityTreasuryPage(props: &Props) -> Html {
    let community_id = props.community_id;

    html! {
        <RequireAuth render={Callback::from(move |profile: UserProfile| {
            let render_content = Callback::from(move |community: CommunityWithRole| {
                html! {
                    <CommunityTreasuryContent
                        community={community}
                        community_id={community_id}
                        user_profile={profile.clone()}
                    />
                }
            });

            html! {
                <CommunityPageWrapper
                    community_id={community_id}
                    children={render_content}
                />
            }
        })} />
    }
}

#[derive(Properties, PartialEq)]
struct ContentProps {
    pub community: CommunityWithRole,
    pub community_id: CommunityId,
    pub user_profile: UserProfile,
}

#[function_component]
fn CommunityTreasuryContent(props: &ContentProps) -> Html {
    // Check if user is coleader+
    if !props.community.user_role.is_ge_coleader() {
        return html! {
            <div class="min-h-screen bg-neutral-50 dark:bg-neutral-900">
                <CommunityTabHeader
                    community={props.community.clone()}
                    active_tab={ActiveTab::Treasury}
                />
                <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
                    <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-6">
                        <p class="text-red-800 dark:text-red-200">
                            {"Access denied. Treasury features are only available to coleaders and leaders."}
                        </p>
                    </div>
                </div>
            </div>
        };
    }

    let target_user_id = props.user_profile.user_id;

    // Fetch treasury account info
    let treasury_account = use_treasury_account(props.community_id);

    // Fetch treasury transactions (20 per page, offset 0 for now)
    let treasury_transactions =
        use_treasury_transactions(props.community_id, 20, 0);

    html! {
        <div class="min-h-screen bg-neutral-50 dark:bg-neutral-900">
            <CommunityTabHeader
                community={props.community.clone()}
                active_tab={ActiveTab::Treasury}
            />

            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
                <div class="space-y-6">
                    // Treasury Balance Section
                    <div class="bg-white dark:bg-neutral-800 rounded-lg shadow p-6">
                        <h2 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                            {"Treasury Account"}
                        </h2>

                        {
                            if treasury_account.is_loading {
                                html! {
                                    <div class="h-20 bg-neutral-200 dark:bg-neutral-700 rounded animate-pulse"></div>
                                }
                            } else if let Some(error) = &treasury_account.error {
                                html! {
                                    <div class="text-red-600 dark:text-red-400">
                                        {format!("Error loading treasury account: {}", error)}
                                    </div>
                                }
                            } else if let Some(account) = treasury_account.data.as_ref() {
                                html! {
                                    <div class="p-4 bg-neutral-50 dark:bg-neutral-700 rounded">
                                        <div class="text-sm text-neutral-600 dark:text-neutral-400">
                                            {"Balance"}
                                        </div>
                                        <div class="text-3xl font-bold text-neutral-900 dark:text-neutral-100">
                                            {format!("{}{}", props.community.community.currency.symbol, account.balance_cached)}
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>

                    // Treasury Credit Operation Form Section
                    <div class="bg-white dark:bg-neutral-800 rounded-lg shadow p-6">
                        <h2 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                            {"Issue Credits"}
                        </h2>
                        <TreasuryCreditForm
                            community_id={props.community_id}
                            community={props.community.clone()}
                            on_success={Callback::from(move |_| {
                                treasury_account.refetch.emit(());
                                treasury_transactions.refetch.emit(());
                            })}
                        />
                    </div>

                    // Treasury Transaction History Section
                    <div class="bg-white dark:bg-neutral-800 rounded-lg shadow p-6">
                        <h2 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                            {"Treasury Transaction History"}
                        </h2>

                        {
                            if treasury_transactions.is_loading {
                                html! {
                                    <div class="space-y-3">
                                        <div class="h-16 bg-neutral-200 dark:bg-neutral-700 rounded animate-pulse"></div>
                                        <div class="h-16 bg-neutral-200 dark:bg-neutral-700 rounded animate-pulse"></div>
                                        <div class="h-16 bg-neutral-200 dark:bg-neutral-700 rounded animate-pulse"></div>
                                    </div>
                                }
                            } else if let Some(error) = &treasury_transactions.error {
                                html! {
                                    <div class="text-red-600 dark:text-red-400">
                                        {format!("Error loading transactions: {}", error)}
                                    </div>
                                }
                            } else if let Some(txns) = treasury_transactions.data.as_ref() {
                                html! {
                                    <TransactionList
                                        transactions={txns.clone()}
                                        currency_symbol={props.community.community.currency.symbol.clone()}
                                        currency_mode={props.community.community.currency.mode_config.mode()}
                                        target_user_id={target_user_id}
                                    />
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                </div>
            </div>
        </div>
    }
}
