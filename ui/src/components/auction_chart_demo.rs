use std::collections::HashMap;

use wasm_bindgen::JsCast;

use payloads::auction_sim::simulate_auction;
use payloads::{
    CurrencyModeConfig, CurrencySettings, IOUConfig, PointsAllocationConfig,
    PrepaidCreditsConfig, SpaceId, UserId,
};
use rust_decimal::Decimal;
use uuid::Uuid;
use yew::prelude::*;

use crate::components::auction_sim_editor::EditorState;
use crate::components::{
    AuctionChartPlayer, AuctionSettlement, AuctionSimEditor,
};

struct Scenario {
    name: &'static str,
    description: &'static str,
    state: EditorState,
    currency: CurrencySettings,
}

fn scenarios() -> Vec<Scenario> {
    vec![rent_splitting(), desk_allocation(), street_fair()]
}

fn rent_splitting() -> Scenario {
    let alex = UserId(Uuid::from_u128(1));
    let ben = UserId(Uuid::from_u128(2));
    let chris = UserId(Uuid::from_u128(3));
    let master = SpaceId(Uuid::from_u128(100));
    let medium = SpaceId(Uuid::from_u128(101));
    let small = SpaceId(Uuid::from_u128(102));

    Scenario {
        name: "Rent splitting",
        description: "Three housemates auction bedrooms. \
            Each person's rent adjustment is their auction \
            price minus their equal share of the total proceeds.",
        state: EditorState {
            spaces: vec![
                (master, "Master".into()),
                (medium, "Medium".into()),
                (small, "Small".into()),
            ],
            bidders: vec![
                (alex, "Alex".into()),
                (ben, "Ben".into()),
                (chris, "Chris".into()),
            ],
            values: HashMap::from([
                ((alex, master), Decimal::new(400, 0)),
                ((alex, medium), Decimal::new(160, 0)),
                ((alex, small), Decimal::new(0, 0)),
                ((ben, master), Decimal::new(300, 0)),
                ((ben, medium), Decimal::new(120, 0)),
                ((ben, small), Decimal::new(0, 0)),
                ((chris, master), Decimal::new(100, 0)),
                ((chris, medium), Decimal::new(60, 0)),
                ((chris, small), Decimal::new(0, 0)),
            ]),
            bid_increment: Decimal::new(10, 0),
        },
        currency: CurrencySettings {
            mode_config: CurrencyModeConfig::DistributedClearing(IOUConfig {
                default_credit_limit: None,
                debts_callable: true,
            }),
            name: "dollars".into(),
            symbol: "$".into(),
            minor_units: 0,
            balances_visible_to_members: true,
            new_members_default_active: true,
        },
    }
}

