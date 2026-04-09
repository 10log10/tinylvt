use std::collections::HashMap;

use payloads::{CurrencySettings, RoundSpaceResult, SpaceId, responses};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::components::user_identity_display::render_user_name;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    /// (space_id, name) pairs in display order
    pub spaces: Vec<(SpaceId, String)>,
    /// Standing high bidders (results from previous round)
    pub results: Vec<RoundSpaceResult>,
    /// Bids placed this round: space_id -> list of bidders
    pub bids: HashMap<SpaceId, Vec<responses::UserIdentity>>,
    /// Maximum price for scaling bar widths
    pub x_max: Decimal,
    pub currency: CurrencySettings,
}

// Desktop grid: space | bar | price | high-bid | new-bids
// Mobile: space | price | high-bid | new-bids (row 1),
//         bar spanning full width (row 2)
//
// Grid column template:
//   sm+: auto 1fr auto auto auto
//   <sm: 1fr auto auto auto (no bar column; bar on its own row)

const GRID_CLASSES: &str = "\
    grid gap-x-3 gap-y-1 items-center \
    grid-cols-[1fr_auto_auto_6rem] \
    sm:grid-cols-[auto_1fr_auto_auto_6rem]";

const HEADING_CLASSES: &str = "\
    text-xs font-medium text-neutral-500 \
    dark:text-neutral-500 uppercase tracking-wide";

#[function_component]
pub fn AuctionChart(props: &Props) -> Html {
    let x_max_f64 = props.x_max.to_f64().unwrap_or(1.0);

    html! {
        <div class={GRID_CLASSES}>
            // Column headings
            <div class={classes!(
                HEADING_CLASSES, "sm:text-right"
            )}>
                {"Space"}
            </div>
            // Bar heading (hidden on mobile)
            <div class="hidden sm:block" />
            <div class={classes!(HEADING_CLASSES, "text-right")}>
                {"Price"}
            </div>
            <div class={HEADING_CLASSES}>
                {"High bid"}
            </div>
            <div class={HEADING_CLASSES}>
                {"New bids"}
            </div>

            {for props.spaces.iter().map(|(space_id, name)| {
                let result = props.results.iter()
                    .find(|r| r.space_id == *space_id);
                let space_bids = props.bids.get(space_id);

                let bar_html = {
                    let pct = result.map(|r| {
                        if x_max_f64 > 0.0 {
                            r.value.to_f64().unwrap_or(0.0)
                                / x_max_f64 * 100.0
                        } else {
                            0.0
                        }
                    }).unwrap_or(0.0);
                    let width_style =
                        format!("width: {:.1}%", pct);
                    html! {
                        <div class="h-6 bg-neutral-100 \
                            dark:bg-neutral-800 rounded \
                            overflow-hidden">
                            <div class="h-full bg-neutral-400 \
                                dark:bg-neutral-500 rounded"
                                style={width_style}
                            />
                        </div>
                    }
                };

                html! {
                    <>
                        // Space name
                        <div class="text-sm font-medium \
                            text-neutral-700 dark:text-neutral-300 \
                            truncate sm:text-right">
                            {name}
                        </div>

                        // Bar (desktop: inline column, mobile: hidden here)
                        <div class="hidden sm:block">
                            {bar_html.clone()}
                        </div>

                        // Price
                        <div class="text-sm \
                            text-neutral-600 dark:text-neutral-400 \
                            text-right tabular-nums">
                            {if let Some(r) = result {
                                props.currency.format_amount(r.value)
                            } else {
                                "\u{2014}".to_string()
                            }}
                        </div>

                        // Standing high bidder
                        <div class="text-sm \
                            text-neutral-700 dark:text-neutral-300 \
                            truncate">
                            {if let Some(r) = result {
                                render_user_name(&r.winner)
                            } else {
                                html! {"\u{2014}"}
                            }}
                        </div>

                        // New bids
                        <div class="text-xs \
                            text-neutral-500 dark:text-neutral-500 \
                            truncate">
                            {if let Some(bidders) = space_bids {
                                bidders.iter().enumerate()
                                    .map(|(i, b)| html! {
                                        <>
                                            {if i > 0 {
                                                html! {", "}
                                            } else {
                                                html! {}
                                            }}
                                            {render_user_name(b)}
                                        </>
                                    })
                                    .collect::<Html>()
                            } else {
                                html! {"\u{2014}"}
                            }}
                        </div>

                        // Bar (mobile: spans full width below,
                        // desktop: hidden)
                        <div class="sm:hidden \
                            col-span-full mb-1">
                            {bar_html}
                        </div>
                    </>
                }
            })}
        </div>
    }
}
