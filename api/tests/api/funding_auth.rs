//! Card authorization machinery tests (phase 5): bid-driven minting and
//! sizing under both hold strategies, swap-reauth raises with the
//! superseded-cancel worker, decline handling and the automatic-raise
//! pause, the pre-authorize endpoint, prerequisite errors, and the
//! Connect webhook intent handlers (adoption, out-of-band cancel,
//! payment_failed).

use api::store;
use payloads::{
    AccountOwner, ApiError, AuctionId, CommunityId, UserId, requests,
    responses::{CardAvailability, FundingDeclinePause},
};
use rust_decimal::{Decimal, dec};
use serde_json::{Value, json};
use test_helpers::{TestApp, assert_api_error, spawn_app};

use crate::funding::{create_open_auction, credit_member};

pub async fn bob_user_id(
    app: &TestApp,
    community_id: &CommunityId,
) -> anyhow::Result<UserId> {
    let members = app.client.get_members(community_id).await?;
    Ok(members
        .iter()
        .find(|m| m.user.username == "bob")
        .unwrap()
        .user
        .user_id)
}

/// Backed community with a charges-enabled mock connected account,
/// bob's saved card (`pm_bob`) and charge grant. Leaves bob logged in.
pub async fn card_enabled_setup(app: &TestApp) -> anyhow::Result<CommunityId> {
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    sqlx::query(
        "UPDATE communities SET stripe_account_id = 'acct_mock_1', \
         stripe_charges_enabled = TRUE WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    app.login_bob().await?;
    save_bob_card(app).await?;
    app.client
        .update_card_charge_grant(&requests::UpdateCardChargeGrant {
            community_id,
            grant: requests::ChargeGrant::Granted,
        })
        .await?;
    Ok(community_id)
}

/// Save a card for the logged-in user (bob in these tests): start a
/// setup session (creating the mock platform customer), surface the
/// mock card, and adopt it via the profile poll path.
async fn save_bob_card(app: &TestApp) -> anyhow::Result<()> {
    app.client.create_card_setup_session().await?;
    app.stripe_service
        .mock_customer_payment_methods
        .lock()
        .unwrap()
        .insert(
            "cus_user_mock_1".to_string(),
            vec![api::stripe_service::CardPaymentMethod {
                id: "pm_bob".to_string(),
                brand: "visa".to_string(),
                last4: "4242".to_string(),
                exp_month: 12,
                exp_year: 2030,
            }],
        );
    app.client.get_payment_profile().await?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
pub struct IntentRow {
    pub payment_intent_id: Option<String>,
    pub is_active: bool,
    pub status: String,
    pub authorized_amount: Option<Decimal>,
    pub last_decline_code: Option<String>,
}

pub async fn intent_rows(
    app: &TestApp,
    auction_id: &AuctionId,
) -> anyhow::Result<Vec<IntentRow>> {
    Ok(sqlx::query_as(
        "SELECT payment_intent_id, is_active, status::TEXT, \
                authorized_amount, last_decline_code \
         FROM funding_intents \
         WHERE auction_id = $1 ORDER BY created_at",
    )
    .bind(auction_id)
    .fetch_all(&app.db_pool)
    .await?)
}

/// A bid exceeding balance mints a manual-capture authorization sized to
/// the shortfall (budget strategy with no stated values has no budget to
/// size beyond need), backing the bid in the same operation.
#[tokio::test]
async fn bid_beyond_balance_mints_authorization() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.commitment, dec!(10));
    assert_eq!(funding.balance_backing, dec!(4));
    assert_eq!(funding.authorized, dec!(6));
    assert!(funding.capture_before.is_some());
    assert_eq!(funding.card_available, CardAvailability::Available);

    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "authorized");
    assert!(rows[0].is_active);
    assert_eq!(rows[0].authorized_amount, Some(dec!(6)));
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
    let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
    let pi = intents.get(&pi_id).unwrap();
    assert_eq!(pi.status, "requires_capture");
    assert_eq!(pi.amount_minor, 600);
    assert_eq!(pi.account_id, "acct_mock_1");
    assert_eq!(pi.metadata.get("user_id"), Some(&bob.to_string()));

    Ok(())
}

/// A cap-rejected bid on the card path creates no funding intent or
/// authorization: the attempt-first shape runs every bid-time validation
/// before any Stripe machinery.
#[tokio::test]
async fn cap_rejected_bid_mints_no_authorization() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;

    // Capped auction with no cap rows: bob's bid fails on the cap, not
    // on funding, despite his empty balance.
    app.login_alice().await?;
    let site_id = app
        .client
        .create_site(&test_helpers::site_details_b(community_id))
        .await?;
    let mut space_details = test_helpers::space_details_a(site_id);
    space_details.reserve_price = payloads::ReservePrice(dec!(10));
    let space_id = app.client.create_space(&space_details).await?;
    let mut auction_details =
        test_helpers::auction_details_a(site_id, &app.time_source);
    auction_details.start_at = Some(app.time_source.now());
    auction_details.capped = true;
    let auction_id = app.client.create_auction(&auction_details).await?;
    app.tick().await;

    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let result = app.client.create_bid(&space_id, &rounds[0].round_id).await;
    assert_api_error(
        result,
        ApiError::ExceedsBidderCap {
            available: 0.0,
            required: 10.0,
            category: None,
        },
    );
    assert!(intent_rows(&app, &auction_id).await?.is_empty());

    Ok(())
}

