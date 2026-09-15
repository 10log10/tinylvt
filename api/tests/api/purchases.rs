//! Credit purchase tests (phase 8): the Checkout-backed top-up flow
//! and its webhook lifecycle (completed/processing/succeeded/failed/
//! expired), idempotent issuance, amount validation, and exact-amount
//! debt settlement against the effective balance (balance + pending
//! captures).

use api::store;
use payloads::{ApiError, CommunityId, PurchaseKind, PurchaseStatus, requests};
use rust_decimal::dec;
use serde_json::json;
use test_helpers::{TestApp, assert_api_error, spawn_app};

use crate::funding::{create_open_auction, credit_member};
use crate::funding_auth::{bob_user_id, card_enabled_setup, intent_rows};
use crate::funding_capture::member_balance;

#[derive(Debug, sqlx::FromRow)]
struct PurchaseRow {
    id: uuid::Uuid,
    status: String,
    checkout_session_id: Option<String>,
    payment_intent_id: Option<String>,
}

async fn purchase_rows(
    app: &TestApp,
    community_id: &CommunityId,
) -> anyhow::Result<Vec<PurchaseRow>> {
    Ok(sqlx::query_as(
        "SELECT id, status::TEXT, checkout_session_id, payment_intent_id \
         FROM credit_purchases WHERE community_id = $1 \
         ORDER BY created_at, id",
    )
    .bind(community_id)
    .fetch_all(&app.db_pool)
    .await?)
}

/// Seed a mock PaymentIntent so code that consults live intent state
/// (the stuck-settlement probe) sees it on the given account.
fn seed_mock_intent(
    app: &TestApp,
    account_id: &str,
    payment_intent_id: &str,
    status: &str,
    amount_received: i64,
) {
    app.stripe_service
        .mock_payment_intents
        .lock()
        .unwrap()
        .insert(
            payment_intent_id.to_string(),
            api::stripe_service::MockPaymentIntent {
                account_id: account_id.to_string(),
                amount_minor: amount_received,
                status: status.to_string(),
                metadata: Default::default(),
                amount_received,
                application_fee_minor: None,
                capture_before_epoch: None,
                cancellation_reason: None,
                capture_idempotency_key: None,
            },
        );
}

/// Stage a mock Checkout session as paid: status `complete` with the
/// collecting PaymentIntent. Pokes retrieve truth — a payload alone no
/// longer moves rows.
fn set_mock_session_complete(app: &TestApp, session_id: &str, pi_id: &str) {
    let mut sessions = app.stripe_service.mock_sessions.lock().unwrap();
    let session = sessions.get_mut(session_id).unwrap();
    session.status = "complete".to_string();
    session.payment_intent_id = Some(pi_id.to_string());
}

/// Set a mock Checkout session's status directly (e.g. `expired`).
fn set_mock_session_status(app: &TestApp, session_id: &str, status: &str) {
    app.stripe_service
        .mock_sessions
        .lock()
        .unwrap()
        .get_mut(session_id)
        .unwrap()
        .status = status.to_string();
}

/// Deliver a Connect webhook event for a purchase's PaymentIntent.
async fn purchase_intent_event(
    app: &TestApp,
    event_type: &str,
    payment_intent_id: &str,
    purchase_id: uuid::Uuid,
) -> anyhow::Result<()> {
    // Mirror Stripe's `amount_received` (minor units) so the issuance amount
    // check passes; derived from the row so every caller need not pass it.
    let (amount, minor_units): (rust_decimal::Decimal, i16) = sqlx::query_as(
        "SELECT cp.amount, c.currency_minor_units \
             FROM credit_purchases cp \
             JOIN communities c ON cp.community_id = c.id \
             WHERE cp.id = $1",
    )
    .bind(purchase_id)
    .fetch_one(&app.db_pool)
    .await?;
    let amount_received =
        payloads::to_minor_units(amount, minor_units).unwrap();
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": event_type,
            "account": "acct_mock_1",
            "data": {"object": {
                "id": payment_intent_id,
                "amount_received": amount_received,
                "metadata": {"credit_purchase_id": purchase_id.to_string()},
            }},
        }),
    )
    .await?;
    Ok(())
}

