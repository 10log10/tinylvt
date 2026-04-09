use std::collections::HashMap;

use payloads::auction_sim::SimInput;
use payloads::responses::UserIdentity;
use payloads::{SpaceId, UserId};
use rust_decimal::Decimal;
use uuid::Uuid;
use web_sys::HtmlElement;
use yew::prelude::*;

use crate::components::InlineEdit;

#[derive(Clone, PartialEq)]
pub struct EditorState {
    pub spaces: Vec<(SpaceId, String)>,
    pub bidders: Vec<(UserId, String)>,
    pub values: HashMap<(UserId, SpaceId), Decimal>,
    pub bid_increment: Decimal,
}

impl EditorState {
    pub fn to_sim_input(&self) -> SimInput {
        SimInput {
            spaces: self.spaces.clone(),
            bidders: self
                .bidders
                .iter()
                .map(|(uid, name)| UserIdentity {
                    user_id: *uid,
                    username: name.clone(),
                    display_name: Some(name.clone()),
                })
                .collect(),
            user_values: self.values.clone(),
            bid_increment: self.bid_increment,
        }
    }
}

const ADD_BTN: &str = "\
    text-sm text-neutral-500 dark:text-neutral-500 \
    hover:text-neutral-700 dark:hover:text-neutral-300 \
    px-2 py-1";

const HEADER_BG: &str = "\
    bg-neutral-50 dark:bg-neutral-800/50 rounded";

#[derive(Properties, PartialEq)]
pub struct Props {
    pub state: UseStateHandle<EditorState>,
}