fn desk_allocation() -> Scenario {
    let alice = UserId(Uuid::from_u128(10));
    let bob = UserId(Uuid::from_u128(11));
    let carol = UserId(Uuid::from_u128(12));
    let dave = UserId(Uuid::from_u128(13));
    let eve = UserId(Uuid::from_u128(14));
    let window = SpaceId(Uuid::from_u128(200));
    let corner = SpaceId(Uuid::from_u128(201));
    let middle = SpaceId(Uuid::from_u128(202));
    let door = SpaceId(Uuid::from_u128(203));

    Scenario {
        name: "Desk allocation",
        description: "Five team members compete for four \
            desks using quarterly credit budgets of 100 \
            each. With more people than desks, every desk \
            is worth something. Bidding over 100 means \
            saving up across quarters.",
        state: EditorState {
            spaces: vec![
                (window, "Window".into()),
                (corner, "Corner".into()),
                (middle, "Middle".into()),
                (door, "By door".into()),
            ],
            bidders: vec![
                (alice, "Alice".into()),
                (bob, "Bob".into()),
                (carol, "Carol".into()),
                (dave, "Dave".into()),
                (eve, "Eve".into()),
            ],
            values: HashMap::from([
                ((alice, window), Decimal::new(250, 0)),
                ((alice, corner), Decimal::new(150, 0)),
                ((alice, middle), Decimal::new(40, 0)),
                ((alice, door), Decimal::new(10, 0)),
                ((bob, window), Decimal::new(200, 0)),
                ((bob, corner), Decimal::new(120, 0)),
                ((bob, middle), Decimal::new(60, 0)),
                ((bob, door), Decimal::new(20, 0)),
                ((carol, window), Decimal::new(180, 0)),
                ((carol, corner), Decimal::new(180, 0)),
                ((carol, middle), Decimal::new(50, 0)),
                ((carol, door), Decimal::new(30, 0)),
                ((dave, window), Decimal::new(80, 0)),
                ((dave, corner), Decimal::new(60, 0)),
                ((dave, middle), Decimal::new(50, 0)),
                ((dave, door), Decimal::new(40, 0)),
                ((eve, window), Decimal::new(120, 0)),
                ((eve, corner), Decimal::new(100, 0)),
                ((eve, middle), Decimal::new(70, 0)),
                ((eve, door), Decimal::new(35, 0)),
            ]),
            bid_increment: Decimal::new(10, 0),
        },
        currency: CurrencySettings {
            mode_config: CurrencyModeConfig::PointsAllocation(Box::new(
                PointsAllocationConfig {
                    allowance_amount: Decimal::new(100, 0),
                    allowance_period: jiff::Span::new().days(90),
                    allowance_start: jiff::Timestamp::UNIX_EPOCH,
                },
            )),
            name: "credits".into(),
            symbol: "C".into(),
            minor_units: 0,
            balances_visible_to_members: true,
            new_members_default_active: true,
        },
    }
}

fn street_fair() -> Scenario {
    let mei = UserId(Uuid::from_u128(20));
    let joe = UserId(Uuid::from_u128(21));
    let sam = UserId(Uuid::from_u128(22));
    let entrance = SpaceId(Uuid::from_u128(300));
    let corner_booth = SpaceId(Uuid::from_u128(301));
    let interior = SpaceId(Uuid::from_u128(302));

    Scenario {
        name: "Street fair",
        description: "Vendors bid on booth locations at a \
            weekend market using prepaid credits. Corner booths have \
            extra frontage, while spots near the entrance get more foot \
            traffic. Revenue offsets event costs.",
        state: EditorState {
            spaces: vec![
                (entrance, "Entrance".into()),
                (corner_booth, "Corner".into()),
                (interior, "Interior".into()),
            ],
            bidders: vec![
                (mei, "Mei".into()),
                (joe, "Joe".into()),
                (sam, "Sam".into()),
            ],
            values: HashMap::from([
                ((mei, entrance), Decimal::new(50, 0)),
                ((mei, corner_booth), Decimal::new(35, 0)),
                ((mei, interior), Decimal::new(10, 0)),
                ((joe, entrance), Decimal::new(40, 0)),
                ((joe, corner_booth), Decimal::new(30, 0)),
                ((joe, interior), Decimal::new(20, 0)),
                ((sam, entrance), Decimal::new(25, 0)),
                ((sam, corner_booth), Decimal::new(25, 0)),
                ((sam, interior), Decimal::new(15, 0)),
            ]),
            bid_increment: Decimal::new(5, 0),
        },
        currency: CurrencySettings {
            mode_config: CurrencyModeConfig::PrepaidCredits(
                PrepaidCreditsConfig {
                    debts_callable: false,
                },
            ),
            name: "credits".into(),
            symbol: "$".into(),
            minor_units: 0,
            balances_visible_to_members: true,
            new_members_default_active: true,
        },
    }
}

/// Inner component keyed by scenario selection, so
/// switching scenarios fully resets state and hooks.
#[derive(Properties, PartialEq)]
struct InnerProps {
    initial_state: EditorState,
    currency: CurrencySettings,
}