/// A top-up purchase: session minted, PaymentIntent linked by
/// `checkout.session.completed`, credits issued exactly once on
/// `payment_intent.succeeded` (redundant delivery no-ops via the
/// entry idempotency key).
#[tokio::test]
async fn top_up_purchase_issues_credits_once() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;

    let response = app
        .client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::TopUp,
            amount: dec!(25),
        })
        .await?;
    assert!(
        response
            .checkout_url
            .contains("mock-stripe.invalid/purchase/")
    );

    let rows = purchase_rows(&app, &community_id).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "created");
    let session_id = rows[0].checkout_session_id.clone().unwrap();
    assert_eq!(session_id, format!("cs_mock_{}", rows[0].id));

    // The session charges the full amount on the community's account.
    // The platform fee is suspended (rate zero), so none is attached.
    {
        let sessions = app.stripe_service.mock_sessions.lock().unwrap();
        let session = sessions.get(&session_id).unwrap();
        assert_eq!(session.account_id, "acct_mock_1");
        assert_eq!(session.amount_minor, 2500);
        assert_eq!(session.currency, "usd");
        assert_eq!(session.application_fee_minor, None);
    }

    // The member pays (card: the intent succeeds at completion); the
    // session event pokes convergence, which links the PaymentIntent
    // from the live session and issues immediately.
    set_mock_session_complete(&app, &session_id, "pi_purchase_1");
    seed_mock_intent(&app, "acct_mock_1", "pi_purchase_1", "succeeded", 2500);
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "checkout.session.completed",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": session_id,
                "payment_intent": "pi_purchase_1",
                "metadata": {
                    "credit_purchase_id": rows[0].id.to_string(),
                },
            }},
        }),
    )
    .await?;
    let rows = purchase_rows(&app, &community_id).await?;
    assert_eq!(rows[0].payment_intent_id.as_deref(), Some("pi_purchase_1"));
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(25));

    purchase_intent_event(
        &app,
        "payment_intent.succeeded",
        "pi_purchase_1",
        rows[0].id,
    )
    .await?;
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(25));
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "succeeded"
    );

    // Redundant delivery: no double issuance.
    purchase_intent_event(
        &app,
        "payment_intent.succeeded",
        "pi_purchase_1",
        rows[0].id,
    )
    .await?;
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(25));

    let listed = app.client.list_credit_purchases(&community_id).await?;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].status, PurchaseStatus::Succeeded);
    assert_eq!(listed[0].kind, PurchaseKind::TopUp);
    assert_eq!(listed[0].amount, dec!(25));
    Ok(())
}

/// Top-up amount validation: below the minimum charge and finer than
/// the currency's minor units are both rejected.
#[tokio::test]
async fn top_up_amount_validation() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;

    for amount in [dec!(0.30), dec!(1.001), dec!(0)] {
        assert_api_error(
            app.client
                .create_credit_purchase(&requests::CreateCreditPurchase {
                    community_id,
                    kind: PurchaseKind::TopUp,
                    amount,
                })
                .await,
            ApiError::InvalidPurchaseAmount,
        );
    }
    assert_api_error(
        app.client
            .create_credit_purchase(&requests::CreateCreditPurchase {
                community_id,
                kind: PurchaseKind::TopUp,
                amount: dec!(1_000_000),
            })
            .await,
        ApiError::AmountTooLarge {
            max: dec!(999999.99),
        },
    );
    assert!(purchase_rows(&app, &community_id).await?.is_empty());
    Ok(())
}

/// Purchases need a charges-enabled connected account.
#[tokio::test]
async fn purchase_requires_card_payments() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;
    app.set_backed_credits_mode(&community_id).await?;
    app.login_bob().await?;
    assert_api_error(
        app.client
            .create_credit_purchase(&requests::CreateCreditPurchase {
                community_id,
                kind: PurchaseKind::TopUp,
                amount: dec!(10),
            })
            .await,
        ApiError::CardPaymentsNotEnabled,
    );
    Ok(())
}

