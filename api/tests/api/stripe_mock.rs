//! Mock StripeService semantics tests: behaviors the mock must share
//! with the real API for integration tests to be trustworthy —
//! idempotent replay returns the stored original response (not a
//! re-derivation from live state), and account-scoped calls reject
//! gone (deleted/disconnected) accounts.

use api::stripe_service::{
    AuthorizationParams, CheckoutSessionParams, SessionMode, StripeCallError,
    StripeService,
};
use secrecy::SecretBox;

fn mock_service() -> StripeService {
    StripeService::new(
        SecretBox::new(Box::new("sk_test_mock".to_string())),
        SecretBox::new(Box::new("whsec_mock".to_string())),
        SecretBox::new(Box::new("whsec_connect_mock".to_string())),
    )
}

fn auth_params<'a>(
    account_id: &'a str,
    payment_method_id: &'a str,
    seed: &'a str,
) -> AuthorizationParams<'a> {
    AuthorizationParams {
        connected_account_id: account_id,
        platform_customer_id: "cus_user_mock_1",
        platform_payment_method_id: payment_method_id,
        amount_minor: 1000,
        currency: "usd",
        metadata: Default::default(),
        idempotency_seed: seed,
    }
}

/// A confirm that succeeded, then had its hold canceled out-of-band,
/// replays the stored original success under the same idempotency key —
/// Stripe's replay-by-key semantics. This is the production shape a
/// crashed-pre-commit retry observes (it activates a dead hold and
/// relies on webhook/reconciliation to converge), so the mock must not
/// fabricate a decline from the intent's live status.
#[tokio::test]
async fn replay_returns_original_success_after_out_of_band_cancel()
-> anyhow::Result<()> {
    let service = mock_service();
    let auth = service
        .create_authorization(auth_params("acct_mock_1", "pm_bob", "seed-1"))
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // The hold dies out-of-band (dashboard cancel).
    service
        .cancel_payment_intent("acct_mock_1", &auth.payment_intent_id, "k")
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let replay = service
        .create_authorization(auth_params("acct_mock_1", "pm_bob", "seed-1"))
        .await
        .map_err(|e| anyhow::anyhow!("replay must succeed: {e}"))?;
    assert_eq!(replay.payment_intent_id, auth.payment_intent_id);
    assert_eq!(replay.amount_minor, auth.amount_minor);
    Ok(())
}

/// A replayed decline returns the original decline verbatim, keeping
/// the issuer decline code — even if the card would now be accepted.
#[tokio::test]
async fn replay_preserves_original_decline_shape() -> anyhow::Result<()> {
    let service = mock_service();
    service
        .mock_declining_payment_methods
        .lock()
        .unwrap()
        .insert("pm_bob".to_string(), "insufficient_funds".to_string());

    let original = match service
        .create_authorization(auth_params("acct_mock_1", "pm_bob", "seed-1"))
        .await
    {
        Err(StripeCallError::Declined(info)) => info,
        other => anyhow::bail!("expected a decline, got {other:?}"),
    };
    assert_eq!(original.decline_code.as_deref(), Some("insufficient_funds"));

    // The card recovers; the replay still returns the stored decline.
    service
        .mock_declining_payment_methods
        .lock()
        .unwrap()
        .clear();
    let replay = match service
        .create_authorization(auth_params("acct_mock_1", "pm_bob", "seed-1"))
        .await
    {
        Err(StripeCallError::Declined(info)) => info,
        other => anyhow::bail!("expected the replayed decline, got {other:?}"),
    };
    assert_eq!(replay.code, original.code);
    assert_eq!(replay.decline_code.as_deref(), Some("insufficient_funds"));
    assert_eq!(replay.payment_intent_id, original.payment_intent_id);

    // A fresh seed is a fresh attempt: the recovered card succeeds.
    service
        .create_authorization(auth_params("acct_mock_1", "pm_bob", "seed-2"))
        .await
        .map_err(|e| anyhow::anyhow!("fresh seed must succeed: {e}"))?;
    Ok(())
}

/// Every account-scoped operation rejects a gone account like the real
/// API — intent ops with `AccountGone`, session and read ops with the
/// access-revoked error — instead of silently succeeding against an
/// account production could no longer reach.
#[tokio::test]
async fn gone_account_rejects_account_scoped_ops() -> anyhow::Result<()> {
    let service = mock_service();

    // Seed a live intent and session first, then lose the account.
    let auth = service
        .create_authorization(auth_params("acct_gone", "pm_bob", "seed-1"))
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let purchase_id = payloads::CreditPurchaseId(uuid::Uuid::from_u128(0x1234));
    let session_params = |account_id| CheckoutSessionParams {
        account_id,
        mode: SessionMode::Purchase {
            purchase_id: &purchase_id,
            application_fee_minor: Some(25),
        },
        amount_minor: 2500,
        currency: "usd",
        description: "Credits",
        success_url: "https://example.invalid/success",
        cancel_url: "https://example.invalid/cancel",
    };
    let session = service
        .create_payment_checkout_session(session_params("acct_gone"))
        .await?;
    service
        .mock_gone_accounts
        .lock()
        .unwrap()
        .insert("acct_gone".to_string());

    let pi = &auth.payment_intent_id;
    assert!(matches!(
        service
            .create_authorization(auth_params("acct_gone", "pm_bob", "seed-3"))
            .await,
        Err(StripeCallError::AccountGone(_))
    ));
    assert!(matches!(
        service.cancel_payment_intent("acct_gone", pi, "k").await,
        Err(StripeCallError::AccountGone(_))
    ));
    assert!(matches!(
        service
            .capture_payment_intent("acct_gone", pi, 1000, None, "k")
            .await,
        Err(StripeCallError::AccountGone(_))
    ));

    let gone_message = |e: anyhow::Error| {
        assert!(
            format!("{e:#}").contains("does not have access"),
            "expected the access-revoked message, got: {e:#}"
        );
    };
    gone_message(
        service
            .retrieve_payment_intent("acct_gone", pi)
            .await
            .expect_err("retrieve must error, not read as missing"),
    );
    gone_message(
        service
            .find_payment_intent_by_metadata("acct_gone", "k", "v", 0, 1)
            .await
            .expect_err("find must error"),
    );
    gone_message(
        service
            .create_payment_checkout_session(session_params("acct_gone"))
            .await
            .expect_err("session creation must error"),
    );
    gone_message(
        service
            .expire_checkout_session("acct_gone", &session.session_id)
            .await
            .expect_err("expire must error"),
    );
    gone_message(
        service
            .retrieve_checkout_session("acct_gone", &session.session_id)
            .await
            .expect_err("session retrieve must error"),
    );
    Ok(())
}