#[function_component]
fn AuctionChartDemoInner(props: &InnerProps) -> Html {
    let state = use_state(|| props.initial_state.clone());

    let sim_input = state.to_sim_input();
    let rounds = simulate_auction(&sim_input);

    let is_distributed_clearing = matches!(
        props.currency.mode_config,
        CurrencyModeConfig::DistributedClearing(_)
    );

    html! {
        <div class="space-y-6">
            <div>
                <h4 class="text-sm font-semibold \
                    text-neutral-900 dark:text-neutral-100 \
                    mb-1">
                    {"Bidder values"}
                </h4>
                <p class="text-xs text-neutral-500 \
                    dark:text-neutral-500 mb-3">
                    {"The maximum each bidder would pay \
                    for each space."}
                </p>
            </div>
            <AuctionSimEditor state={state.clone()} />

            <div>
                <h4 class="text-sm font-semibold \
                    text-neutral-900 dark:text-neutral-100 \
                    mb-1">
                    {"Auction rounds"}
                </h4>
                <p class="text-xs text-neutral-500 \
                    dark:text-neutral-500 mb-3">
                    {"Bidders compete by placing bids each \
                    round. The auction ends when no new \
                    bids are placed."}
                </p>
            </div>
            <AuctionChartPlayer
                spaces={sim_input.spaces.clone()}
                rounds={rounds.clone()}
                currency={props.currency.clone()}
                autoplay={true}
            />

            // Settlement for DistributedClearing mode
            {if is_distributed_clearing {
                let final_results = rounds
                    .last()
                    .map(|r| r.results.clone())
                    .unwrap_or_default();
                html! {
                    <>
                    <div>
                        <h4 class="text-sm font-semibold \
                            text-neutral-900 \
                            dark:text-neutral-100 mb-1">
                            {"Settlement"}
                        </h4>
                        <p class="text-xs text-neutral-500 \
                            dark:text-neutral-500 mb-3">
                            {"Auction proceeds are split \
                            equally. Each person\u{2019}s \
                            adjustment is their price \
                            minus their share."}
                        </p>
                    </div>
                    <AuctionSettlement
                        spaces={sim_input.spaces}
                        bidders={sim_input.bidders}
                        results={final_results}
                        currency={props.currency.clone()}
                    />
                    </>
                }
            } else {
                html! {}
            }}
        </div>
    }
}

#[function_component]
pub fn AuctionChartDemo() -> Html {
    let selected = use_state(|| 0_usize);
    let all_scenarios = scenarios();

    let on_select = {
        let selected = selected.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target()
                && let Ok(select) =
                    target.dyn_into::<web_sys::HtmlSelectElement>()
                && let Ok(idx) = select.value().parse::<usize>()
            {
                selected.set(idx);
            }
        })
    };

    let scenario = &all_scenarios[*selected];

    html! {
        <div class="border border-neutral-200 dark:border-neutral-700 \
            rounded-lg p-6 bg-white dark:bg-neutral-900">
            <div class="flex flex-wrap items-center justify-between gap-3 mb-2">
                <h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100">
                    {"Auction Simulation"}
                </h3>
                <select
                    onchange={on_select}
                    class="text-sm px-2 py-1 border border-neutral-200 \
                        dark:border-neutral-700 rounded bg-white dark:bg-neutral-800 \
                        text-neutral-900 dark:text-neutral-100"
                >
                    {for all_scenarios.iter().enumerate().map(|(i, s)| {
                        html! {
                            <option
                                value={i.to_string()}
                                selected={i == *selected}
                            >
                                {s.name}
                            </option>
                        }
                    })}
                </select>
            </div>
            <p class="text-sm text-neutral-500 dark:text-neutral-500 mb-4">
                {scenario.description}
            </p>
            <AuctionChartDemoInner
                key={*selected}
                initial_state={scenario.state.clone()}
                currency={scenario.currency.clone()}
            />
        </div>
    }
}