/// A delayed payment method (ACH): `processing` shows the purchase as
/// pending, `payment_failed` fails it without credits, and a late
/// success (Stripe collected after all) still issues.
#[tokio::test]
async fn delayed_purchase_processing_failure_and_late_success()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;

    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::TopUp,
            amount: dec!(30),
        })
        .await?;
    let purchase_id = purchase_rows(&app, &community_id).await?[0].id;

    purchase_intent_event(
        &app,
        "payment_intent.processing",
        "pi_ach_1",
        purchase_id,
    )
    .await?;
    let listed = app.client.list_credit_purchases(&community_id).await?;
    assert_eq!(listed[0].status, PurchaseStatus::Processing);
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));

    // The ACH bounces: the intent parks at requires_payment_method and
    // the failure poke converges from that live state.
    seed_mock_intent(
        &app,
        "acct_mock_1",
        "pi_ach_1",
        "requires_payment_method",
        0,
    );
    purchase_intent_event(
        &app,
        "payment_intent.payment_failed",
        "pi_ach_1",
        purchase_id,
    )
    .await?;
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "failed"
    );
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));

    // The bank payment turns out to have gone through: the money was
    // collected, so the credits issue despite the local failure mark.
    seed_mock_intent(&app, "acct_mock_1", "pi_ach_1", "succeeded", 3000);
    purchase_intent_event(
        &app,
        "payment_intent.succeeded",
        "pi_ach_1",
        purchase_id,
    )
    .await?;
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "succeeded"
    );
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(30));
    Ok(())
}

/// An abandoned Checkout session retires the purchase row.
#[tokio::test]
async fn abandoned_session_expires() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;

    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::TopUp,
            amount: dec!(10),
        })
        .await?;
    let session_id = purchase_rows(&app, &community_id).await?[0]
        .checkout_session_id
        .clone()
        .unwrap();

    // Stripe expires the abandoned session; the event pokes convergence
    // against that live state.
    set_mock_session_status(&app, &session_id, "expired");
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "checkout.session.expired",
            "account": "acct_mock_1",
            "data": {"object": {"id": session_id}},
        }),
    )
    .await?;
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "expired"
    );
    Ok(())
}

/// Debt settlement end to end. Bob wins beyond his balance, the capture
/// is canceled out-of-band (terminal failure), and the resulting debt
/// is settled with an exact-amount purchase. While the capture was
/// merely pending, the effective balance counted it and no debt was
/// repayable; a stale or wrong amount is rejected; a processing
/// settlement blocks a second attempt.
#[tokio::test]
async fn debt_settlement_flow() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    app.login_alice().await?;
    credit_member(&app, community_id, bob, dec!(4)).await?;
    let (space_id, auction_id) =
        create_open_auction(&app, community_id, "site a", dec!(10)).await?;

    // Bob wins at 10 with 4 in balance: capture of 6, frozen
    // un-capturable so it stays pending.
    app.login_bob().await?;
    let rounds = app.client.list_auction_rounds(&auction_id).await?;
    app.client
        .create_bid(&space_id, &rounds[0].round_id)
        .await?;
    let pi = intent_rows(&app, &auction_id).await?[0]
        .payment_intent_id
        .clone()
        .unwrap();
    app.stripe_service
        .mock_payment_intents
        .lock()
        .unwrap()
        .get_mut(&pi)
        .unwrap()
        .status = "processing".to_string();
    crate::funding::run_until_ended(&app, &[auction_id]).await?;

    // Capture pending: the effective balance counts it, so nothing is
    // repayable yet and the exact-amount check rejects any attempt.
    let info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(info.balance, dec!(-6));
    assert_eq!(info.pending_captures, dec!(6));
    assert_api_error(
        app.client
            .create_credit_purchase(&requests::CreateCreditPurchase {
                community_id,
                kind: PurchaseKind::DebtSettlement,
                amount: dec!(6),
            })
            .await,
        ApiError::DebtAmountMismatch,
    );

    // Out-of-band cancel makes the capture failure terminal: real debt.
    // Pokes retrieve live state, so stage the truth first.
    app.stripe_service
        .mock_payment_intents
        .lock()
        .unwrap()
        .get_mut(&pi)
        .unwrap()
        .status = "canceled".to_string();
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.canceled",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": pi,
                "cancellation_reason": "requested_by_customer",
            }},
        }),
    )
    .await?;
    let info = app
        .client
        .get_member_currency_info(&requests::GetMemberCurrencyInfo {
            community_id,
            member_user_id: None,
        })
        .await?;
    assert_eq!(info.balance, dec!(-6));
    assert_eq!(info.pending_captures, dec!(0));

    // A stale amount is rejected; the exact debt starts checkout.
    assert_api_error(
        app.client
            .create_credit_purchase(&requests::CreateCreditPurchase {
                community_id,
                kind: PurchaseKind::DebtSettlement,
                amount: dec!(5),
            })
            .await,
        ApiError::DebtAmountMismatch,
    );
    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::DebtSettlement,
            amount: dec!(6),
        })
        .await?;
    let settlement_id =
        purchase_rows(&app, &community_id).await?.last().unwrap().id;

    // The bank payment starts processing: a second settlement attempt
    // is blocked (it would overshoot once both collect). The attempt's
    // probe consults live intent state, so model the ACH genuinely in
    // flight at Stripe.
    seed_mock_intent(&app, "acct_mock_1", "pi_settle_1", "processing", 0);
    purchase_intent_event(
        &app,
        "payment_intent.processing",
        "pi_settle_1",
        settlement_id,
    )
    .await?;
    assert_api_error(
        app.client
            .create_credit_purchase(&requests::CreateCreditPurchase {
                community_id,
                kind: PurchaseKind::DebtSettlement,
                amount: dec!(6),
            })
            .await,
        ApiError::SettlementAlreadyProcessing,
    );

    seed_mock_intent(&app, "acct_mock_1", "pi_settle_1", "succeeded", 600);
    purchase_intent_event(
        &app,
        "payment_intent.succeeded",
        "pi_settle_1",
        settlement_id,
    )
    .await?;
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    let listed = app.client.list_credit_purchases(&community_id).await?;
    let settled = listed.iter().find(|p| p.id.0 == settlement_id).unwrap();
    assert_eq!(settled.kind, PurchaseKind::DebtSettlement);
    assert_eq!(settled.status, PurchaseStatus::Succeeded);

    // Debt cleared: nothing further is repayable.
    assert_api_error(
        app.client
            .create_credit_purchase(&requests::CreateCreditPurchase {
                community_id,
                kind: PurchaseKind::DebtSettlement,
                amount: dec!(6),
            })
            .await,
        ApiError::DebtAmountMismatch,
    );
    Ok(())
}

