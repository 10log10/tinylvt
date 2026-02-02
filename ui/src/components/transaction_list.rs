use payloads::{
    CurrencyMode, CurrencySettings, UserId,
    responses::{MemberTransaction, TransactionParty},
};
use yew::prelude::*;

#[derive(Properties)]
pub struct Props {
    pub transactions: Vec<MemberTransaction>,
    pub currency: CurrencySettings,
    pub target_user_id: UserId,
}

impl PartialEq for Props {
    fn eq(&self, other: &Self) -> bool {
        self.currency == other.currency
            && self.target_user_id == other.target_user_id
            && self.transactions.len() == other.transactions.len()
    }
}

#[function_component]
pub fn TransactionList(props: &Props) -> Html {
    if props.transactions.is_empty() {
        return html! {
            <div class="text-center py-8 text-neutral-600 dark:text-neutral-400">
                {"No transactions yet"}
            </div>
        };
    }

    html! {
        <div class="space-y-3">
            {
                props.transactions.iter().map(|txn| {
                    html! {
                        <TransactionRow
                            transaction={txn.clone()}
                            currency={props.currency.clone()}
                            target_user_id={props.target_user_id}
                        />
                    }
                }).collect::<Html>()
            }
        </div>
    }
}

#[derive(Properties)]
struct TransactionRowProps {
    pub transaction: MemberTransaction,
    pub currency: CurrencySettings,
    pub target_user_id: UserId,
}

impl PartialEq for TransactionRowProps {
    fn eq(&self, other: &Self) -> bool {
        self.currency == other.currency
            && self.target_user_id == other.target_user_id
    }
}

#[function_component]
fn TransactionRow(props: &TransactionRowProps) -> Html {
    let txn = &props.transaction;

    // Format entry type for display
    let entry_type_label = match txn.entry_type {
        payloads::EntryType::IssuanceGrantSingle => "Allowance",
        payloads::EntryType::IssuanceGrantBulk => "Allowance (Bulk)",
        payloads::EntryType::CreditPurchase => "Credit Purchase",
        payloads::EntryType::DistributionCorrection => {
            "Distribution Correction"
        }
        payloads::EntryType::DebtSettlement => "Debt Settlement",
        payloads::EntryType::AuctionSettlement => "Auction Settlement",
        payloads::EntryType::Transfer => "Transfer",
        payloads::EntryType::BalanceReset => "Balance Reset",
    };

    // Determine counterparty based on entry type and currency mode
    let counterparty = match txn.entry_type {
        payloads::EntryType::IssuanceGrantSingle
        | payloads::EntryType::IssuanceGrantBulk
        | payloads::EntryType::CreditPurchase
        | payloads::EntryType::DistributionCorrection
        | payloads::EntryType::DebtSettlement
        | payloads::EntryType::BalanceReset => "Treasury",
        payloads::EntryType::AuctionSettlement => match props.currency.mode() {
            CurrencyMode::DistributedClearing => "The Community",
            CurrencyMode::PointsAllocation
            | CurrencyMode::DeferredPayment
            | CurrencyMode::PrepaidCredits => "Treasury",
        },
        payloads::EntryType::Transfer => {
            // For transfers, find the other party (not target user)
            txn.lines
                .iter()
                .find_map(|line| match &line.party {
                    TransactionParty::Member(identity) => {
                        if identity.user_id != props.target_user_id {
                            Some(
                                identity
                                    .display_name
                                    .as_ref()
                                    .unwrap_or(&identity.username)
                                    .as_str(),
                            )
                        } else {
                            None
                        }
                    }
                    TransactionParty::Treasury => None,
                })
                .unwrap_or("Unknown")
        }
    };

    // Net amount for the target user only (not all lines which sum to
    // zero)
    let net_amount: rust_decimal::Decimal = txn
        .lines
        .iter()
        .filter(|line| match &line.party {
            TransactionParty::Member(identity) => {
                identity.user_id == props.target_user_id
            }
            TransactionParty::Treasury => false,
        })
        .map(|line| line.amount)
        .sum();

    // Determine if this is a debit or credit
    let is_credit = net_amount > rust_decimal::Decimal::ZERO;

    // Format timestamp
    let timestamp_display = {
        use jiff::tz::TimeZone;
        let system_tz = TimeZone::system();
        let zoned = txn.created_at.to_zoned(system_tz);
        zoned.strftime("%b %d, %Y %I:%M %p").to_string()
    };

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 rounded-lg p-4 bg-white dark:bg-neutral-800">
            <div class="flex items-start justify-between">
                <div class="flex-1">
                    <div class="flex items-center gap-2">
                        <span class="font-medium text-neutral-900 dark:text-neutral-100">
                            {entry_type_label}
                        </span>
                        <span class="text-sm text-neutral-600 dark:text-neutral-400">
                            {if is_credit { "from" } else { "to" }}
                            {" "}
                            {counterparty}
                        </span>
                    </div>

                    <div class="text-sm text-neutral-600 dark:text-neutral-400 mt-1">
                        {&timestamp_display}
                    </div>

                    {
                        if let Some(note) = &txn.note {
                            html! {
                                <div class="text-sm text-neutral-700 dark:text-neutral-300 mt-2 italic">
                                    {"\""}{note}{"\""}
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                <div class={classes!(
                    "text-lg", "font-semibold",
                    if is_credit {
                        classes!("text-green-600", "dark:text-green-400")
                    } else {
                        classes!("text-red-600", "dark:text-red-400")
                    }
                )}>
                    {if is_credit { "+" } else { "" }}
                    {props.currency.format_amount(net_amount.abs())}
                </div>
            </div>

            // Show transaction lines (for debugging/transparency)
            {
                if txn.lines.len() > 2 {
                    html! {
                        <details class="mt-3">
                            <summary class="text-xs text-neutral-500 dark:text-neutral-400 cursor-pointer">
                                {"View details"}
                            </summary>
                            <div class="mt-2 space-y-1 text-xs">
                                {
                                    txn.lines.iter().map(|line| {
                                        let party_name = match &line.party {
                                            TransactionParty::Member(identity) => {
                                                identity.display_name.as_ref()
                                                    .unwrap_or(&identity.username).clone()
                                            }
                                            TransactionParty::Treasury => "Treasury".to_string(),
                                        };

                                        html! {
                                            <div class="flex justify-between text-neutral-600 dark:text-neutral-400">
                                                <span>{party_name}</span>
                                                <span>
                                                    {if line.amount > rust_decimal::Decimal::ZERO { "+" } else { "" }}
                                                    {props.currency.format_amount(line.amount.abs())}
                                                </span>
                                            </div>
                                        }
                                    }).collect::<Html>()
                                }
                            </div>
                        </details>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