/// Free balance stays transferable while commitments are card-backed: the
/// generic credit check gates backed-mode debits on balance − Σ balance
/// commitments (`max(0, commitment − live auth)` per auction), not balance −
/// commitment, so a member whose commitment is mostly card-authorized can still
/// move the balance their bids don't require.
#[tokio::test]
async fn transfer_spends_free_balance_with_card_backed_commitments()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    // Bob's bid: 10 committed, 6 card-authorized, so 4 of balance required.
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    // A later top-up leaves 20 of balance no bid commitment requires.
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(20)).await?;

    app.login_bob().await?;
    let info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(info.balance, dec!(24));
    assert_eq!(info.commitment, dec!(10));
    assert_eq!(info.available_credit, Some(dec!(20)));

    // The free 20 transfers despite the commitment (10) exceeding what the
    // balance backs (4).
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to: AccountOwner::Treasury,
            amount: dec!(20),
            note: None,
            idempotency_key: requests::ClientIdempotencyKey::new(),
        })
        .await?;

    // The remaining 4 backs the live bid and is not transferable.
    let result = app
        .client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to: AccountOwner::Treasury,
            amount: dec!(1),
            note: None,
            idempotency_key: requests::ClientIdempotencyKey::new(),
        })
        .await;
    assert_api_error(result, ApiError::InsufficientBalance);

    // The bid's backing is untouched by the transfer.
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.balance_backing, dec!(4));
    assert_eq!(funding.authorized, dec!(6));

    Ok(())
}

/// Budget strategy: the hold covers the member's auction budget (their
/// values) net of balance backing, not just the triggering bid.
#[tokio::test]
async fn budget_hold_sized_to_values() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(5)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(40),
        })
        .await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    // need = 10 − 5 = 5, budget net of balance = 40 − 5 = 35 → 35.
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(35));
    Ok(())
}

/// Minimum-start strategy opens at the denomination minimum, and a
/// member-present authorize resizes the hold exactly to the requested
/// amount via a swap-reauth: a new intent activates, the old is
/// superseded, and the worker cancels it at Stripe. (Bid-driven raises
/// keep catch-up-or-double; see the proxy swap-raise test.)
#[tokio::test]
async fn min_start_and_swap_raise() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.client
        .update_hold_strategy(&requests::UpdateHoldStrategy {
            budget_holds: false,
        })
        .await?;
    app.login_alice().await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(1)).await?;

    // Zero balance: the whole reserve rides the card, opening at
    // max(stripe_min, need) = 1.
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(1));

    // A member-present authorize for 1.50 resizes exactly to the
    // request. The old intent is superseded, then canceled by the
    // worker on the next tick.
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(1.50)),
        })
        .await?;
    // (Row creation order is ambiguous under frozen mock time, so find
    // rows by status rather than position.)
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 2);
    let old = rows.iter().find(|r| r.status == "superseded").unwrap();
    assert!(!old.is_active);
    assert_eq!(old.authorized_amount, Some(dec!(1)));
    let new = rows.iter().find(|r| r.status == "authorized").unwrap();
    assert!(new.is_active);
    assert_eq!(new.authorized_amount, Some(dec!(1.50)));

    app.tick().await;
    let rows = intent_rows(&app, &auction_id).await?;
    let old = rows.iter().find(|r| r.status == "canceled").unwrap();
    assert_eq!(old.authorized_amount, Some(dec!(1)));
    let old_pi = old.payment_intent_id.clone().unwrap();
    assert_eq!(
        app.stripe_service
            .mock_payment_intents
            .lock()
            .unwrap()
            .get(&old_pi)
            .unwrap()
            .status,
        "canceled"
    );
    // Bob's bid stays backed by the replacement.
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(1.50));
    assert_eq!(funding.commitment, dec!(1));

    Ok(())
}

/// Missing prerequisites surface contextual errors: no saved card, no
/// grant. A community without a charges-enabled account has no card path
/// at all, so the bid is honestly short on balance.
#[tokio::test]
async fn card_prerequisite_errors() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    app.login_alice().await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;

    // Not connected: balance-only community.
    app.login_bob().await?;
    let result = app.client.create_bid(&space_id, &rounds[0].round_id).await;
    assert_api_error(result, ApiError::InsufficientBalance);

    sqlx::query(
        "UPDATE communities SET stripe_account_id = 'acct_mock_1', \
         stripe_charges_enabled = TRUE WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    let result = app.client.create_bid(&space_id, &rounds[0].round_id).await;
    assert_api_error(result, ApiError::SavedCardRequired);

    save_bob_card(&app).await?;
    let result = app.client.create_bid(&space_id, &rounds[0].round_id).await;
    assert_api_error(result, ApiError::CardChargeGrantRequired);

    app.client
        .update_card_charge_grant(&requests::UpdateCardChargeGrant {
            community_id,
            grant: requests::ChargeGrant::Granted,
        })
        .await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    Ok(())
}

/// A declined confirm cancels the pending row with decline metadata, the
/// bid is rejected with the decline code, and no hold or bid exists.
#[tokio::test]
async fn declined_confirm_records_metadata() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.stripe_service
        .mock_declining_payment_methods
        .lock()
        .unwrap()
        .insert("pm_bob".to_string(), "insufficient_funds".to_string());
    app.login_alice().await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    let result = app.client.create_bid(&space_id, &rounds[0].round_id).await;
    assert_api_error(
        result,
        ApiError::CardDeclined {
            code: Some("insufficient_funds".to_string()),
        },
    );

    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.commitment, Decimal::ZERO);
    assert_eq!(funding.authorized, Decimal::ZERO);
    // The decline pause is surfaced so the page can explain it and
    // offer the checkout recovery path.
    assert_eq!(
        funding.decline_pause,
        Some(FundingDeclinePause {
            code: Some("insufficient_funds".to_string()),
        })
    );
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "canceled");
    assert_eq!(
        rows[0].last_decline_code.as_deref(),
        Some("insufficient_funds")
    );

    Ok(())
}