/// Manufacture debt directly: a treasury debit is simpler than the
/// full capture-failure flow exercised in `debt_settlement_flow`.
async fn set_member_balance(
    app: &TestApp,
    community_id: &CommunityId,
    user_id: &payloads::UserId,
    balance: rust_decimal::Decimal,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE accounts SET balance_cached = $1 \
         WHERE community_id = $2 AND owner_type = 'member_main' \
           AND owner_id = $3",
    )
    .bind(balance)
    .bind(community_id)
    .bind(user_id)
    .execute(&app.db_pool)
    .await?;
    Ok(())
}

/// A fresh settlement attempt supersedes an abandoned (still 'created')
/// one rather than being blocked by it, and expires the superseded
/// session at Stripe so it can no longer be completed into a second
/// payment.
#[tokio::test]
async fn abandoned_settlement_superseded_by_retry() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    set_member_balance(&app, &community_id, &bob, dec!(-5)).await?;

    for _ in 0..2 {
        app.client
            .create_credit_purchase(&requests::CreateCreditPurchase {
                community_id,
                kind: PurchaseKind::DebtSettlement,
                amount: dec!(5),
            })
            .await?;
    }
    // Both rows share the mocked creation instant, so assert on the
    // status multiset rather than an order that would tiebreak on
    // random ids: the first attempt was retired, the retry is live.
    let rows = purchase_rows(&app, &community_id).await?;
    let mut statuses: Vec<_> = rows.iter().map(|r| r.status.as_str()).collect();
    statuses.sort_unstable();
    assert_eq!(statuses, ["created", "expired"]);

    // The retired attempt's Stripe session was expired by the retry;
    // the survivor's stays open.
    let retired = rows.iter().find(|r| r.status == "expired").unwrap();
    let live = rows.iter().find(|r| r.status == "created").unwrap();
    let sessions = app.stripe_service.mock_sessions.lock().unwrap();
    assert_eq!(
        sessions
            .get(&format!("cs_mock_{}", retired.id))
            .unwrap()
            .status,
        "expired"
    );
    assert_eq!(
        sessions
            .get(&format!("cs_mock_{}", live.id))
            .unwrap()
            .status,
        "open"
    );
    Ok(())
}

