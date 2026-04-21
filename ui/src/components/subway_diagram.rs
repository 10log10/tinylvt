use std::collections::{HashMap, HashSet};

use payloads::auction_sim::SimRound;
use payloads::{CurrencySettings, SpaceId, UserId, responses};
use rust_decimal::Decimal;
use yew::prelude::*;

use crate::components::user_identity_display::{
    format_user_name_unambiguous, render_user_name,
};

#[derive(Properties, PartialEq)]
pub struct Props {
    /// (space_id, name) pairs in display order (top to bottom)
    pub spaces: Vec<(SpaceId, String)>,
    /// Global bidder ordering. Lane positions within a space preserve this
    /// relative order.
    pub bidders: Vec<responses::UserIdentity>,
    /// All simulation rounds. Lane allocation uses the full series so lanes
    /// don't reflow as playback advances.
    pub rounds: Vec<SimRound>,
    /// Which frame to display, using AuctionChartPlayer's frame semantics.
    /// Controls the horizontal extent of rendered activity, not the column
    /// count.
    pub frame: usize,
    /// Bid increment used by the simulation. Needed to compute the price a new
    /// bid was placed at (prior round's price plus the increment).
    pub bid_increment: Decimal,
    /// Currency used to format prices in tooltips.
    pub currency: CurrencySettings,
}

// Layout constants (SVG user units).
const LEFT_PAD: f64 = 80.0;
const RIGHT_PAD: f64 = 0.0;
const TOP_PAD: f64 = 28.0;
const BOTTOM_PAD: f64 = 6.0;
const COL_W: f64 = 36.0;
const LANE_SPACING: f64 = 12.0;
const BAND_PAD: f64 = 10.0;
const DOT_RADIUS: f64 = 2.5;
const HIGH_DOT_RADIUS: f64 = 4.0;

// Bidder distinguishability is driven by luminance only (so the current palette
// is effectively greyscale). These bounds define the usable lightness range per
// color scheme; within them, bidders are assigned evenly by index.
const LIGHT_MODE_L_MIN: f64 = 25.0;
const LIGHT_MODE_L_MAX: f64 = 65.0;
const DARK_MODE_L_MIN: f64 = 45.0;
const DARK_MODE_L_MAX: f64 = 85.0;