/// The proxy mints authorizations reactively for its funding-short
/// bids, and a decline pauses further automatic attempts (bids that
/// fit existing backing still land) until member action.
#[tokio::test]
async fn proxy_mints_and_decline_pauses() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(3)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(25),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.tick().await;

    // Budget member: hold sized to budget (25) net of balance (3).
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.commitment, dec!(10));
    assert_eq!(funding.authorized, dec!(22));

    // Auction ends; a fresh auction with a declining card: the proxy
    // records the decline once and pauses — repeated ticks mint no
    // further intents, and no bid lands beyond balance.
    crate::funding::run_until_ended(&app, &[auction_id]).await?;
    app.stripe_service
        .mock_declining_payment_methods
        .lock()
        .unwrap()
        .insert("pm_bob".to_string(), "do_not_honor".to_string());
    app.login_alice().await?;
    let (space_b, auction_b) =
        create_open_auction(&app, community_id, "site b", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: space_b,
            value: dec!(25),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id: auction_b,
            max_items: 1,
        })
        .await?;
    app.tick().await;
    app.tick().await;

    let rows = intent_rows(&app, &auction_b).await?;
    assert_eq!(rows.len(), 1, "decline pause must stop repeat attempts");
    assert_eq!(rows[0].status, "canceled");
    assert_eq!(rows[0].last_decline_code.as_deref(), Some("do_not_honor"));
    let funding = app.client.get_auction_funding(&auction_b).await?;
    assert_eq!(funding.commitment, Decimal::ZERO);

    Ok(())
}

/// The proxy raises an existing authorization automatically when bids
/// climb across rounds: the swap replacement activates sized
/// catch-up-or-double, the predecessor is superseded then canceled by
/// the worker, and the standing bid stays backed throughout.
#[tokio::test]
async fn proxy_swap_raise_across_rounds() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.client
        .update_hold_strategy(&requests::UpdateHoldStrategy {
            budget_holds: false,
        })
        .await?;

    app.login_alice().await?;
    let members = app.client.get_members(&community_id).await?;
    let alice = members
        .iter()
        .find(|m| m.user.username == "alice")
        .unwrap()
        .user
        .user_id;
    credit_member(&app, community_id, alice, dec!(20)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(1)).await?;

    // Round 0: bob's proxy opens at the reserve, minimum-start hold of 1
    // (zero balance, so the whole bid rides the card).
    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(25),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.tick().await;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.commitment, dec!(1));
    assert_eq!(funding.authorized, dec!(1));

    // Round 1: alice (balance-backed) outbids bob at 2.
    crate::funding::finish_round(&app, &auction_id).await?;
    app.login_alice().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds.last().unwrap().round_id)
        .await?;

    // Round 2: processing the round also runs bob's proxy, which needs 3
    // against a 1 authorization — an automatic swap raise to
    // max(2 × 1, 3) = 3 backs the re-bid. The same tick's intent worker
    // then cancels the superseded predecessor, so by the time we look
    // the swap is fully settled.
    crate::funding::finish_round(&app, &auction_id).await?;
    app.login_bob().await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.commitment, dec!(3));
    assert_eq!(funding.authorized, dec!(3));
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 2);
    let old = rows.iter().find(|r| r.status == "canceled").unwrap();
    assert_eq!(old.authorized_amount, Some(dec!(1)));
    assert!(!old.is_active);
    let new = rows.iter().find(|r| r.status == "authorized").unwrap();
    assert!(new.is_active);
    assert_eq!(new.authorized_amount, Some(dec!(3)));
    let old_pi = old.payment_intent_id.clone().unwrap();
    assert_eq!(
        app.stripe_service
            .mock_payment_intents
            .lock()
            .unwrap()
            .get(&old_pi)
            .unwrap()
            .status,
        "canceled"
    );

    // The invariant suite as oracle: the swap-raise end state is clean.
    let report = app.reconcile().await;
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);
    Ok(())
}

/// Re-running the proxy pass with an unchanged bid set must not raise
/// the hold: the claim replaces the member's current-round bids with
/// the plan's successors, so the sizing must not count both.
#[tokio::test]
async fn proxy_rerun_does_not_double_hold() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.client
        .update_hold_strategy(&requests::UpdateHoldStrategy {
            budget_holds: false,
        })
        .await?;
    app.login_alice().await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(25),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.tick().await;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.commitment, dec!(10));
    assert_eq!(funding.authorized, dec!(10));

    // Re-save the same settings: the dirty flag re-runs the pass with
    // no change to the planned bids. The hold must not move.
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.tick().await;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.commitment, dec!(10));
    assert_eq!(
        funding.authorized,
        dec!(10),
        "re-run with unchanged bids must not raise the hold"
    );
    assert_eq!(intent_rows(&app, &auction_id).await?.len(), 1);
    Ok(())
}