/// A superseded settlement session the member already paid blocks the
/// new attempt outright: the expire call reports it completed, no new
/// session is minted (no double charge), and the in-flight payment
/// settles the debt through the normal webhook path.
#[tokio::test]
async fn paid_superseded_session_blocks_new_settlement() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    set_member_balance(&app, &community_id, &bob, dec!(-5)).await?;

    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::DebtSettlement,
            amount: dec!(5),
        })
        .await?;
    let rows = purchase_rows(&app, &community_id).await?;
    let session_id = rows[0].checkout_session_id.clone().unwrap();

    // The member pays but abandons the redirect, then retries before
    // any webhook arrives. The payment is still collecting (ACH), so
    // the pre-flight probe leaves the row, the session can't be
    // expired, and the retry is refused and mints nothing.
    set_mock_session_complete(&app, &session_id, "pi_paid_1");
    seed_mock_intent(&app, "acct_mock_1", "pi_paid_1", "processing", 0);
    assert_api_error(
        app.client
            .create_credit_purchase(&requests::CreateCreditPurchase {
                community_id,
                kind: PurchaseKind::DebtSettlement,
                amount: dec!(5),
            })
            .await,
        ApiError::SettlementAlreadyProcessing,
    );
    let rows = purchase_rows(&app, &community_id).await?;
    assert_eq!(rows.len(), 1);
    // The retry's probe converged the paid row from live state: the
    // session is complete and its intent still collecting.
    assert_eq!(rows[0].status, "processing");
    assert_eq!(rows[0].payment_intent_id.as_deref(), Some("pi_paid_1"));

    // The payment lands and its webhook settles the debt normally.
    seed_mock_intent(&app, "acct_mock_1", "pi_paid_1", "succeeded", 500);
    purchase_intent_event(
        &app,
        "payment_intent.succeeded",
        "pi_paid_1",
        rows[0].id,
    )
    .await?;
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "succeeded"
    );
    Ok(())
}

/// A succeeded event carrying a real purchase id but originating on a
/// different connected account (a forged cross-account event) must not
/// issue: the envelope account is the authorization, not the metadata.
#[tokio::test]
async fn succeeded_event_from_foreign_account_does_not_issue()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;

    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::TopUp,
            amount: dec!(25),
        })
        .await?;
    let purchase_id = purchase_rows(&app, &community_id).await?[0].id;

    // Same purchase id, wrong account: no row matches, nothing issues.
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.succeeded",
            "account": "acct_attacker",
            "data": {"object": {
                "id": "pi_forged_1",
                "amount_received": 2500,
                "metadata": {
                    "credit_purchase_id": purchase_id.to_string(),
                },
            }},
        }),
    )
    .await?;
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "created"
    );
    Ok(())
}

/// A succeeded event on the right account but for an amount smaller than
/// the purchase's face value must not issue the face value.
#[tokio::test]
async fn succeeded_event_with_wrong_amount_does_not_issue() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;

    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::TopUp,
            amount: dec!(25),
        })
        .await?;
    let rows = purchase_rows(&app, &community_id).await?;
    let purchase_id = rows[0].id;
    let session_id = rows[0].checkout_session_id.clone().unwrap();

    // Correct account, but only 50 minor units collected against a
    // 2500-minor-unit face value: refuse to issue.
    set_mock_session_complete(&app, &session_id, "pi_underpaid_1");
    seed_mock_intent(&app, "acct_mock_1", "pi_underpaid_1", "succeeded", 50);
    store::connect::handle_connect_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &json!({
            "type": "payment_intent.succeeded",
            "account": "acct_mock_1",
            "data": {"object": {
                "id": "pi_underpaid_1",
                "amount_received": 50,
                "metadata": {
                    "credit_purchase_id": purchase_id.to_string(),
                },
            }},
        }),
    )
    .await?;
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    assert_ne!(
        purchase_rows(&app, &community_id).await?[0].status,
        "succeeded"
    );
    Ok(())
}