#[function_component]
pub fn SubwayDiagram(props: &Props) -> Html {
    // Per-space lane order: bidders who ever bid on that space, filtered from
    // props.bidders to preserve global ordering.
    let lane_order: HashMap<SpaceId, Vec<UserId>> =
        compute_lane_order(&props.spaces, &props.bidders, &props.rounds);

    // Per-space band height: scales with the number of lanes. With N lanes
    // there are N-1 gaps between lane centers, plus BAND_PAD above the first
    // and below the last.
    let band_heights: Vec<f64> = props
        .spaces
        .iter()
        .map(|(sid, _)| {
            let n = lane_order.get(sid).map(|v| v.len()).unwrap_or(0);
            (n.saturating_sub(1)) as f64 * LANE_SPACING + BAND_PAD * 2.0
        })
        .collect();

    // Y offset (top edge) of each band in SVG coords.
    let band_tops: Vec<f64> = {
        let mut acc = TOP_PAD;
        let mut out = Vec::with_capacity(band_heights.len());
        for h in &band_heights {
            out.push(acc);
            acc += h;
        }
        out
    };

    let num_rounds = props.rounds.len();
    let svg_height = TOP_PAD + band_heights.iter().sum::<f64>() + BOTTOM_PAD;
    let svg_width = LEFT_PAD + (num_rounds as f64).max(1.0) * COL_W + RIGHT_PAD;

    let viewbox = format!("0 0 {} {}", svg_width, svg_height);

    // Band backgrounds (alternating neutral stripes).
    let bands = props
        .spaces
        .iter()
        .enumerate()
        .map(|(i, (_sid, name))| {
            let y = band_tops[i];
            let h = band_heights[i];
            let stripe_class = if i % 2 == 0 {
                "fill-white dark:fill-neutral-900"
            } else {
                "fill-neutral-100 dark:fill-neutral-800"
            };
            let label_y = y + h / 2.0;
            html! {
                <>
                    <rect
                        x="0"
                        y={y.to_string()}
                        width={svg_width.to_string()}
                        height={h.to_string()}
                        class={stripe_class}
                    />
                    <text
                        x={(LEFT_PAD - 8.0).to_string()}
                        y={label_y.to_string()}
                        text-anchor="end"
                        dominant-baseline="central"
                        class="fill-neutral-700 dark:fill-neutral-300 \
                            text-xs"
                    >
                        {name.clone()}
                    </text>
                </>
            }
        })
        .collect::<Html>();

    // Per (space, bidder) presence entries through `frame`, each carrying the
    // round, whether the bidder was standing (carried from prior results) or
    // placing a new bid, and the price that dot represents.
    let last_frame = props.frame.min(num_rounds.saturating_sub(1));
    let presence: HashMap<(SpaceId, UserId), Vec<PresenceEntry>> =
        compute_presence(&props.rounds, last_frame, props.bid_increment);

    // (space, bidder) -> lane_y, used to place dots and resolve segment
    // endpoints.
    let lane_y_map: HashMap<(SpaceId, UserId), f64> = props
        .spaces
        .iter()
        .enumerate()
        .flat_map(|(space_idx, (space_id, _))| {
            let band_top = band_tops[space_idx];
            lane_order
                .get(space_id)
                .map(|lanes| {
                    lanes
                        .iter()
                        .enumerate()
                        .map(move |(lane_i, uid)| {
                            ((*space_id, *uid), lane_y(band_top, lane_i))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect();

    // Bidder -> (stack order, inline style). The stroke classes read the custom
    // properties via Tailwind's `dark:` variant. Stack order equals the
    // bidder's position in `props.bidders` and breaks ties when two segments
    // have the same slope.
    let bidder_info: HashMap<UserId, (usize, String)> = props
        .bidders
        .iter()
        .enumerate()
        .map(|(i, b)| {
            (b.user_id, (i, bidder_stroke_style(i, props.bidders.len())))
        })
        .collect();

    // Bidder lookup for tooltip labels.
    let bidder_names: HashMap<UserId, String> = props
        .bidders
        .iter()
        .map(|b| (b.user_id, format_user_name_unambiguous(b)))
        .collect();

    let segments = render_segments(
        &compute_segments(&props.rounds, last_frame),
        &lane_y_map,
        &bidder_info,
    );
    let dots = render_dots(
        &presence,
        &lane_y_map,
        &bidder_info,
        &bidder_names,
        &props.currency,
    );

    // Round axis labels (top) + vertical gridlines.
    let axis = (0..num_rounds)
        .map(|r| {
            let x = round_x(r);
            let round_num = props.rounds[r].round_num;
            let grid_class = if r <= props.frame {
                "stroke-neutral-300 dark:stroke-neutral-700"
            } else {
                "stroke-neutral-200 dark:stroke-neutral-800"
            };
            let label_class = if r <= props.frame {
                "fill-neutral-600 dark:fill-neutral-400 text-xs \
                tabular-nums"
            } else {
                "fill-neutral-400 dark:fill-neutral-600 text-xs \
                tabular-nums"
            };
            html! {
                <>
                    <line
                        x1={x.to_string()}
                        y1={TOP_PAD.to_string()}
                        x2={x.to_string()}
                        y2={(svg_height - BOTTOM_PAD).to_string()}
                        class={grid_class}
                        stroke-width="1"
                    />
                    <text
                        x={x.to_string()}
                        y={(TOP_PAD - 8.0).to_string()}
                        text-anchor="middle"
                        class={label_class}
                    >
                        {round_num.to_string()}
                    </text>
                </>
            }
        })
        .collect::<Html>();

    let legend = render_legend(&props.bidders, &bidder_info);

    // Rendered-size bounds on the diagram, in rem so they scale with the user's
    // font-size preference. Min: each round stays at least
    // MIN_RENDERED_COL_W_REM wide so text and dots are legible; if the
    // container is narrower, the wrapper scrolls horizontally. Max: prevents
    // short auctions from stretching huge in a wide container; when capped,
    // mx-auto centers the SVG within its cell. pad_rem converts the
    // viewBox-unit paddings (LEFT_PAD + RIGHT_PAD) into rem, assuming the
    // default 1rem = 16px mapping.
    const MIN_RENDERED_COL_W_REM: f64 = 2.0;
    const MAX_RENDERED_COL_W_REM: f64 = 4.0;
    let pad_rem = (LEFT_PAD + RIGHT_PAD) / 16.0;
    let svg_min_rem =
        pad_rem + num_rounds.max(1) as f64 * MIN_RENDERED_COL_W_REM;
    let svg_max_rem =
        pad_rem + num_rounds.max(1) as f64 * MAX_RENDERED_COL_W_REM;
    let svg_style = format!(
        "min-width: {:.2}rem; max-width: {:.2}rem",
        svg_min_rem, svg_max_rem,
    );

    html! {
        <div class="space-y-2">
            <div class="overflow-x-auto">
                <svg
                    viewBox={viewbox}
                    class="w-full h-auto mx-auto"
                    preserveAspectRatio="xMinYMin meet"
                    style={svg_style}
                >
                    {bands}
                    {axis}
                    {segments}
                    {dots}
                </svg>
            </div>
            {legend}
        </div>
    }
}

fn round_x(round_idx: usize) -> f64 {
    LEFT_PAD + (round_idx as f64 + 0.5) * COL_W
}

/// Renders each segment as an SVG `<line>`, ordered so that steeper switches
/// render first (deeper) and horizontal continuations render last (on top).
/// Ties within a slope break by bidder index to ensure consistency across
/// frames.
fn render_segments(
    segments: &[Segment],
    lane_y_map: &HashMap<(SpaceId, UserId), f64>,
    bidder_info: &HashMap<UserId, (usize, String)>,
) -> Html {
    let mut entries: Vec<(u64, usize, Html)> = segments
        .iter()
        .filter_map(|seg| {
            let y0 = *lane_y_map.get(&(seg.space0, seg.bidder))?;
            let y1 = *lane_y_map.get(&(seg.space1, seg.bidder))?;
            let (z, style) = bidder_info.get(&seg.bidder)?.clone();
            // Multiply by 1000 so sub-unit |dy| differences survive the u64
            // cast (in practice dy is an integer multiple of LANE_SPACING, but
            // this is cheap insurance).
            let dy_key = ((y1 - y0).abs() * 1000.0) as u64;
            let node = html! {
                <line
                    x1={round_x(seg.r0).to_string()}
                    y1={y0.to_string()}
                    x2={round_x(seg.r1).to_string()}
                    y2={y1.to_string()}
                    stroke-width="6"
                    class="stroke-[var(--subway-light)] \
                        dark:stroke-[var(--subway-dark)]"
                    style={style}
                />
            };
            Some((dy_key, z, node))
        })
        .collect();
    entries.sort_by_key(|(dy, z, _)| (std::cmp::Reverse(*dy), *z));
    entries.into_iter().map(|(_, _, h)| h).collect()
}

/// Renders a horizontal legend of bidder swatches. Each entry is a small square
/// colored with that bidder's --subway-light / --subway-dark values followed by
/// the bidder's name.
fn render_legend(
    bidders: &[responses::UserIdentity],
    bidder_info: &HashMap<UserId, (usize, String)>,
) -> Html {
    let entries = bidders
        .iter()
        .filter_map(|b| {
            let (_, style) = bidder_info.get(&b.user_id)?;
            Some(html! {
                <span class="inline-flex items-center gap-1.5 \
                    text-xs text-neutral-700 \
                    dark:text-neutral-300">
                    <span
                        class="w-6 h-1 rounded-full \
                            bg-[var(--subway-light)] \
                            dark:bg-[var(--subway-dark)]"
                        style={style.clone()}
                    />
                    {render_user_name(b)}
                </span>
            })
        })
        .collect::<Html>();
    html! {
        <div class="flex flex-wrap gap-x-4 gap-y-1">
            {entries}
        </div>
    }
}

/// Renders one dot per (space, round, bidder) presence entry. New-bid dots are
/// filled white with bidder's color border; high-bid dots are white-on-black
/// (dark mode inverts) to stand out. No sort needed: positions are unique to
/// (space, bidder, round).
fn render_dots(
    presence: &HashMap<(SpaceId, UserId), Vec<PresenceEntry>>,
    lane_y_map: &HashMap<(SpaceId, UserId), f64>,
    bidder_info: &HashMap<UserId, (usize, String)>,
    bidder_names: &HashMap<UserId, String>,
    currency: &CurrencySettings,
) -> Html {
    presence
        .iter()
        .flat_map(|((space_id, uid), entries)| {
            if let Some(&cy) = lane_y_map.get(&(*space_id, *uid))
                && let Some((_, style)) = bidder_info.get(uid)
                && let Some(name) = bidder_names.get(uid).cloned()
            {
                entries
                    .iter()
                    .map(|entry| {
                        let cx = round_x(entry.round);
                        let price_str = currency.format_amount(entry.price);
                        let tooltip = if entry.is_high {
                            format!(
                                "{} — round {} — high bidder at {}",
                                name, entry.round, price_str,
                            )
                        } else {
                            format!(
                                "{} — round {} — new bid at {}",
                                name, entry.round, price_str,
                            )
                        };
                        if entry.is_high {
                            html! {
                                <circle
                                    cx={cx.to_string()}
                                    cy={cy.to_string()}
                                    r={HIGH_DOT_RADIUS.to_string()}
                                    stroke-width="1.5"
                                    class="fill-white stroke-black \
                                        dark:fill-black \
                                        dark:stroke-white"
                                >
                                    <title>{tooltip}</title>
                                </circle>
                            }
                        } else {
                            html! {
                                <circle
                                    cx={cx.to_string()}
                                    cy={cy.to_string()}
                                    r={DOT_RADIUS.to_string()}
                                    stroke-width="1"
                                    class="fill-white dark:fill-black \
                                        stroke-[var(--subway-light)] \
                                        dark:stroke-[var(--subway-dark)]"
                                    style={style.clone()}
                                >
                                    <title>{tooltip}</title>
                                </circle>
                            }
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        })
        .collect()
}

/// Per-bidder inline-style string setting --subway-light and --subway-dark CSS
/// custom properties to HSL values. The corresponding stroke classes on each
/// element read whichever applies to the active color mode via Tailwind's
/// `dark:` variant. Bidders are spread evenly across each mode's lightness
/// range; hue and saturation are both zero so the current ramp is greyscale —
/// when we later want color, only the hue/saturation constants need to change.
fn bidder_stroke_style(idx: usize, n_bidders: usize) -> String {
    let t = if n_bidders <= 1 {
        0.0
    } else {
        idx as f64 / (n_bidders - 1) as f64
    };
    let l_light = LIGHT_MODE_L_MIN + t * (LIGHT_MODE_L_MAX - LIGHT_MODE_L_MIN);
    // Dark mode ramps in the opposite direction so the first bidder stays the
    // highest-contrast shade in both modes.
    let l_dark = DARK_MODE_L_MAX - t * (DARK_MODE_L_MAX - DARK_MODE_L_MIN);
    format!(
        "--subway-light: hsl(0 0% {:.1}%); --subway-dark: hsl(0 0% {:.1}%);",
        l_light, l_dark
    )
}

/// Vertical center of a lane within its band. The first lane sits BAND_PAD
/// below the band top; each subsequent lane is LANE_SPACING below the previous.
fn lane_y(band_top: f64, lane_idx: usize) -> f64 {
    band_top + BAND_PAD + lane_idx as f64 * LANE_SPACING
}

struct Segment {
    bidder: UserId,
    r0: usize,
    space0: SpaceId,
    r1: usize,
    space1: SpaceId,
}

/// Computes the connecting line segments for each bidder's activity across
/// adjacent rounds in `0..=last`.
///
/// Rules per adjacent pair (r, r+1) and per bidder, letting A = spaces present
/// in r, B = spaces present in r+1:
/// - For each space in A ∩ B: connect within that space (continuation).
/// - For each (stopped in A\B, new in B\A) pair: connect them (activity flows
///   from stopped spaces to newly-joined ones, fully bipartite when multiples
///   on both sides).
/// - If A\B is empty but B\A is not, new spaces emerge as unconnected branches.
/// - If B\A is empty but A\B is not, stopped spaces end as unconnected
///   branches.
fn compute_segments(rounds: &[SimRound], last: usize) -> Vec<Segment> {
    // bidder -> round -> set of spaces present
    let mut presence_by_bidder_round: HashMap<
        UserId,
        HashMap<usize, HashSet<SpaceId>>,
    > = HashMap::new();
    for r in 0..=last {
        let Some(round) = rounds.get(r) else { continue };
        for (sid, bidders_this_round) in &round.bids {
            for b in bidders_this_round {
                presence_by_bidder_round
                    .entry(b.user_id)
                    .or_default()
                    .entry(r)
                    .or_default()
                    .insert(*sid);
            }
        }
        if r > 0
            && let Some(prev) = rounds.get(r - 1)
        {
            for rsr in &prev.results {
                presence_by_bidder_round
                    .entry(rsr.winner.user_id)
                    .or_default()
                    .entry(r)
                    .or_default()
                    .insert(rsr.space_id);
            }
        }
    }

    let mut out = Vec::new();
    for (bidder, by_round) in presence_by_bidder_round {
        for r in 0..last {
            let (Some(a), Some(b)) = (by_round.get(&r), by_round.get(&(r + 1)))
            else {
                continue;
            };
            // Continuations
            for sid in a.intersection(b) {
                out.push(Segment {
                    bidder,
                    r0: r,
                    space0: *sid,
                    r1: r + 1,
                    space1: *sid,
                });
            }
            // Switches: bipartite across stopped × new
            let stopped: Vec<&SpaceId> = a.difference(b).collect();
            let new_spaces: Vec<&SpaceId> = b.difference(a).collect();
            for s_from in &stopped {
                for s_to in &new_spaces {
                    out.push(Segment {
                        bidder,
                        r0: r,
                        space0: **s_from,
                        r1: r + 1,
                        space1: **s_to,
                    });
                }
            }
        }
    }
    out
}

/// A single presence entry: which round, whether the bidder was the standing
/// high bidder, and the price for that dot.
#[derive(Clone, Copy)]
struct PresenceEntry {
    round: usize,
    is_high: bool,
    price: Decimal,
}

/// For each (space, bidder), presence entries in `0..=last` where that bidder
/// was present on that space. `is_high` is true when the bidder entered the
/// round as the standing winner (carried from the prior round's results), false
/// when the bidder placed a new bid that round. `price` is what that dot
/// represents — the carried value for high bidders, and the new bid price
/// (`prior + bid_increment`, or 0 if no prior result) for new bids.
fn compute_presence(
    rounds: &[SimRound],
    last: usize,
    bid_increment: Decimal,
) -> HashMap<(SpaceId, UserId), Vec<PresenceEntry>> {
    let mut out: HashMap<(SpaceId, UserId), Vec<PresenceEntry>> =
        HashMap::new();
    for r in 0..=last {
        let Some(round) = rounds.get(r) else { continue };
        // Prior-round results indexed by space, for pricing new bids and
        // high-bid carry-forwards this round.
        let prev_price: HashMap<SpaceId, Decimal> = if r == 0 {
            HashMap::new()
        } else {
            rounds
                .get(r - 1)
                .map(|prev| {
                    prev.results
                        .iter()
                        .map(|rsr| (rsr.space_id, rsr.value))
                        .collect()
                })
                .unwrap_or_default()
        };
        for (sid, bidders_this_round) in &round.bids {
            // New bid price = prior-round price + increment, or 0 if the space
            // has no prior result.
            let price = prev_price
                .get(sid)
                .map(|p| *p + bid_increment)
                .unwrap_or(Decimal::ZERO);
            for b in bidders_this_round {
                out.entry((*sid, b.user_id))
                    .or_default()
                    .push(PresenceEntry {
                        round: r,
                        is_high: false,
                        price,
                    });
            }
        }
        if r > 0
            && let Some(prev) = rounds.get(r - 1)
        {
            // A bidder who was the high bidder entering round r does not place
            // a new bid that round on the same space, so these two presence
            // sources are disjoint per (space, bidder) — no dedup needed.
            for rsr in &prev.results {
                out.entry((rsr.space_id, rsr.winner.user_id))
                    .or_default()
                    .push(PresenceEntry {
                        round: r,
                        is_high: true,
                        price: rsr.value,
                    });
            }
        }
    }
    out
}

/// For each space, the ordered list of user_ids that ever placed a bid on that
/// space, filtered from the global `bidders` list to preserve ordering.
fn compute_lane_order(
    spaces: &[(SpaceId, String)],
    bidders: &[responses::UserIdentity],
    rounds: &[SimRound],
) -> HashMap<SpaceId, Vec<UserId>> {
    // Collect the set of user_ids that bid on each space across all rounds.
    let mut bid_sets: HashMap<SpaceId, HashSet<UserId>> = HashMap::new();
    for round in rounds {
        for (sid, bidders_this_round) in &round.bids {
            let set = bid_sets.entry(*sid).or_default();
            for b in bidders_this_round {
                set.insert(b.user_id);
            }
        }
    }

    spaces
        .iter()
        .map(|(sid, _)| {
            let set = bid_sets.remove(sid).unwrap_or_default();
            let ordered: Vec<UserId> = bidders
                .iter()
                .filter(|b| set.contains(&b.user_id))
                .map(|b| b.user_id)
                .collect();
            (*sid, ordered)
        })
        .collect()
}