/// A space the member is already winning must not absorb the freed bid
/// slot or the hold sizing: the reactive order sizes the
/// actually-placeable next space's bid, so it lands backed rather than
/// stalling behind a wrongly-sized hold.
#[tokio::test]
async fn proxy_sizes_past_already_winning_space() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.client
        .update_hold_strategy(&requests::UpdateHoldStrategy {
            budget_holds: false,
        })
        .await?;

    // One auction with a cheap space (reserve 5) and a pricey one
    // (reserve 30).
    app.login_alice().await?;
    let mut site_details = test_helpers::site_details_b(community_id);
    site_details.name = "site a".to_string();
    let site_id = app.client.create_site(&site_details).await?;
    let mut cheap_details = test_helpers::space_details_a(site_id);
    cheap_details.reserve_price = payloads::ReservePrice(dec!(5));
    let cheap = app.client.create_space(&cheap_details).await?;
    let mut pricey_details = test_helpers::space_details_b(site_id);
    pricey_details.reserve_price = payloads::ReservePrice(dec!(30));
    let pricey = app.client.create_space(&pricey_details).await?;
    let mut auction_details =
        test_helpers::auction_details_a(site_id, &app.time_source);
    auction_details.start_at = Some(app.time_source.now());
    auction_details
        .auction_params
        .activity_rule_params
        .eligibility_progression = vec![];
    let auction_id = app.client.create_auction(&auction_details).await?;
    app.tick().await;

    // Round 0: max_items 1, bob's proxy takes the higher-surplus cheap
    // space at its reserve with a hold of 5, and wins it unopposed.
    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: cheap,
            value: dec!(50),
        })
        .await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: pricey,
            value: dec!(40),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.tick().await;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.commitment, dec!(5));
    assert_eq!(funding.authorized, dec!(5));
    crate::funding::finish_round(&app, &auction_id).await?;

    // Round 1: raising max_items to 2 frees one slot, which must go to
    // the pricey space — the won space can't be bid on again, so the
    // sizing must cover the pricey bid (30) plus the standing win (5).
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 2,
        })
        .await?;
    app.tick().await;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(
        funding.commitment,
        dec!(35),
        "the pricey space's bid must land, backed by a right-sized hold"
    );
    assert_eq!(funding.authorized, dec!(35));

    let report = app.reconcile().await;
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);
    Ok(())
}

/// An eligibility-blocked space ahead of a placeable one must not
/// derail the sizing: the reactive order sizes the bid that actually
/// failed on funding — which already passed the eligibility gate — so
/// the placeable bid lands backed. (With plan-prefix sizing this
/// stalled: the hold sized to the blocked space's cheap bid, the real
/// bid failed on funds, and the retry saw no gap.)
#[tokio::test]
async fn proxy_orders_past_eligibility_blocked_space() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.client
        .update_hold_strategy(&requests::UpdateHoldStrategy {
            budget_holds: false,
        })
        .await?;

    // One auction with a cheap high-point space (reserve 1, 10 points)
    // and a pricey low-point one (reserve 30, 2 points), under an
    // activity rule (threshold 1.0) so round-0 bidding sets each
    // member's eligibility budget.
    app.login_alice().await?;
    let members = app.client.get_members(&community_id).await?;
    let alice = members
        .iter()
        .find(|m| m.user.username == "alice")
        .unwrap()
        .user
        .user_id;
    credit_member(&app, community_id, alice, dec!(40)).await?;
    let mut site_details = test_helpers::site_details_b(community_id);
    site_details.name = "site a".to_string();
    let site_id = app.client.create_site(&site_details).await?;
    let mut cheap_details = test_helpers::space_details_a(site_id);
    cheap_details.reserve_price = payloads::ReservePrice(dec!(1));
    cheap_details.eligibility_points = 10.0;
    let cheap = app.client.create_space(&cheap_details).await?;
    let mut pricey_details = test_helpers::space_details_b(site_id);
    pricey_details.reserve_price = payloads::ReservePrice(dec!(30));
    pricey_details.eligibility_points = 2.0;
    let pricey = app.client.create_space(&pricey_details).await?;
    let mut auction_details =
        test_helpers::auction_details_a(site_id, &app.time_source);
    auction_details.start_at = Some(app.time_source.now());
    auction_details
        .auction_params
        .activity_rule_params
        .eligibility_progression = vec![(0, 1.0)];
    let auction_id = app.client.create_auction(&auction_details).await?;
    app.tick().await;

    // Round 0 (no eligibility gate yet): alice bids the pricey space
    // from balance. Bob's proxy values order it first too (surplus 10
    // vs the cheap space's 9.5), so max_items 1 stops after the pricey
    // bid — a reactive hold of 30 — and bob's round-0 activity is 2
    // points, his budget from round 1 on.
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client.create_bid(&pricey, &rounds[0].round_id).await?;
    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: cheap,
            value: dec!(10.5),
        })
        .await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id: pricey,
            value: dec!(40),
        })
        .await?;
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 1,
        })
        .await?;
    app.tick().await;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.commitment, dec!(30));
    assert_eq!(funding.authorized, dec!(30));

    // Round 1: alice won the tie, so bob's proxy re-bids. Rising prices
    // reorder the walk — the cheap space's surplus (9.5) now beats the
    // pricey one's (40 − 31 = 9) — but its 10 points exceed bob's
    // budget of 2, so it must be skipped, and the pricey re-bid (31,
    // exactly 2 points) must land backed by a raise to
    // max(2 × 30, 31) = 60.
    crate::funding::finish_round(&app, &auction_id).await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(
        funding.commitment,
        dec!(31),
        "the eligible pricey re-bid must land, not stall behind the \
         blocked space"
    );
    assert_eq!(funding.authorized, dec!(60));
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .find(|r| r.status == "authorized")
            .unwrap()
            .authorized_amount,
        Some(dec!(60))
    );

    let report = app.reconcile().await;
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);
    Ok(())
}

/// The pre-authorize endpoint sizes per strategy without a bid, and
/// no-ops when the live authorization already covers the target.
#[tokio::test]
async fn authorize_funding_strategy_default() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(5)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(40),
        })
        .await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: None,
        })
        .await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(35));
    // The preview mirrors the sizing the endpoint just applied.
    assert_eq!(funding.preauth_target, Some(dec!(35)));

    // Already covered: no new intent.
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: None,
        })
        .await?;
    assert_eq!(intent_rows(&app, &auction_id).await?.len(), 1);

    Ok(())
}