/// A settlement row that never stored a session id (its creation failed
/// or crashed mid-mint) blocks retries only within the mint grace
/// window; after it, the sweep retires the orphan and a new settlement
/// succeeds.
#[tokio::test]
async fn orphaned_settlement_row_is_grace_retired() -> anyhow::Result<()> {
    use jiff_sqlx::ToSqlx;

    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    set_member_balance(&app, &community_id, &bob, dec!(-5)).await?;

    // A crashed creation: a 'created' row with no session id.
    sqlx::query(
        "INSERT INTO credit_purchases \
         (community_id, user_id, kind, amount, created_at, updated_at) \
         VALUES ($1, $2, 'debt_settlement', $3, $4, $4)",
    )
    .bind(community_id)
    .bind(bob)
    .bind(dec!(5))
    .bind(app.time_source.now().to_sqlx())
    .execute(&app.db_pool)
    .await?;

    // Within the grace window the orphan could still be an in-flight
    // mint: the retry mints, sees the live row at the election, and
    // aborts its own session.
    assert_api_error(
        app.client
            .create_credit_purchase(&requests::CreateCreditPurchase {
                community_id,
                kind: PurchaseKind::DebtSettlement,
                amount: dec!(5),
            })
            .await,
        ApiError::SettlementAlreadyProcessing,
    );

    // Past the window the orphan is provably dead (no URL was ever
    // returned): the sweep retires it and the retry succeeds.
    app.time_source.advance(jiff::Span::new().minutes(16));
    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::DebtSettlement,
            amount: dec!(5),
        })
        .await?;

    let rows = purchase_rows(&app, &community_id).await?;
    let mut statuses: Vec<_> = rows.iter().map(|r| r.status.as_str()).collect();
    statuses.sort_unstable();
    assert_eq!(statuses, ["created", "expired", "expired"]);
    Ok(())
}

/// Drive a settlement to 'processing' on the current account: create it
/// and deliver its `payment_intent.processing` event. Returns the row id.
async fn processing_settlement(
    app: &TestApp,
    community_id: &CommunityId,
    payment_intent_id: &str,
) -> anyhow::Result<uuid::Uuid> {
    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id: *community_id,
            kind: PurchaseKind::DebtSettlement,
            amount: dec!(5),
        })
        .await?;
    let settlement_id = purchase_rows(app, community_id).await?[0].id;
    purchase_intent_event(
        app,
        "payment_intent.processing",
        payment_intent_id,
        settlement_id,
    )
    .await?;
    Ok(settlement_id)
}

/// A settlement stuck 'processing' on a since-replaced connected account
/// no longer blocks settlement forever: the new attempt's probe finds no
/// such intent on the current account and retires the row. A late
/// succeeded event from the old account still converges via the stored
/// intent id (unscoped) and issues the collected money.
#[tokio::test]
async fn stuck_processing_settlement_on_replaced_account_heals()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    set_member_balance(&app, &community_id, &bob, dec!(-5)).await?;

    let settlement_id =
        processing_settlement(&app, &community_id, "pi_replaced_1").await?;
    // The intent lives on the original account; the account is then
    // replaced (the swap retires only 'created' rows, so the
    // 'processing' row survives it).
    seed_mock_intent(&app, "acct_mock_1", "pi_replaced_1", "processing", 0);
    sqlx::query(
        "UPDATE communities SET stripe_account_id = 'acct_mock_2' \
         WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    // The retry probes pi_replaced_1 on acct_mock_2, finds nothing,
    // retires the stuck row, and proceeds to mint.
    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::DebtSettlement,
            amount: dec!(5),
        })
        .await?;
    let rows = purchase_rows(&app, &community_id).await?;
    let mut statuses: Vec<_> = rows.iter().map(|r| r.status.as_str()).collect();
    statuses.sort_unstable();
    assert_eq!(statuses, ["created", "expired"]);

    // The old-account ACH lands after all: the late event's envelope
    // account no longer matches the community, but the stored intent id
    // does — the poke retrieves on the envelope account (where the
    // intent lives), and the collected money issues on the retired row.
    seed_mock_intent(&app, "acct_mock_1", "pi_replaced_1", "succeeded", 500);
    purchase_intent_event(
        &app,
        "payment_intent.succeeded",
        "pi_replaced_1",
        settlement_id,
    )
    .await?;
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    let rows = purchase_rows(&app, &community_id).await?;
    let old = rows.iter().find(|r| r.id == settlement_id).unwrap();
    assert_eq!(old.status, "succeeded");
    Ok(())
}