/// The editable grid for bidder names, space names, and
/// user values. Mutates the parent's state handle directly.
#[function_component]
pub fn AuctionSimEditor(props: &Props) -> Html {
    let state = &props.state;

    let grid_template = format!(
        "grid-template-columns: 8rem repeat({}, minmax(5rem, 1fr))",
        state.bidders.len()
    );

    let num_bidders = state.bidders.len();
    let num_spaces = state.spaces.len();

    // NodeRefs for each editable cell, indexed as
    // [col][row_in_col]. Col = bidder index. Row 0 = bidder
    // name in the header, row 1..=num_spaces = value cells.
    // Used to click the next cell below on Enter.
    let cell_refs = use_memo((num_bidders, num_spaces), |(nb, ns)| {
        (0..*nb)
            .map(|_| {
                (0..(*ns + 1))
                    .map(|_| NodeRef::default())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });

    // NodeRefs for the space name column
    let space_name_refs = use_memo(num_spaces, |ns| {
        (0..*ns).map(|_| NodeRef::default()).collect::<Vec<_>>()
    });

    // Pre-snapshot state for the grid rendering
    let bidders_snapshot = state.bidders.clone();
    let spaces_snapshot = state.spaces.clone();
    let values_snapshot = state.values.clone();

    // Click the next space name cell below.
    let click_next_space = {
        let space_name_refs = space_name_refs.clone();
        move |si: usize| {
            let next = si + 1;
            if next < space_name_refs.len()
                && let Some(el) = space_name_refs[next].cast::<HtmlElement>()
            {
                el.click();
            }
        }
    };

    // Click the next cell below in the same column.
    let click_next = {
        let cell_refs = cell_refs.clone();
        move |col: usize, row_in_col: usize| {
            let next_row = row_in_col + 1;
            if col < cell_refs.len()
                && next_row < cell_refs[col].len()
                && let Some(el) = cell_refs[col][next_row].cast::<HtmlElement>()
            {
                el.click();
            }
        }
    };

    // Build all grid cells as a flat list
    let mut cells: Vec<Html> = Vec::new();

    // Header row: empty corner + bidder names
    cells.push(html! {
        <div class={HEADER_BG} />
    });
    for (bi, (_, name)) in bidders_snapshot.iter().enumerate() {
        let on_change = {
            let state = state.clone();
            Callback::from(move |val: String| {
                let mut s = (*state).clone();
                s.bidders[bi].1 = val;
                state.set(s);
            })
        };
        let on_enter = {
            let click_next = click_next.clone();
            Callback::from(move |()| {
                click_next(bi, 0);
            })
        };
        let on_remove = {
            let state = state.clone();
            Callback::from(move |()| {
                let mut s = (*state).clone();
                let (uid, _) = s.bidders.remove(bi);
                s.values.retain(|&(u, _), _| u != uid);
                state.set(s);
            })
        };
        let container_ref = cell_refs[bi][0].clone();
        cells.push(html! {
            <InlineEdit
                value={name.clone()}
                on_change={on_change}
                on_enter={on_enter}
                on_remove={on_remove}
                container_ref={container_ref}
                class={classes!(HEADER_BG)}
                inner_class={classes!("font-medium", "text-right")}
            />
        });
    }

    // Space rows
    for (si, (sid, sname)) in spaces_snapshot.iter().enumerate() {
        let sid = *sid;

        // Space name cell
        let on_sname = {
            let state = state.clone();
            Callback::from(move |val: String| {
                let mut s = (*state).clone();
                s.spaces[si].1 = val;
                state.set(s);
            })
        };
        let on_enter_space = {
            let click_next_space = click_next_space.clone();
            Callback::from(move |()| {
                click_next_space(si);
            })
        };
        let on_remove_space = {
            let state = state.clone();
            Callback::from(move |()| {
                let mut s = (*state).clone();
                s.spaces.remove(si);
                s.values.retain(|&(_, s), _| s != sid);
                state.set(s);
            })
        };
        let space_ref = space_name_refs[si].clone();
        cells.push(html! {
            <InlineEdit
                value={sname.clone()}
                on_change={on_sname}
                on_enter={on_enter_space}
                on_remove={on_remove_space}
                container_ref={space_ref}
                class={classes!(HEADER_BG)}
            />
        });

        // Value cells
        for (bi, (uid, _)) in bidders_snapshot.iter().enumerate() {
            let uid = *uid;
            let row_in_col = si + 1; // row 0 = bidder name
            let val = values_snapshot
                .get(&(uid, sid))
                .map(|d| d.to_string())
                .unwrap_or_default();
            let on_val = {
                let state = state.clone();
                Callback::from(move |v: String| {
                    let mut s = (*state).clone();
                    if v.is_empty() {
                        s.values.remove(&(uid, sid));
                    } else if let Ok(d) = v.parse::<Decimal>() {
                        s.values.insert((uid, sid), d);
                    }
                    // Invalid input is silently ignored;
                    // InlineEdit reverts to the prior
                    // display value on blur.
                    state.set(s);
                })
            };
            let on_enter = {
                let click_next = click_next.clone();
                Callback::from(move |()| {
                    click_next(bi, row_in_col);
                })
            };
            let container_ref = cell_refs[bi][row_in_col].clone();
            cells.push(html! {
                <InlineEdit
                    value={val}
                    on_change={on_val}
                    on_enter={on_enter}
                    container_ref={container_ref}
                    inner_class={classes!("text-right")}
                    inputmode={AttrValue::Static("numeric")}
                />
            });
        }
    }

    let on_add_space = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            let mut s = (*state).clone();
            let n = s.spaces.len() + 1;
            s.spaces
                .push((SpaceId(Uuid::new_v4()), format!("Space {}", n)));
            state.set(s);
        })
    };

    let on_add_bidder = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            let mut s = (*state).clone();
            let n = s.bidders.len() + 1;
            s.bidders
                .push((UserId(Uuid::new_v4()), format!("Bidder {}", n)));
            state.set(s);
        })
    };
    let on_bid_increment = {
        let state = state.clone();
        Callback::from(move |val: String| {
            if let Ok(d) = val.parse::<Decimal>()
                && d > Decimal::ZERO
            {
                let mut s = (*state).clone();
                s.bid_increment = d;
                state.set(s);
            }
        })
    };

    html! {
        <div class="space-y-2">
            <div class="overflow-x-auto">
                <div
                    class="grid gap-1 items-center p-0.5"
                    style={grid_template}
                >
                    {for cells.into_iter()}
                </div>
            </div>

            <div class="flex flex-wrap items-center gap-2">
                <button
                    onclick={on_add_space}
                    class={ADD_BTN}
                >
                    {"+ Space"}
                </button>
                <button
                    onclick={on_add_bidder}
                    class={ADD_BTN}
                >
                    {"+ Bidder"}
                </button>
                <div class="flex items-center gap-2 ml-auto">
                    <label class="text-sm text-neutral-600 \
                        dark:text-neutral-400">
                        {"Bid increment"}
                    </label>
                    <InlineEdit
                        value={state.bid_increment.to_string()}
                        on_change={on_bid_increment}
                        inner_class={classes!("text-right")}
                        inputmode={AttrValue::Static("numeric")}
                        class={classes!("w-10")}
                    />
                </div>
            </div>
        </div>
    }
}