/// A member-present pre-authorize resizes the hold in both directions:
/// lowering values lowers the strategy target and re-authorizing swaps
/// the hold down to it (the old hold is superseded and canceled), while
/// an explicit reduction below the commitment's card need is refused
/// with the replacement floor.
#[tokio::test]
async fn member_preauth_resizes_down() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    // Budget strategy, no balance: the hold covers the full budget.
    app.login_bob().await?;
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(40),
        })
        .await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: None,
        })
        .await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(40));

    // Lowering the value lowers the target; re-authorizing swaps the
    // hold down to it.
    app.client
        .create_or_update_user_value(&requests::UserValue {
            space_id,
            value: dec!(20),
        })
        .await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.preauth_target, Some(dec!(20)));
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: None,
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 2);
    let old = rows.iter().find(|r| r.status == "superseded").unwrap();
    assert!(!old.is_active);
    assert_eq!(old.authorized_amount, Some(dec!(40)));
    let new = rows.iter().find(|r| r.status == "authorized").unwrap();
    assert!(new.is_active);
    assert_eq!(new.authorized_amount, Some(dec!(20)));

    // The worker cancels the superseded hold at Stripe.
    app.tick().await;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(20));

    // With a bid committed and no balance, an explicit reduction below
    // the commitment's card need is refused with the floor.
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    assert_api_error(
        app.client
            .authorize_funding(&requests::AuthorizeFunding {
                auction_id,
                amount: Some(dec!(5)),
            })
            .await,
        ApiError::HoldReplacementTooSmall {
            current: dec!(20),
            floor: dec!(10),
        },
    );
    // At or above the floor, the explicit amount lands exactly.
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(12)),
        })
        .await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(12));
    assert_eq!(funding.commitment, dec!(10));

    Ok(())
}

/// Without a proxy-bidding row the auction budget assumes a single
/// item — the hold sizes to the largest valued space, not the sum —
/// and setting proxy `max_items` widens it, with the member-present
/// re-authorize raising exactly to the new target.
#[tokio::test]
async fn budget_defaults_to_one_item_without_proxy() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (space_a, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    let site_id: payloads::SiteId =
        sqlx::query_scalar("SELECT site_id FROM spaces WHERE id = $1")
            .bind(space_a)
            .fetch_one(&app.db_pool)
            .await?;
    let space_b = app
        .client
        .create_space(&test_helpers::space_details_b(site_id))
        .await?;

    app.login_bob().await?;
    for (space_id, value) in [(space_a, dec!(40)), (space_b, dec!(30))] {
        app.client
            .create_or_update_user_value(&requests::UserValue {
                space_id,
                value,
            })
            .await?;
    }
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.preauth_target, Some(dec!(40)));
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: None,
        })
        .await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(40));

    // Proxy bidding for two items widens the budget to the sum; the
    // re-authorize raises exactly to it.
    app.client
        .create_or_update_proxy_bidding(&requests::UseProxyBidding {
            auction_id,
            max_items: 2,
        })
        .await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: None,
        })
        .await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(70));

    Ok(())
}

/// Requested pre-auth amounts are validated up front: non-positive and
/// over the card-charge maximum are rejected before any order commits —
/// an over-limit pending order would fail permanently at Stripe on
/// every replay, poisoning the card path until the aged-order arm.
#[tokio::test]
async fn authorize_funding_amount_validation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    assert_api_error(
        app.client
            .authorize_funding(&requests::AuthorizeFunding {
                auction_id,
                amount: Some(dec!(0)),
            })
            .await,
        ApiError::AmountMustBePositive,
    );
    assert_api_error(
        app.client
            .authorize_funding(&requests::AuthorizeFunding {
                auction_id,
                amount: Some(dec!(1_000_000)),
            })
            .await,
        ApiError::AmountTooLarge {
            max: dec!(999999.99),
        },
    );
    assert_eq!(intent_rows(&app, &auction_id).await?.len(), 0);

    Ok(())
}

/// A 3DS demand surfaces `authentication_required` — the error code,
/// not the accompanying `authentication_not_handled` decline code
/// Stripe sends beside it — since the UI keys the resolve-via-checkout
/// path off that string, and the recorded decline matches.
#[tokio::test]
async fn authorize_funding_3ds_surfaces_error_code() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.stripe_service
        .mock_declining_payment_methods
        .lock()
        .unwrap()
        .insert("pm_bob".to_string(), "authentication_required".to_string());
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    app.login_bob().await?;
    assert_api_error(
        app.client
            .authorize_funding(&requests::AuthorizeFunding {
                auction_id,
                amount: Some(dec!(5)),
            })
            .await,
        ApiError::CardDeclined {
            code: Some("authentication_required".to_string()),
        },
    );
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].last_decline_code.as_deref(),
        Some("authentication_required")
    );

    Ok(())
}

