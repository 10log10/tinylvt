//! Note that for real auctions, the equal share payout is not necessarily based
//! upon auction participation. Instead, pass in the transaction that references
//! this auction to display actual payments.

use payloads::responses::UserIdentity;
use payloads::{CurrencySettings, RoundSpaceResult, SpaceId, UserId};
use rust_decimal::Decimal;
use yew::prelude::*;

use crate::components::user_identity_display::render_user_name;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub spaces: Vec<(SpaceId, String)>,
    pub bidders: Vec<UserIdentity>,
    /// Final round results (winners and prices)
    pub results: Vec<RoundSpaceResult>,
    pub currency: CurrencySettings,
}

const MUTED: &str = "text-neutral-500 dark:text-neutral-500";
const LABEL: &str = "text-neutral-700 dark:text-neutral-300";
const BOLD: &str = "font-medium text-neutral-900 dark:text-neutral-100";

struct WinnerEntry {
    user: UserIdentity,
    space_names: Vec<String>,
    total_price: Decimal,
}

#[function_component]
pub fn AuctionSettlement(props: &Props) -> Html {
    let total_proceeds: Decimal = props.results.iter().map(|r| r.value).sum();
    let n = props.bidders.len();
    if n == 0 {
        return html! {};
    }
    let share = total_proceeds / Decimal::from(n);

    // Group wins by bidder, preserving space order
    let mut winner_entries: Vec<WinnerEntry> = Vec::new();
    let mut winner_ids: Vec<UserId> = Vec::new();

    for (sid, name) in &props.spaces {
        if let Some(r) = props.results.iter().find(|r| r.space_id == *sid) {
            if let Some(entry) = winner_entries
                .iter_mut()
                .find(|e| e.user.user_id == r.winner.user_id)
            {
                entry.space_names.push(name.clone());
                entry.total_price += r.value;
            } else {
                winner_ids.push(r.winner.user_id);
                winner_entries.push(WinnerEntry {
                    user: r.winner.clone(),
                    space_names: vec![name.clone()],
                    total_price: r.value,
                });
            }
        }
    }

    // Non-winners: bidders who didn't win any space
    let non_winners: Vec<_> = props
        .bidders
        .iter()
        .filter(|b| !winner_ids.contains(&b.user_id))
        .collect();

    let fmt = |amount: Decimal| props.currency.format_amount(amount);

    let format_adjustment = |adj: Decimal| -> String {
        if adj >= Decimal::ZERO {
            format!("+{}", fmt(adj))
        } else {
            fmt(adj)
        }
    };

    html! {
        <div>
            // Totals
            <div class={classes!(
                "flex", "flex-wrap", "gap-x-6", "gap-y-1",
                "text-sm", "mb-3", MUTED
            )}>
                <span>
                    {"Total proceeds: "}
                    <span class={BOLD}>
                        {fmt(total_proceeds)}
                    </span>
                </span>
                <span>
                    {"Equal share (1/"}
                    {n.to_string()}
                    {"): "}
                    <span class={BOLD}>
                        {fmt(share)}
                    </span>
                </span>
            </div>

            // Winner adjustments
            <div class="space-y-2 sm:space-y-1">
                {for winner_entries.iter().map(|entry| {
                    let adjustment = share - entry.total_price;
                    let spaces_str = entry.space_names.join(", ");
                    html! {
                        <div class="flex flex-col sm:flex-row \
                            sm:items-center sm:gap-3 text-sm">
                            <div class="flex items-center \
                                gap-2 sm:gap-3 min-w-0 \
                                sm:flex-none">
                                <span class={classes!(
                                    "sm:w-20", "truncate", LABEL
                                )}>
                                    {render_user_name(&entry.user)}
                                </span>
                                <span class={MUTED}>
                                    {"\u{2192}"}
                                </span>
                                <span class={classes!(
                                    "sm:w-28", "truncate", LABEL
                                )}>
                                    {spaces_str}
                                </span>
                            </div>
                            <div class="flex items-center gap-1">
                                <span class={classes!(
                                    "tabular-nums",
                                    "whitespace-nowrap", MUTED
                                )}>
                                    {fmt(share)}
                                    {" \u{2212} "}
                                    {fmt(entry.total_price)}
                                    {" = "}
                                </span>
                                <span class={classes!(
                                    "tabular-nums",
                                    "whitespace-nowrap", BOLD
                                )}>
                                    {format_adjustment(adjustment)}
                                </span>
                            </div>
                        </div>
                    }
                })}

                // Non-winners receive their share
                {for non_winners.iter().map(|&bidder| {
                    html! {
                        <div class="flex flex-col sm:flex-row \
                            sm:items-center sm:gap-3 text-sm">
                            <div class="flex items-center \
                                gap-2 sm:gap-3 min-w-0 \
                                sm:flex-none">
                                <span class={classes!(
                                    "sm:w-20", "truncate", LABEL
                                )}>
                                    {render_user_name(bidder)}
                                </span>
                                <span class={MUTED}>
                                    {"\u{2192}"}
                                </span>
                                <span class={classes!(
                                    "sm:w-28", "truncate",
                                    "italic", MUTED
                                )}>
                                    {"none"}
                                </span>
                            </div>
                            <div class="flex items-center gap-1">
                                <span class={classes!(
                                    "tabular-nums",
                                    "whitespace-nowrap", MUTED
                                )}>
                                    {fmt(share)}
                                    {" \u{2212} 0 = "}
                                </span>
                                <span class={classes!(
                                    "tabular-nums",
                                    "whitespace-nowrap", BOLD
                                )}>
                                    {format_adjustment(share)}
                                </span>
                            </div>
                        </div>
                    }
                })}
            </div>
        </div>
    }
}
