use payloads::auction_sim::SimRound;
use payloads::{CurrencySettings, SpaceId, responses};
use rust_decimal::Decimal;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, WheelEvent};
use yew::prelude::*;

use crate::components::price_chart::{PriceChart, PriceLegend};
use crate::components::subway_diagram::{SubwayDiagram, SubwayLegend};
use crate::components::timeline_layout::{DEFAULT_COL_W, MAX_COL_W, MIN_COL_W};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub spaces: Vec<(SpaceId, String)>,
    pub bidders: Vec<responses::UserIdentity>,
    pub rounds: Vec<SimRound>,
    pub frame: usize,
    pub bid_increment: Decimal,
    pub currency: CurrencySettings,
}

/// Container that renders the price chart above the subway diagram, sharing a
/// single horizontal scroll container and a common `col_w` scale.
///
/// Owns:
/// - `col_w` state, driven by a slider.
/// - Horizontal scroll container and scroll-center-preservation across scale
///   changes.
/// - Shift+wheel → horizontal scroll.
///
/// The inner SVGs are pure rendering: they receive `col_w` as a prop and
/// compute their own viewBox geometry. Legends render outside the scroll
/// container so they stay visible regardless of horizontal scroll position.
#[function_component]
pub fn AuctionTimeline(props: &Props) -> Html {
    let col_w = use_state(|| DEFAULT_COL_W);

    // Ref on the scroll container so we can preserve the user's visual center
    // when col_w changes. onInput snapshots the horizontal center as a fraction
    // of the SVG's rendered width; the effect keyed on col_w reads the new
    // rendered width and rewrites scrollLeft to put that same fraction back at
    // the viewport center. Uses the price chart (first SVG) as the reference,
    // but both SVGs share the same viewBox width so either works.
    let scroll_container_ref = use_node_ref();
    let pending_center_fraction = use_mut_ref(|| None::<f64>);

    {
        let scroll_container_ref = scroll_container_ref.clone();
        let pending = pending_center_fraction.clone();
        use_effect_with(*col_w, move |_| {
            let fraction = pending.borrow_mut().take();
            if let Some(fraction) = fraction
                && let Some(container) =
                    scroll_container_ref.cast::<web_sys::HtmlElement>()
                && let Some(svg_el) = first_svg_child(&container)
            {
                let svg_width = svg_el.get_bounding_client_rect().width();
                let viewport_w = container.client_width() as f64;
                let target_center = fraction * svg_width;
                container.set_scroll_left(
                    (target_center - viewport_w / 2.0).max(0.0) as i32,
                );
            }
            || ()
        });
    }

    let on_scale_change = {
        let col_w = col_w.clone();
        let scroll_container_ref = scroll_container_ref.clone();
        let pending = pending_center_fraction.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e
                .target()
                .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                && let Ok(v) = input.value().parse::<f64>()
            {
                // Snapshot where the viewport is currently centered, as a
                // fraction of the SVG's rendered width. The effect keyed on
                // col_w will restore that center after the re-render.
                if let Some(container) =
                    scroll_container_ref.cast::<web_sys::HtmlElement>()
                    && let Some(svg_el) = first_svg_child(&container)
                {
                    let svg_width = svg_el.get_bounding_client_rect().width();
                    if svg_width > 0.0 {
                        let center_px = container.scroll_left() as f64
                            + container.client_width() as f64 / 2.0;
                        *pending.borrow_mut() = Some(center_px / svg_width);
                    }
                }
                col_w.set(v);
            }
        })
    };

    // Shift + wheel = horizontal scroll, for mouse users without native
    // horizontal scrolling. Trackpads already emit native deltaX, so we only
    // translate when the gesture is predominantly vertical.
    let on_wheel = {
        let scroll_container_ref = scroll_container_ref.clone();
        Callback::from(move |e: WheelEvent| {
            if e.shift_key()
                && e.delta_y().abs() > e.delta_x().abs()
                && let Some(container) =
                    scroll_container_ref.cast::<web_sys::HtmlElement>()
            {
                e.prevent_default();
                let new_left = container.scroll_left() + e.delta_y() as i32;
                container.set_scroll_left(new_left);
            }
        })
    };

    html! {
        <div class="space-y-2">
            <div
                class="overflow-x-auto"
                ref={scroll_container_ref}
                onwheel={on_wheel}
            >
                <PriceChart
                    spaces={props.spaces.clone()}
                    rounds={props.rounds.clone()}
                    frame={props.frame}
                    col_w={*col_w}
                    currency={props.currency.clone()}
                />
                <SubwayDiagram
                    spaces={props.spaces.clone()}
                    bidders={props.bidders.clone()}
                    rounds={props.rounds.clone()}
                    frame={props.frame}
                    col_w={*col_w}
                    bid_increment={props.bid_increment}
                    currency={props.currency.clone()}
                />
            </div>
            <div class="flex flex-wrap items-start justify-between \
                gap-x-4 gap-y-2">
                <div class="flex flex-col gap-y-1">
                    <PriceLegend spaces={props.spaces.clone()} />
                    <SubwayLegend bidders={props.bidders.clone()} />
                </div>
                <input
                    type="range"
                    min={MIN_COL_W.to_string()}
                    max={MAX_COL_W.to_string()}
                    step="1"
                    value={col_w.to_string()}
                    oninput={on_scale_change}
                    class="h-1.5 w-32 py-2 accent-neutral-500 \
                        cursor-pointer"
                    title="Horizontal scale"
                />
            </div>
        </div>
    }
}

/// Finds the first direct-child SVG element of `container`. Used by the
/// scroll-center-preservation logic to measure the charts' rendered width.
/// Walks siblings rather than just taking `first_element_child` so future
/// additions (a header, overlay, etc.) above the SVGs wouldn't break the
/// lookup.
fn first_svg_child(
    container: &web_sys::HtmlElement,
) -> Option<web_sys::Element> {
    let mut el = container.first_element_child();
    while let Some(e) = el {
        if e.tag_name().eq_ignore_ascii_case("svg") {
            return Some(e);
        }
        el = e.next_element_sibling();
    }
    None
}