/// A member-present authorize that finds a stranded pending order too
/// small for the request cancels it in place and orders fresh at the
/// current size — reusing it would execute the stored amount and
/// under-deliver while reporting success. The stranded order's crashed
/// execute did mint a hold at Stripe: its retried confirm webhook finds
/// the row canceled (adoption closed) and resolves the hold on the
/// spot instead of waiting for the orphaned-hold sweep.
#[tokio::test]
async fn undersized_stranded_order_replaced() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    let stranded_id = stage_pending_intent(&app, &auction_id, &bob).await?;
    stage_mock_auth(
        &app,
        "pi_stranded",
        stranded_id,
        app.time_source.now().as_second() + 7 * 24 * 3600,
    );

    // The stranded order is for 7; the member asks for 9.
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(9)),
        })
        .await?;

    let (status, pi): (String, Option<String>) = sqlx::query_as(
        "SELECT status::TEXT, payment_intent_id FROM funding_intents \
         WHERE id = $1",
    )
    .bind(stranded_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(status, "canceled");
    assert_eq!(pi, None);
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(9));

    // The retried webhook for the replaced order's hold: adoption is
    // closed, so the handler cancels the hold at Stripe and reconciles
    // the row immediately.
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &adoption_event(&app, "pi_stranded", stranded_id),
    )
    .await?;
    {
        let intents = app.stripe_service.mock_payment_intents.lock().unwrap();
        assert_eq!(intents.get("pi_stranded").unwrap().status, "canceled");
    }
    let (pi, reconciled): (Option<String>, bool) = sqlx::query_as(
        "SELECT payment_intent_id, reconciled_at IS NOT NULL \
         FROM funding_intents WHERE id = $1",
    )
    .bind(stranded_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(pi.as_deref(), Some("pi_stranded"));
    assert!(reconciled);

    // Redelivery converges: the row now matches by PaymentIntent id and
    // the event no-ops; the replacement authorization is untouched.
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &adoption_event(&app, "pi_stranded", stranded_id),
    )
    .await?;
    let funding = app.client.get_auction_funding(&auction_id).await?;
    assert_eq!(funding.authorized, dec!(9));

    Ok(())
}

/// A stranded pending order large enough to cover the request is reused
/// as the order — its id is the idempotency-key seed a crashed create
/// may already have used — and the execute holds its stored amount.
#[tokio::test]
async fn covering_stranded_order_reused() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    let stranded_id = stage_pending_intent(&app, &auction_id, &bob).await?;

    // The stranded order is for 7; the member asks for 2.
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(2)),
        })
        .await?;

    let rows: Vec<(uuid::Uuid, String, Option<Decimal>)> = sqlx::query_as(
        "SELECT id, status::TEXT, authorized_amount FROM funding_intents \
         WHERE auction_id = $1 AND user_id = $2",
    )
    .bind(auction_id)
    .bind(bob)
    .fetch_all(&app.db_pool)
    .await?;
    assert_eq!(rows.len(), 1, "the covering order is reused, not replaced");
    assert_eq!(rows[0].0, stranded_id);
    assert_eq!(rows[0].1, "authorized");
    assert_eq!(rows[0].2, Some(dec!(7)));

    Ok(())
}

/// A canceled event whose object is actually still a live hold (stale
/// or reordered delivery) is a no-op: the poke re-derives from live
/// state, so the last poke always lands on the truth.
#[tokio::test]
async fn stale_cancel_event_is_noop() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(6)),
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();

    // The payload claims canceled; the live intent still holds.
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.canceled",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": pi_id,
                "cancellation_reason": "requested_by_customer",
            }},
        }),
    )
    .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "authorized");
    assert!(rows[0].is_active);
    Ok(())
}

/// A succeeded event for a live authorization (the dashboard's Capture
/// button) converges through the webhook itself: the row books as
/// captured with the collected amount credited — no waiting for
/// reconciliation.
#[tokio::test]
async fn succeeded_webhook_books_out_of_band_capture() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    app.login_bob().await?;
    app.client
        .authorize_funding(&requests::AuthorizeFunding {
            auction_id,
            amount: Some(dec!(6)),
        })
        .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    let pi_id = rows[0].payment_intent_id.clone().unwrap();
    {
        let mut intents =
            app.stripe_service.mock_payment_intents.lock().unwrap();
        let intent = intents.get_mut(&pi_id).unwrap();
        intent.status = "succeeded".to_string();
        intent.amount_received = 600;
    }

    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.succeeded",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": pi_id,
                "amount_received": 600,
            }},
        }),
    )
    .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "captured");
    assert!(!rows[0].is_active);
    assert_eq!(
        crate::funding_capture::member_balance(&app, &community_id, &bob)
            .await?,
        dec!(6)
    );
    Ok(())
}

/// Webhook intent handlers: adoption of an orphaned create into its
/// pending row, out-of-band cancellation of a live authorization, and
/// payment_failed decline metadata. Foreign events (no metadata, unknown
/// intent) are skipped without error.
#[tokio::test]
async fn connect_webhook_intent_handlers() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    // Foreign event: unknown intent, no metadata — expected traffic.
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.amount_capturable_updated",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": "pi_foreign", "amount_capturable": 100,
            }},
        }),
    )
    .await?;

    // Adoption: a pending row whose create succeeded at Stripe but whose
    // finalize never ran (crash window). The event carries our metadata;
    // the handler retrieves the intent's live state and converges the
    // row to an active authorization.
    let pending_id = stage_pending_intent(&app, &auction_id, &bob).await?;
    stage_mock_auth(
        &app,
        "pi_orphan",
        pending_id,
        app.time_source.now().as_second() + 7 * 24 * 3600,
    );
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.amount_capturable_updated",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": "pi_orphan",
                "amount_capturable": 700,
                "created": app.time_source.now().as_second(),
                "metadata": {"funding_intent_id": pending_id.to_string()},
            }},
        }),
    )
    .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "authorized");
    assert!(rows[0].is_active);
    assert_eq!(rows[0].authorized_amount, Some(dec!(7)));
    assert_eq!(rows[0].payment_intent_id.as_deref(), Some("pi_orphan"));
    // The adopted authorization backs bids like any other.
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;

    // Out-of-band cancel (community dashboard): the poke retrieves the
    // canceled live state and the row converges; the standing bid's
    // lost backing is logged.
    {
        let mut intents =
            app.stripe_service.mock_payment_intents.lock().unwrap();
        let intent = intents.get_mut("pi_orphan").unwrap();
        intent.status = "canceled".to_string();
        intent.cancellation_reason = Some("requested_by_customer".to_string());
    }
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.canceled",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": "pi_orphan",
                "cancellation_reason": "requested_by_customer",
            }},
        }),
    )
    .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "canceled");
    assert!(!rows[0].is_active);

    // payment_failed on a pending row: decline metadata, canceled.
    let pending2 = stage_pending_intent(&app, &auction_id, &bob).await?;
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.payment_failed",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": "pi_failed",
                "metadata": {"funding_intent_id": pending2.to_string()},
                "last_payment_error": {
                    "code": "card_declined",
                    "decline_code": "expired_card",
                },
            }},
        }),
    )
    .await?;
    let (status, code): (String, Option<String>) = sqlx::query_as(
        "SELECT status::TEXT, last_decline_code FROM funding_intents \
         WHERE id = $1",
    )
    .bind(pending2)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(status, "canceled");
    assert_eq!(code.as_deref(), Some("expired_card"));

    Ok(())
}

