use std::collections::HashMap;

use markdown_html::markdown_html;
use payloads::{
    CurrencyModeConfig, CurrencySettings, IOUConfig, PrepaidCreditsConfig,
    SpaceId, UserId,
};
use rust_decimal::Decimal;
use uuid::Uuid;
use yew::prelude::*;

use crate::components::auction_chart_demo::{
    Scenario, desk_allocation, rent_splitting_large,
};
use crate::components::auction_sim_editor::EditorState;
use crate::hooks::use_title;

use crate::components::AuctionScenarioPlayer;
use crate::pages::docs::MarkdownContent;

fn prepaid_config() -> CurrencySettings {
    CurrencySettings {
        mode_config: CurrencyModeConfig::PrepaidCredits(PrepaidCreditsConfig {
            debts_callable: true,
        }),
        name: "dollars".into(),
        symbol: "$".into(),
        minor_units: 2,
        balances_visible_to_members: true,
        new_members_default_active: true,
    }
}

fn distributed_config() -> CurrencySettings {
    CurrencySettings {
        mode_config: CurrencyModeConfig::DistributedClearing(IOUConfig {
            default_credit_limit: None,
            debts_callable: true,
        }),
        name: "dollars".into(),
        symbol: "$".into(),
        minor_units: 2,
        balances_visible_to_members: true,
        new_members_default_active: true,
    }
}

fn bike_auction() -> Scenario {
    let nina = UserId(Uuid::from_u128(20));
    let omar = UserId(Uuid::from_u128(21));
    let bike = SpaceId(Uuid::from_u128(300));

    Scenario {
        name: "Bike auction",
        description: "",
        state: EditorState {
            spaces: vec![(bike, "Bike".into())],
            bidders: vec![(nina, "Nina".into()), (omar, "Omar".into())],
            values: HashMap::from([
                ((nina, bike), Decimal::new(150, 0)),
                ((omar, bike), Decimal::new(100, 0)),
            ]),
            bid_increment: Decimal::new(10, 0),
        },
        currency: prepaid_config(),
        item_term: "item",
    }
}

fn bigger_room_auction() -> Scenario {
    let alex = UserId(Uuid::from_u128(20));
    let ben = UserId(Uuid::from_u128(21));
    let room = SpaceId(Uuid::from_u128(300));

    Scenario {
        name: "Room auction",
        description: "",
        state: EditorState {
            spaces: vec![(room, "Bigger room".into())],
            bidders: vec![(alex, "Alex".into()), (ben, "Ben".into())],
            values: HashMap::from([
                ((alex, room), Decimal::new(80, 0)),
                ((ben, room), Decimal::new(50, 0)),
            ]),
            bid_increment: Decimal::new(5, 0),
        },
        currency: distributed_config(),
        item_term: "room",
    }
}

fn three_room_sequence_auction() -> Scenario {
    let alex = UserId(Uuid::from_u128(20));
    let ben = UserId(Uuid::from_u128(21));
    let cam = UserId(Uuid::from_u128(22));
    let large = SpaceId(Uuid::from_u128(300));
    let medium = SpaceId(Uuid::from_u128(301));
    let small = SpaceId(Uuid::from_u128(302));

    Scenario {
        name: "Three-room auction",
        description: "",
        state: EditorState {
            spaces: vec![
                (large, "Large".into()),
                (medium, "Medium".into()),
                (small, "Small".into()),
            ],
            bidders: vec![
                (alex, "Alex".into()),
                (ben, "Ben".into()),
                (cam, "Cam".into()),
            ],
            values: HashMap::from([
                ((alex, large), Decimal::new(150, 0)),
                ((alex, medium), Decimal::new(100, 0)),
                ((alex, small), Decimal::new(0, 0)),
                ((ben, large), Decimal::new(110, 0)),
                ((ben, medium), Decimal::new(80, 0)),
                ((ben, small), Decimal::new(0, 0)),
                ((cam, large), Decimal::new(60, 0)),
                ((cam, medium), Decimal::new(30, 0)),
                ((cam, small), Decimal::new(0, 0)),
            ]),
            bid_increment: Decimal::new(10, 0),
        },
        currency: distributed_config(),
        item_term: "room",
    }
}

fn three_room_less_competition() -> Scenario {
    let mut scenario = three_room_sequence_auction();
    let large = scenario.state.spaces[0].0;
    let medium = scenario.state.spaces[1].0;
    let small = scenario.state.spaces[2].0;
    let cam = scenario.state.bidders[2].0;
    scenario
        .state
        .values
        .insert((cam, large), Decimal::new(0, 0));
    scenario
        .state
        .values
        .insert((cam, medium), Decimal::new(0, 0));
    scenario
        .state
        .values
        .insert((cam, small), Decimal::new(50, 0));
    scenario
}

#[function_component]
pub fn AuctionGuidePage() -> Html {
    use_title("Interactive Guide to Cooperative Auctions - TinyLVT");

    html! {
        <div class="max-w-4xl mx-auto px-4 py-8 space-y-10">

            <MarkdownContent html={markdown_html!(
                file: "docs/auction-guide.md",
                section: "intro"
            )} />

            <ScenarioCard scenario={bike_auction()} />

            <MarkdownContent html={markdown_html!(
                file: "docs/auction-guide.md",
                section: "after_bike_auction"
            )} />

            <ScenarioCard scenario={bigger_room_auction()} />

            <MarkdownContent html={markdown_html!(
                file: "docs/auction-guide.md",
                section: "after_single_room_auction"
            )} />

            <ScenarioCard scenario={three_room_sequence_auction()} />

            <MarkdownContent html={markdown_html!(
                file: "docs/auction-guide.md",
                section: "after_three_room_sequence_auction"
            )} />

            <ScenarioCard scenario={rent_splitting_large()} />

            <MarkdownContent html={markdown_html!(
                file: "docs/auction-guide.md",
                section: "after_large_auction"
            )} />

            <ScenarioCard scenario={three_room_less_competition()} />

            <MarkdownContent html={markdown_html!(
                file: "docs/auction-guide.md",
                section: "after_three_room_less_competition_auction"
            )} />

            <ScenarioCard scenario={desk_allocation()} />

            <MarkdownContent html={markdown_html!(
                file: "docs/auction-guide.md",
                section: "after_desk_auction"
            )} />

        </div>
    }
}

/// Card wrapper for a scenario demo, matching the
/// style used in AuctionChartDemo.
#[derive(Properties, PartialEq)]
struct ScenarioCardProps {
    scenario: Scenario,
}

#[function_component]
fn ScenarioCard(props: &ScenarioCardProps) -> Html {
    let s = &props.scenario;
    html! {
        <div class="border border-neutral-200 \
            dark:border-neutral-700 rounded-lg p-6 \
            bg-white dark:bg-neutral-900">
            <h3 class="text-lg font-semibold \
                text-neutral-900 \
                dark:text-neutral-100 mb-2">
                {s.name}
            </h3>
            <p class="text-sm text-neutral-500 \
                dark:text-neutral-500 mb-4">
                {s.description}
            </p>
            <AuctionScenarioPlayer
                initial_state={s.state.clone()}
                currency={s.currency.clone()}
                item_term={s.item_term}
            />
        </div>
    }
}
