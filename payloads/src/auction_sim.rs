use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AuctionRoundId, RoundSpaceResult, SpaceId, UserId, responses};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimInput {
    /// (space_id, name) — name is used for deterministic ordering
    pub spaces: Vec<(SpaceId, String)>,
    pub bidders: Vec<responses::UserIdentity>,
    pub user_values: HashMap<(UserId, SpaceId), Decimal>,
    pub bid_increment: Decimal,
}

/// Runs a deterministic auction simulation.
///
/// Simplifications vs the full system:
/// - Each bidder wants exactly one space (max_items = 1)
/// - No eligibility constraints
/// - No credit limits
/// - Values are fixed throughout the auction
///
/// Per-round output from the simulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimRound {
    pub round_num: i32,
    pub results: Vec<RoundSpaceResult>,
    /// Which users bid on which spaces this round.
    pub bids: HashMap<SpaceId, Vec<responses::UserIdentity>>,
}

pub fn simulate_auction(input: &SimInput) -> Vec<SimRound> {
    // Sort spaces by name for deterministic iteration
    let mut spaces = input.spaces.clone();
    spaces.sort_by(|a, b| a.1.cmp(&b.1));

    // Sort bidders by username for deterministic processing
    let mut bidders = input.bidders.clone();
    bidders.sort_by(|a, b| a.username.cmp(&b.username));

    // Bidder lookup by user_id
    let bidder_map: HashMap<UserId, &responses::UserIdentity> =
        bidders.iter().map(|b| (b.user_id, b)).collect();

    let mut rounds: Vec<SimRound> = Vec::new();

    loop {
        // Derive previous results from the last round
        let prev_results: HashMap<SpaceId, (UserId, Decimal)> = rounds
            .last()
            .map(|r| {
                r.results
                    .iter()
                    .map(|r| (r.space_id, (r.winner.user_id, r.value)))
                    .collect()
            })
            .unwrap_or_default();

        // Price a bidder would pay for a space this round
        let bid_price = |space_id: &SpaceId| -> Decimal {
            prev_results
                .get(space_id)
                .map(|&(_, p)| p + input.bid_increment)
                .unwrap_or(Decimal::ZERO)
        };

        // Collect bids: space_id -> list of bidders
        let mut bids: HashMap<SpaceId, Vec<responses::UserIdentity>> =
            HashMap::new();

        for bidder in &bidders {
            // Skip if already winning a space (max_items = 1)
            let already_winning =
                prev_results.values().any(|&(uid, _)| uid == bidder.user_id);
            if already_winning {
                continue;
            }

            // Calculate surplus for each space (in name order)
            // Tuples: (space_id, surplus, value)
            let mut space_surpluses: Vec<(SpaceId, Decimal, Decimal)> =
                Vec::new();
            for (space_id, _name) in &spaces {
                if let Some(&user_value) =
                    input.user_values.get(&(bidder.user_id, *space_id))
                {
                    let surplus = user_value - bid_price(space_id);
                    if surplus >= Decimal::ZERO {
                        space_surpluses.push((*space_id, surplus, user_value));
                    }
                }
            }

            // Sort by surplus descending, then value descending
            // to break ties
            space_surpluses
                .sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));

            // Bid on the top space
            if let Some((space_id, _, _)) = space_surpluses.first() {
                bids.entry(*space_id).or_default().push(bidder.clone());
            }
        }

        let any_bids = !bids.is_empty();

        // Resolve round results
        let round_num = rounds.len() as i32;
        let round_id = AuctionRoundId(Uuid::new_v4());
        let mut round_results: Vec<RoundSpaceResult> = Vec::new();
        for (space_id, _name) in &spaces {
            if let Some(space_bids) = bids.get(space_id) {
                // Winner: alphabetically first by username
                let winner = space_bids
                    .iter()
                    .min_by(|a, b| a.username.cmp(&b.username))
                    .unwrap();
                round_results.push(RoundSpaceResult {
                    space_id: *space_id,
                    round_id,
                    winner: winner.clone(),
                    value: bid_price(space_id),
                });
            } else if let Some(&(winner_id, price)) = prev_results.get(space_id)
            {
                // Carry forward previous result
                round_results.push(RoundSpaceResult {
                    space_id: *space_id,
                    round_id,
                    winner: bidder_map[&winner_id].clone(),
                    value: price,
                });
            }
            // else: no bids and no previous result — skip
        }

        rounds.push(SimRound {
            round_num,
            results: round_results,
            bids,
        });

        // Auction concludes when no new bids were placed
        if !any_bids {
            break;
        }
    }

    rounds
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uid(n: u128) -> UserId {
        UserId(Uuid::from_u128(n))
    }

    fn sid(n: u128) -> SpaceId {
        SpaceId(Uuid::from_u128(n))
    }

    fn identity(id: UserId, name: &str) -> responses::UserIdentity {
        responses::UserIdentity {
            user_id: id,
            username: name.to_string(),
            display_name: None,
        }
    }

    #[test]
    fn test_two_bidders_one_space() {
        // Alice values space at 3, Bob values at 5.
        // Bid increment = 1. Both bid at price 0 in round 0.
        // Alice wins round 0 (alphabetical). Bob bids round 1
        // at price 1, wins. Alice bids round 2 at price 2, wins.
        // ... continues until Alice drops out at price 4
        // (surplus = 3 - 4 = -1).
        // Bob wins at price 3.
        let alice = uid(1);
        let bob = uid(2);
        let space = sid(100);

        let input = SimInput {
            spaces: vec![(space, "space".into())],
            bidders: vec![identity(alice, "alice"), identity(bob, "bob")],
            user_values: HashMap::from([
                ((alice, space), Decimal::new(3, 0)),
                ((bob, space), Decimal::new(5, 0)),
            ]),
            bid_increment: Decimal::new(1, 0),
        };

        let result = simulate_auction(&input);

        // Round 0: both bid, alice wins (alphabetical), price 0
        assert_eq!(result[0].round_num, 0);
        assert_eq!(result[0].results.len(), 1);
        assert_eq!(result[0].results[0].winner.username, "alice");
        assert_eq!(result[0].results[0].value, Decimal::ZERO);
        assert_eq!(result[0].bids[&space].len(), 2);

        // Round 1: bob bids (alice is standing), bob wins,
        // price 1
        assert_eq!(result[1].results[0].winner.username, "bob");
        assert_eq!(result[1].results[0].value, Decimal::new(1, 0));

        // Round 2: alice bids (surplus=3-2=1), alice wins,
        // price 2
        assert_eq!(result[2].results[0].winner.username, "alice");
        assert_eq!(result[2].results[0].value, Decimal::new(2, 0));

        // Round 3: bob bids (surplus=5-3=2), bob wins, price 3
        assert_eq!(result[3].results[0].winner.username, "bob");
        assert_eq!(result[3].results[0].value, Decimal::new(3, 0));

        // Round 4: alice surplus = 3-4 = -1, no bids,
        // carried-forward results, auction concludes
        assert_eq!(result.len(), 5);
        assert_eq!(result[4].results[0].winner.username, "bob");
        assert_eq!(result[4].results[0].value, Decimal::new(3, 0));
        assert!(result[4].bids.is_empty());
    }

    #[test]
    fn test_no_competition() {
        // Alice values A, Bob values B. No overlap.
        // Round 0: both bid on their space at price 0.
        // Round 1: both are standing high bidders, no bids,
        // auction concludes.
        let alice = uid(1);
        let bob = uid(2);
        let space_a = sid(100);
        let space_b = sid(101);

        let input = SimInput {
            spaces: vec![(space_a, "a".into()), (space_b, "b".into())],
            bidders: vec![identity(alice, "alice"), identity(bob, "bob")],
            user_values: HashMap::from([
                ((alice, space_a), Decimal::new(5, 0)),
                ((bob, space_b), Decimal::new(5, 0)),
            ]),
            bid_increment: Decimal::new(1, 0),
        };

        let result = simulate_auction(&input);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].results.len(), 2);

        let a_result = result[0]
            .results
            .iter()
            .find(|r| r.space_id == space_a)
            .unwrap();
        let b_result = result[0]
            .results
            .iter()
            .find(|r| r.space_id == space_b)
            .unwrap();
        assert_eq!(a_result.winner.username, "alice");
        assert_eq!(a_result.value, Decimal::ZERO);
        assert_eq!(b_result.winner.username, "bob");
        assert_eq!(b_result.value, Decimal::ZERO);

        // Round 1: carried forward, auction concludes
        assert_eq!(result[1].results.len(), 2);
        assert!(result[1].bids.is_empty());
    }

    #[test]
    fn test_no_bidders() {
        let input = SimInput {
            spaces: vec![(sid(1), "space".into())],
            bidders: vec![],
            user_values: HashMap::new(),
            bid_increment: Decimal::new(1, 0),
        };

        let result = simulate_auction(&input);
        assert_eq!(result.len(), 1);
        assert!(result[0].results.is_empty());
    }

    #[test]
    fn test_single_bidder() {
        // One bidder, one space. Wins at price 0, then no more
        // bids (standing high bidder), auction concludes.
        let alice = uid(1);
        let space = sid(100);

        let input = SimInput {
            spaces: vec![(space, "space".into())],
            bidders: vec![identity(alice, "alice")],
            user_values: HashMap::from([((alice, space), Decimal::new(10, 0))]),
            bid_increment: Decimal::new(1, 0),
        };

        let result = simulate_auction(&input);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].results[0].winner.username, "alice");
        assert_eq!(result[0].results[0].value, Decimal::ZERO);
        // Round 1: carried forward, auction concludes
        assert_eq!(result[1].results[0].winner.username, "alice");
        assert_eq!(result[1].results[0].value, Decimal::ZERO);
    }

    #[test]
    fn test_surplus_ordering() {
        // Alice values space A at 10, space B at 2.
        // With max_items=1, she should bid on A (higher surplus).
        let alice = uid(1);
        let space_a = sid(100);
        let space_b = sid(101);

        let input = SimInput {
            spaces: vec![(space_a, "a".into()), (space_b, "b".into())],
            bidders: vec![identity(alice, "alice")],
            user_values: HashMap::from([
                ((alice, space_a), Decimal::new(10, 0)),
                ((alice, space_b), Decimal::new(2, 0)),
            ]),
            bid_increment: Decimal::new(1, 0),
        };

        let result = simulate_auction(&input);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].results.len(), 1);
        assert_eq!(result[0].results[0].space_id, space_a);
        assert_eq!(result[0].results[0].winner.username, "alice");
        // Round 1: carried forward, auction concludes
        assert_eq!(result[1].results[0].space_id, space_a);
    }
}