/// Intent events are scoped to the event's envelope account: an event forging
/// another community's `funding_intent_id` from a foreign connected account
/// must not adopt the pending row, and a foreign event naming a stored
/// PaymentIntent id must not converge it — the envelope account is the
/// authorization, not the metadata.
#[tokio::test]
async fn intent_events_from_foreign_account_are_ignored() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    let pending_id = stage_pending_intent(&app, &auction_id, &bob).await?;

    // Correct metadata, wrong envelope account: no adoption.
    let forged = json!({
        "type": "payment_intent.amount_capturable_updated",
        "account": "acct_attacker",
        "data": {"object": {
            "id": "pi_foreign_forged",
            "amount_capturable": 700,
            "created": app.time_source.now().as_second(),
            "metadata": {"funding_intent_id": pending_id.to_string()},
        }},
    });
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &forged,
    )
    .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "pending");
    assert_eq!(rows[0].payment_intent_id, None);

    // The same event on the community's own account adopts normally.
    stage_mock_auth(
        &app,
        "pi_foreign_forged",
        pending_id,
        app.time_source.now().as_second() + 7 * 24 * 3600,
    );
    let mut adopted = forged;
    adopted["account"] = json!("acct_mock_1");
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &adopted,
    )
    .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "authorized");

    // A foreign-account cancel naming the stored intent id doesn't
    // touch the row.
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.canceled",
            "account": "acct_attacker",
            "data": {"object": {
                "id": "pi_foreign_forged",
                "cancellation_reason": "requested_by_customer",
            }},
        }),
    )
    .await?;
    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "authorized");
    assert!(rows[0].is_active);

    Ok(())
}

/// Stage a pending intent for (auction, member), returning its id.
async fn stage_pending_intent(
    app: &TestApp,
    auction_id: &AuctionId,
    user_id: &UserId,
) -> anyhow::Result<uuid::Uuid> {
    let (pending_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO funding_intents \
         (auction_id, user_id, status, origin, requested_amount, \
          created_at, updated_at) \
         VALUES ($1, $2, 'pending', 'bid_flow', 7, NOW(), NOW()) \
         RETURNING id",
    )
    .bind(auction_id)
    .bind(user_id)
    .fetch_one(&app.db_pool)
    .await?;
    Ok(pending_id)
}

/// Stage a live authorized intent (the (auction, member)'s active
/// hold), returning its id.
async fn stage_live_auth(
    app: &TestApp,
    auction_id: &AuctionId,
    user_id: &UserId,
    payment_intent_id: &str,
) -> anyhow::Result<uuid::Uuid> {
    let (id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO funding_intents \
         (auction_id, user_id, payment_intent_id, is_active, status, \
          origin, requested_amount, authorized_amount, capture_before, \
          authorized_at, created_at, updated_at) \
         VALUES ($1, $2, $3, TRUE, 'authorized', 'bid_flow', 5, 5, \
                 NOW() + INTERVAL '7 days', NOW(), NOW(), NOW()) \
         RETURNING id",
    )
    .bind(auction_id)
    .bind(user_id)
    .bind(payment_intent_id)
    .fetch_one(&app.db_pool)
    .await?;
    Ok(id)
}

/// Insert a mock PaymentIntent awaiting capture on the community's
/// account, as a crashed execute's successful confirm would leave it.
fn stage_mock_auth(
    app: &TestApp,
    pi_id: &str,
    intent_id: uuid::Uuid,
    capture_before_epoch: i64,
) {
    app.stripe_service
        .mock_payment_intents
        .lock()
        .unwrap()
        .insert(
            pi_id.to_string(),
            api::stripe_service::MockPaymentIntent {
                account_id: "acct_mock_1".to_string(),
                amount_minor: 700,
                status: "requires_capture".to_string(),
                metadata: std::collections::HashMap::from([(
                    "funding_intent_id".to_string(),
                    intent_id.to_string(),
                )]),
                amount_received: 0,
                application_fee_minor: None,
                capture_before_epoch: Some(capture_before_epoch),
                cancellation_reason: None,
                capture_idempotency_key: None,
            },
        );
}

/// The adoption event payload for a PaymentIntent.
fn adoption_event(app: &TestApp, pi_id: &str, intent_id: uuid::Uuid) -> Value {
    json!({
        "type": "payment_intent.amount_capturable_updated",
        "account": "acct_mock_1",
        "data": {"object": {
            "id": pi_id,
            "amount_capturable": 700,
            "created": app.time_source.now().as_second(),
            "metadata": {"funding_intent_id": intent_id.to_string()},
        }},
    })
}