/// A permanently missed `payment_intent.succeeded` on the current
/// account: the money was collected but no credits issued, and the row
/// blocks settlement. The next attempt's probe sees the succeeded intent
/// and issues, after which the debt is cleared and the attempt itself is
/// rejected for amount mismatch.
#[tokio::test]
async fn stuck_processing_settlement_missed_success_issues()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    set_member_balance(&app, &community_id, &bob, dec!(-5)).await?;

    processing_settlement(&app, &community_id, "pi_missed_1").await?;
    seed_mock_intent(&app, "acct_mock_1", "pi_missed_1", "succeeded", 500);

    assert_api_error(
        app.client
            .create_credit_purchase(&requests::CreateCreditPurchase {
                community_id,
                kind: PurchaseKind::DebtSettlement,
                amount: dec!(5),
            })
            .await,
        ApiError::DebtAmountMismatch,
    );
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "succeeded"
    );
    Ok(())
}

/// A missed `payment_intent.payment_failed`: the probe sees the canceled
/// intent, retires the row as failed, and the new settlement proceeds.
#[tokio::test]
async fn stuck_processing_settlement_canceled_intent_retires()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    set_member_balance(&app, &community_id, &bob, dec!(-5)).await?;

    processing_settlement(&app, &community_id, "pi_bounced_1").await?;
    seed_mock_intent(&app, "acct_mock_1", "pi_bounced_1", "canceled", 0);

    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::DebtSettlement,
            amount: dec!(5),
        })
        .await?;
    let rows = purchase_rows(&app, &community_id).await?;
    let mut statuses: Vec<_> = rows.iter().map(|r| r.status.as_str()).collect();
    statuses.sort_unstable();
    assert_eq!(statuses, ["created", "failed"]);
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(-5));
    Ok(())
}

/// A late old-account `payment_intent.payment_failed` converges a
/// 'processing' row through the stored intent id even though the
/// envelope account no longer matches the community — no probe needed.
#[tokio::test]
async fn late_old_account_failure_converges_processing_row()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    set_member_balance(&app, &community_id, &bob, dec!(-5)).await?;

    let settlement_id =
        processing_settlement(&app, &community_id, "pi_late_1").await?;
    seed_mock_intent(
        &app,
        "acct_mock_1",
        "pi_late_1",
        "requires_payment_method",
        0,
    );
    sqlx::query(
        "UPDATE communities SET stripe_account_id = 'acct_mock_2' \
         WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    purchase_intent_event(
        &app,
        "payment_intent.payment_failed",
        "pi_late_1",
        settlement_id,
    )
    .await?;
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "failed"
    );
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(-5));
    Ok(())
}

/// Review 3 #6: `payment_failed` delivered before the delayed
/// `payment_intent.processing` must not strand the row in processing.
/// The failure poke converges from live state (session complete,
/// intent parked at requires_payment_method), and the stale processing
/// payload then finds nothing to move.
#[tokio::test]
async fn reordered_failure_then_processing_converges() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;

    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::TopUp,
            amount: dec!(30),
        })
        .await?;
    let rows = purchase_rows(&app, &community_id).await?;
    let purchase_id = rows[0].id;
    let session_id = rows[0].checkout_session_id.clone().unwrap();

    // The member submitted an ACH payment (session complete) and it
    // bounced; only then do the webhooks arrive, failure first.
    set_mock_session_complete(&app, &session_id, "pi_reorder_1");
    seed_mock_intent(
        &app,
        "acct_mock_1",
        "pi_reorder_1",
        "requires_payment_method",
        0,
    );
    purchase_intent_event(
        &app,
        "payment_intent.payment_failed",
        "pi_reorder_1",
        purchase_id,
    )
    .await?;
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "failed"
    );

    // The stale processing delivery lands afterwards: the payload arm
    // only moves created rows, so the terminal state stands.
    purchase_intent_event(
        &app,
        "payment_intent.processing",
        "pi_reorder_1",
        purchase_id,
    )
    .await?;
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "failed"
    );
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    Ok(())
}

