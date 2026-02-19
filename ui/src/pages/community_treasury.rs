use payloads::{AccountOwner, CommunityId, responses::CommunityWithRole};
use yew::prelude::*;

use crate::components::{
    ActiveTab, CommunityPageWrapper, CommunityTabHeader, PaginationControls,
    RequireAuth, ResetBalancesButton, TransactionList, TreasuryCreditForm,
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
        <RequireAuth render={Callback::from(move |_profile| {
            let render_content = Callback::from(move |community: CommunityWithRole| {
                html! {
                    <CommunityTreasuryContent
                        community={community}
                        community_id={community_id}
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

    // Pagination state
    let offset = use_state(|| 0i64);
    let limit = 20i64;

    // Fetch treasury account info
    let treasury_account = use_treasury_account(props.community_id);

    // Fetch treasury transactions
    let treasury_transactions =
        use_treasury_transactions(props.community_id, limit, *offset);

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
                                            {props.community.community.currency.format_amount(account.balance_cached)}
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

                        // Explanatory text based on currency mode
                        <div class="mb-4 p-4 bg-neutral-50 dark:bg-neutral-700 rounded text-sm text-neutral-700 dark:text-neutral-300">
                        {
                            match props.community.community.currency.mode_config.mode() {
                                payloads::CurrencyMode::PointsAllocation => html! {
                                    <p>
                                        {"Treasury operations in "}
                                        <span class="font-semibold">{"Points Allocation"}</span>
                                        {" mode are used to issue allowances to members."}
                                    </p>
                                },
                                payloads::CurrencyMode::DistributedClearing => html! {
                                    <>
                                        <p class="mb-2">
                                            {"Treasury operations in "}
                                            <span class="font-semibold">{"Distributed Clearing"}</span>
                                            {" mode are only needed when an auction balance was sent to the treasury."}
                                        </p>
                                        <p>
                                            {"This occurs when no members were \"active\" during an auction. Only active members are eligible to receive auction distributions. Leaders must manually correct the failed allocation after fixing the lack of active members."}
                                        </p>
                                    </>
                                },
                                payloads::CurrencyMode::DeferredPayment => html! {
                                    <>
                                        <p class="mb-2">
                                            {"Treasury operations in "}
                                            <span class="font-semibold">{"Deferred Payment"}</span>
                                            {" mode are used to mark debts as paid."}
                                        </p>
                                        <p>
                                            {"Payment occurs outside TinyLVT. Use this operation to record that a member has settled their debt."}
                                        </p>
                                    </>
                                },
                                payloads::CurrencyMode::PrepaidCredits => html! {
                                    <>
                                        <p class="mb-2">
                                            {"Treasury operations in "}
                                            <span class="font-semibold">{"Prepaid Credits"}</span>
                                            {" mode are used to record credit purchases."}
                                        </p>
                                        <p>
                                            {"Members purchase credits from the treasury to use in auctions. Record each purchase with this operation."}
                                        </p>
                                    </>
                                },
                            }
                        }
                        </div>

                        <TreasuryCreditForm
                            community_id={props.community_id}
                            community={props.community.clone()}
                            on_success={{
                                let treasury_account = treasury_account.refetch.clone();
                                let treasury_transactions = treasury_transactions.refetch.clone();
                                let offset_handle = offset.clone();
                                Callback::from(move |_| {
                                    treasury_account.emit(());
                                    treasury_transactions.emit(());
                                    offset_handle.set(0);
                                })
                            }}
                        />
                    </div>

                    // Reset All Balances Section
                    <div class="bg-white dark:bg-neutral-800 rounded-lg shadow p-6">
                        <h2 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-4">
                            {"Reset All Balances"}
                        </h2>
                        <p class="text-sm text-neutral-600 dark:text-neutral-400 mb-4">
                            {"Transfer all member balances to the treasury. This operation cannot be performed during active auctions."}
                        </p>
                        <ResetBalancesButton
                            community_id={props.community_id}
                            on_success={{
                                let treasury_account = treasury_account.refetch.clone();
                                let treasury_transactions = treasury_transactions.refetch.clone();
                                let offset_handle = offset.clone();
                                Callback::from(move |_| {
                                    treasury_account.emit(());
                                    treasury_transactions.emit(());
                                    offset_handle.set(0);
                                })
                            }}
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
                                let offset_handle = offset.clone();
                                let on_offset_change = Callback::from(move |new_offset: i64| {
                                    offset_handle.set(new_offset);
                                });

                                html! {
                                    <>
                                        <TransactionList
                                            transactions={txns.clone()}
                                            currency={props.community.community.currency.clone()}
                                            target_account={AccountOwner::Treasury}
                                        />
                                        <PaginationControls
                                            offset={*offset}
                                            limit={limit}
                                            current_count={txns.len()}
                                            on_offset_change={on_offset_change}
                                            is_loading={treasury_transactions.is_loading}
                                        />
                                    </>
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