/// Webhook adoption stores the retrieved per-charge capture window, not
/// the assumed floor: the event payload lacks the window, so the handler
/// must read it from the PaymentIntent itself.
#[tokio::test]
async fn adoption_uses_retrieved_capture_window() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    let pending_id = stage_pending_intent(&app, &auction_id, &bob).await?;

    // A 7-day window, well past the floor the old code assumed.
    let capture_before_epoch =
        app.time_source.now().as_second() + 7 * 24 * 3600;
    stage_mock_auth(&app, "pi_adopt", pending_id, capture_before_epoch);
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &adoption_event(&app, "pi_adopt", pending_id),
    )
    .await?;

    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "authorized");
    assert!(rows[0].is_active);
    assert_eq!(rows[0].authorized_amount, Some(dec!(7)));
    assert_eq!(rows[0].payment_intent_id.as_deref(), Some("pi_adopt"));
    let (stored,): (i64,) = sqlx::query_as(
        "SELECT extract(epoch FROM capture_before)::BIGINT \
         FROM funding_intents WHERE id = $1",
    )
    .bind(pending_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(
        stored, capture_before_epoch,
        "adoption must store the per-charge window, not the floor"
    );
    Ok(())
}

/// Webhook adoption of an authorization whose true window can't cover
/// the auction deadline leaves the row pending and the live predecessor
/// untouched — activating it would supersede a valid hold and set up
/// the finalize's short-window rejection to strip the member's backing.
#[tokio::test]
async fn adoption_refuses_short_window_auth() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    let live_id = stage_live_auth(&app, &auction_id, &bob, "pi_live").await?;
    let pending_id = stage_pending_intent(&app, &auction_id, &bob).await?;

    // A 24h window: below the auction's 60h viability requirement.
    let capture_before_epoch = app.time_source.now().as_second() + 24 * 3600;
    stage_mock_auth(&app, "pi_short", pending_id, capture_before_epoch);
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &adoption_event(&app, "pi_short", pending_id),
    )
    .await?;

    let (status, pi): (String, Option<String>) = sqlx::query_as(
        "SELECT status::TEXT, payment_intent_id FROM funding_intents \
         WHERE id = $1",
    )
    .bind(pending_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(status, "pending", "short-window auth must not activate");
    assert_eq!(pi, None);
    let (live_status, live_active): (String, bool) = sqlx::query_as(
        "SELECT status::TEXT, is_active FROM funding_intents WHERE id = $1",
    )
    .bind(live_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(live_status, "authorized");
    assert!(live_active, "the live predecessor must stay active");
    Ok(())
}

/// An adoption event whose PaymentIntent doesn't exist on the event's
/// account (gone, or a payload we can't verify) leaves the row pending
/// without erroring — the aged-pending arm and orphan sweep converge it.
#[tokio::test]
async fn adoption_missing_intent_leaves_row_pending() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    let pending_id = stage_pending_intent(&app, &auction_id, &bob).await?;

    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &adoption_event(&app, "pi_ghost", pending_id),
    )
    .await?;

    let rows = intent_rows(&app, &auction_id).await?;
    assert_eq!(rows[0].status, "pending");
    assert_eq!(rows[0].payment_intent_id, None);
    Ok(())
}

/// `activate_intent_tx` with a target that already reached a terminal
/// status is a whole no-op: demoting first would strand the funding row
/// with no active authorization and hand the live predecessor's
/// still-valid hold to the cancel worker.
#[tokio::test]
async fn dead_target_activation_is_inert() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    let (_, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;
    let live_id = stage_live_auth(&app, &auction_id, &bob, "pi_live").await?;
    // A raise order converged to canceled by an out-of-band cancel's
    // webhook before its finalize could activate it.
    let (dead_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO funding_intents \
         (auction_id, user_id, payment_intent_id, status, origin, \
          requested_amount, created_at, updated_at) \
         VALUES ($1, $2, 'pi_dead', 'canceled', 'bid_flow', 7, \
          NOW(), NOW()) \
         RETURNING id",
    )
    .bind(auction_id)
    .bind(bob)
    .fetch_one(&app.db_pool)
    .await?;

    let capture_before = jiff::Timestamp::from_second(
        app.time_source.now().as_second() + 7 * 24 * 3600,
    )?;
    let mut ttx = store::locks::TrackedTx::begin(&app.db_pool).await?;
    let outcome = store::funding::activate_intent_tx(
        &payloads::FundingIntentId(dead_id),
        "pi_dead",
        dec!(7),
        capture_before,
        store::funding::AdoptedRows::Include,
        &app.time_source,
        &mut ttx,
    )
    .await?;
    ttx.commit().await?;
    assert_eq!(
        outcome,
        store::funding::ActivateOutcome::RowMovedOn,
        "a terminal target must not promote"
    );

    let (live_status, live_active): (String, bool) = sqlx::query_as(
        "SELECT status::TEXT, is_active FROM funding_intents WHERE id = $1",
    )
    .bind(live_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(
        live_status, "authorized",
        "the live predecessor must not be demoted"
    );
    assert!(live_active);
    let (dead_status,): (String,) = sqlx::query_as(
        "SELECT status::TEXT FROM funding_intents WHERE id = $1",
    )
    .bind(dead_id)
    .fetch_one(&app.db_pool)
    .await?;
    assert_eq!(dead_status, "canceled");
    Ok(())
}

/// An out-of-band platform-dashboard detach clears the stored card via
/// the platform webhook (the profile sync can't see it — the method
/// list is just empty).
#[tokio::test]
async fn payment_method_detached_webhook() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    save_bob_card(&app).await?; // alice is the logged-in user here

    // Simulate a dashboard detach: Stripe no longer lists the method,
    // and the platform webhook delivers payment_method.detached.
    app.stripe_service
        .mock_customer_payment_methods
        .lock()
        .unwrap()
        .clear();
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_method.detached",
            "data": {"object": {"id": "pm_bob"}},
        }),
    )
    .await?;

    let profile = app.client.get_payment_profile().await?;
    assert_eq!(profile.card, None);
    Ok(())
}