/// Review 3 #1: a paid settlement session whose webhooks were all
/// permanently missed no longer blocks settlement forever. The next
/// attempt's probe retrieves the completed session, takes its
/// PaymentIntent, issues the credits, and the attempt itself is then
/// rejected because the debt is already cleared.
#[tokio::test]
async fn paid_session_with_no_webhooks_heals_at_settlement()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;
    set_member_balance(&app, &community_id, &bob, dec!(-5)).await?;

    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::DebtSettlement,
            amount: dec!(5),
        })
        .await?;
    let rows = purchase_rows(&app, &community_id).await?;
    let settlement_id = rows[0].id;
    let session_id = rows[0].checkout_session_id.clone().unwrap();

    // The member pays and every webhook is lost past Stripe's retry
    // horizon: money collected, row still 'created'.
    set_mock_session_complete(&app, &session_id, "pi_healed_1");
    seed_mock_intent(&app, "acct_mock_1", "pi_healed_1", "succeeded", 500);

    // The retry's probe converges the paid row; with the debt cleared,
    // the stale-amount attempt is refused rather than double-charging.
    assert_api_error(
        app.client
            .create_credit_purchase(&requests::CreateCreditPurchase {
                community_id,
                kind: PurchaseKind::DebtSettlement,
                amount: dec!(5),
            })
            .await,
        ApiError::DebtAmountMismatch,
    );
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    let rows = purchase_rows(&app, &community_id).await?;
    assert_eq!(rows.len(), 1);
    let healed = rows.iter().find(|r| r.id == settlement_id).unwrap();
    assert_eq!(healed.status, "succeeded");
    assert_eq!(healed.payment_intent_id.as_deref(), Some("pi_healed_1"));
    Ok(())
}

/// A paid top-up whose webhook deliveries were all permanently missed
/// is converged by the hourly reconciliation sweep — the only actor
/// reaching a stuck top-up whose member never settles debt — including
/// on a merely charges-disabled account. A fresh row inside the
/// worker-lag grace is left to the webhook path.
#[tokio::test]
async fn stuck_topup_converged_by_reconciliation_sweep() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;

    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::TopUp,
            amount: dec!(25),
        })
        .await?;
    let rows = purchase_rows(&app, &community_id).await?;
    let session_id = rows[0].checkout_session_id.clone().unwrap();

    // Paid at Stripe; every delivery lost. Stripe also pauses charges
    // over requirements — the account stays readable.
    set_mock_session_complete(&app, &session_id, "pi_lost_1");
    seed_mock_intent(&app, "acct_mock_1", "pi_lost_1", "succeeded", 2500);
    sqlx::query(
        "UPDATE communities SET stripe_charges_enabled = FALSE \
         WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    let report = app.reconcile().await;
    assert_eq!(report.purchase_candidates, 0);

    app.time_source.advance(jiff::Span::new().hours(4));
    let report = app.reconcile().await;
    assert_eq!(report.purchase_candidates, 1);
    assert_eq!(report.errors().count(), 0, "{:?}", report.violations);
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "succeeded"
    );
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(25));

    // Terminal rows leave the candidate set.
    let report = app.reconcile().await;
    assert_eq!(report.purchase_candidates, 0);
    Ok(())
}

/// A `processing` row stranded by an account replacement (its intent
/// lives on the gone account) is retired as expired by the sweep's
/// probe on the current account — unblocking the member without
/// disowning the payment, per `converge_purchase`'s missing-intent arm.
#[tokio::test]
async fn swap_stranded_processing_purchase_retired() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = card_enabled_setup(&app).await?;
    let bob = bob_user_id(&app, &community_id).await?;

    app.client
        .create_credit_purchase(&requests::CreateCreditPurchase {
            community_id,
            kind: PurchaseKind::TopUp,
            amount: dec!(30),
        })
        .await?;
    let purchase_id = purchase_rows(&app, &community_id).await?[0].id;
    seed_mock_intent(&app, "acct_mock_1", "pi_ach_swap", "processing", 0);
    purchase_intent_event(
        &app,
        "payment_intent.processing",
        "pi_ach_swap",
        purchase_id,
    )
    .await?;
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "processing"
    );

    sqlx::query(
        "UPDATE communities SET stripe_account_id = 'acct_mock_2' \
         WHERE id = $1",
    )
    .bind(community_id)
    .execute(&app.db_pool)
    .await?;

    app.time_source.advance(jiff::Span::new().hours(4));
    let report = app.reconcile().await;
    assert_eq!(report.purchase_candidates, 1);
    assert_eq!(
        purchase_rows(&app, &community_id).await?[0].status,
        "expired"
    );
    assert_eq!(member_balance(&app, &community_id, &bob).await?, dec!(0));
    Ok(())
}
