use std::collections::HashMap;

use payloads::{
    CurrencySettings, RoundSpaceResult, SpaceId, UserId, responses,
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::components::price_chart::space_color_style;
use crate::components::subway_diagram::{
    bidder_color_style, bidder_pill_needs_dark_text,
};
use crate::components::user_identity_display::render_user_name;
use crate::utils::capitalize;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    /// (space_id, name) pairs in display order. Same ordering as PriceChart so
    /// space colors line up across the two views.
    pub spaces: Vec<(SpaceId, String)>,
    /// Global bidder list in the same order used by SubwayDiagram, so bidder
    /// pill colors match the subway lanes.
    pub bidders: Vec<responses::UserIdentity>,
    /// Standing high bidders (results from previous round)
    pub results: Vec<RoundSpaceResult>,
    /// Bids placed this round: space_id -> list of bidders
    pub bids: HashMap<SpaceId, Vec<responses::UserIdentity>>,
    /// Maximum price for scaling bar widths
    pub x_max: Decimal,
    pub currency: CurrencySettings,
    pub item_term: &'static str,
}

// Desktop grid: space | bar | price | high-bid | new-bids
// Mobile: space | price | high-bid | new-bids (row 1),
//         bar spanning full width (row 2)
//
// Grid column template:
//   sm+: auto 1fr auto auto auto
//   <sm: 1fr auto auto auto (no bar column; bar on its own row)

// Price column uses minmax(4rem, auto): stays wide enough for typical values so
// the bar column doesn't reflow as digit counts grow, but can still expand for
// unusually large prices.
const GRID_CLASSES: &str = "\
    grid gap-x-3 gap-y-1 items-center \
    grid-cols-[1fr_minmax(4rem,auto)_auto_6rem] \
    sm:grid-cols-[auto_1fr_minmax(4rem,auto)_auto_6rem]";

const HEADING_CLASSES: &str = "\
    text-xs font-medium text-neutral-500 \
    dark:text-neutral-500 uppercase tracking-wide";

#[function_component]
pub fn AuctionChart(props: &Props) -> Html {
    let x_max_f64 = props.x_max.to_f64().unwrap_or(1.0);

    // Bidder -> palette index, matching the order SubwayDiagram assigns.
    let bidder_idx: HashMap<UserId, usize> = props
        .bidders
        .iter()
        .enumerate()
        .map(|(i, b)| (b.user_id, i))
        .collect();

    html! {
        <div class={GRID_CLASSES}>
            // Column headings
            <div class={classes!(
                HEADING_CLASSES, "sm:text-right"
            )}>
                {capitalize(props.item_term)}
            </div>
            // Bar heading (hidden on mobile)
            <div class="hidden sm:block" />
            <div class={classes!(HEADING_CLASSES, "-ml-2", "text-right")}>
                {"Price"}
            </div>
            <div class={HEADING_CLASSES}>
                {"High bid"}
            </div>
            <div class={HEADING_CLASSES}>
                {"New bids"}
            </div>

            {for props.spaces.iter().enumerate().map(
                |(space_idx, (space_id, name))| {
                let result = props.results.iter()
                    .find(|r| r.space_id == *space_id);
                let space_bids = props.bids.get(space_id);
                let space_style = space_color_style(space_idx);

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
                        // Space name — pill tinted with the space color so it
                        // ties to the line in PriceChart and the band in
                        // SubwayDiagram.
                        <div class="text-sm truncate sm:text-right">
                            <span
                                class="inline-block px-1.5 py-0.5 rounded \
                                    font-medium \
                                    text-neutral-900 \
                                    bg-[var(--space-light)] \
                                    dark:bg-[var(--space-dark)]"
                                style={space_style.clone()}
                            >
                                {name}
                            </span>
                        </div>

                        // Bar (desktop: inline column, mobile: hidden here)
                        <div class="hidden sm:block">
                            {bar_html.clone()}
                        </div>

                        // Price
                        <div class="-ml-2 text-sm \
                            text-neutral-600 dark:text-neutral-400 \
                            text-right tabular-nums">
                            {if let Some(r) = result {
                                props.currency.format_amount(r.value)
                            } else {
                                "\u{2014}".to_string()
                            }}
                        </div>

                        // Standing high bidder
                        <div class="text-sm truncate">
                            {if let Some(r) = result {
                                render_bidder_pill(&r.winner, &bidder_idx)
                            } else {
                                html! {
                                    <span class="text-neutral-700 \
                                        dark:text-neutral-300">
                                        {"\u{2014}"}
                                    </span>
                                }
                            }}
                        </div>

                        // New bids
                        <div class="text-xs truncate space-x-1">
                            {if let Some(bidders) = space_bids {
                                bidders.iter()
                                    .map(|b| render_bidder_pill(
                                        b, &bidder_idx
                                    ))
                                    .collect::<Html>()
                            } else {
                                html! {
                                    <span class="text-neutral-500 \
                                        dark:text-neutral-500">
                                        {"\u{2014}"}
                                    </span>
                                }
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

/// Render a bidder name inside a colored pill so it can be visually linked to
/// the matching lane in SubwayDiagram. Falls back to a neutral label when the
/// bidder is unknown to the global bidder list.
fn render_bidder_pill(
    bidder: &responses::UserIdentity,
    bidder_idx: &HashMap<UserId, usize>,
) -> Html {
    let Some(&idx) = bidder_idx.get(&bidder.user_id) else {
        return html! {
            <span class="inline-block text-neutral-700 \
                dark:text-neutral-300">
                {render_user_name(bidder)}
            </span>
        };
    };
    let style = bidder_color_style(idx);
    let text_class = if bidder_pill_needs_dark_text(idx) {
        "text-neutral-900"
    } else {
        "text-white"
    };
    html! {
        <span
            class={classes!(
                "inline-block", "px-1.5", "py-0.5", "rounded",
                "bg-[var(--subway-light)]",
                "dark:bg-[var(--subway-dark)]",
                text_class,
            )}
            style={style}
        >
            {render_user_name(bidder)}
        </span>
    }
}
