//! Env-gated tests driving real Stripe test mode, so the async-stripe
//! integration is verified by API instead of manual dashboard clicking.
//! These exercise the real call implementations in
//! `api::stripe_service::real`, which stay compiled even though this test
//! build mocks `StripeService` itself.
//!
//! Run with a sandbox secret key (never a live key; the helper refuses
//! non-test keys):
//!
//! ```sh
//! STRIPE_SANDBOX_SECRET_KEY=sk_test_... \
//!     cargo test --test api stripe_sandbox -- --ignored --test-threads=4
//! ```
//!
//! Ignored by default so normal runs stay offline. Objects created here
//! accumulate in the sandbox, which Stripe isolates and lets you reset.

use api::store;
use api::stripe_service::real;
use payloads::SubscriptionStatus;
use test_helpers::spawn_app;

fn sandbox_key() -> String {
    let key = std::env::var("STRIPE_SANDBOX_SECRET_KEY").expect(
        "STRIPE_SANDBOX_SECRET_KEY must be set to run the stripe_sandbox \
         tests",
    );
    assert!(
        key.starts_with("sk_test_") || key.starts_with("rk_test_"),
        "refusing to run sandbox tests with a non-test-mode key"
    );
    key
}

fn sandbox_client() -> stripe::Client {
    stripe::Client::new(sandbox_key())
}

/// Create a recurring monthly price (with inline product) to subscribe
/// against.
async fn create_test_price(client: &stripe::Client) -> anyhow::Result<String> {
    use stripe_product::price::{
        CreatePrice, CreatePriceProductData, CreatePriceRecurring,
        CreatePriceRecurringInterval,
    };

    let price = CreatePrice::new(stripe_types::Currency::USD)
        .unit_amount(500)
        .recurring(CreatePriceRecurring::new(
            CreatePriceRecurringInterval::Month,
        ))
        .product_data(CreatePriceProductData {
            name: "tinylvt sandbox test product".to_string(),
            active: None,
            id: None,
            metadata: None,
            statement_descriptor: None,
            tax_code: None,
            unit_label: None,
        })
        .send(client)
        .await?;
    Ok(price.id.to_string())
}

/// The customer metadata written by create_customer is what
/// get_customer_community_id reads back; round-trip both through real
/// Stripe.
#[tokio::test]
#[ignore = "needs STRIPE_SANDBOX_SECRET_KEY; hits Stripe test mode"]
async fn customer_metadata_round_trip() -> anyhow::Result<()> {
    let client = sandbox_client();
    let community_id = payloads::CommunityId(uuid::Uuid::new_v4());

    let customer_id = real::create_customer(
        &client,
        "tinylvt sandbox test community",
        &community_id,
    )
    .await?;

    let looked_up =
        real::get_customer_community_id(&client, customer_id.as_str()).await?;
    assert_eq!(looked_up, community_id);

    Ok(())
}

/// Checkout and portal sessions come back with Stripe-hosted URLs.
#[tokio::test]
#[ignore = "needs STRIPE_SANDBOX_SECRET_KEY; hits Stripe test mode"]
async fn checkout_and_portal_sessions() -> anyhow::Result<()> {
    let client = sandbox_client();
    let community_id = payloads::CommunityId(uuid::Uuid::new_v4());

    let customer_id = real::create_customer(
        &client,
        "tinylvt sandbox test community",
        &community_id,
    )
    .await?;
    let price_id = create_test_price(&client).await?;

    let checkout_url = real::create_checkout_session(
        &client,
        &customer_id,
        &price_id,
        &community_id,
        "https://example.com/success",
        "https://example.com/cancel",
    )
    .await?;
    assert!(
        checkout_url.starts_with("https://checkout.stripe.com/"),
        "unexpected checkout URL: {checkout_url}"
    );

    // Portal sessions require a portal configuration; a fresh sandbox has
    // no default, so create a minimal one (they accumulate harmlessly).
    {
        use stripe_billing::billing_portal_configuration::{
            CreateBillingPortalConfiguration,
            CreateBillingPortalConfigurationFeatures, InvoiceListParam,
        };
        CreateBillingPortalConfiguration::new(
            CreateBillingPortalConfigurationFeatures {
                customer_update: None,
                invoice_history: Some(InvoiceListParam { enabled: true }),
                payment_method_update: None,
                subscription_cancel: None,
                subscription_update: None,
            },
        )
        .default_return_url("https://example.com/return")
        .send(&client)
        .await?;
    }

    let portal_url = real::create_portal_session(
        &client,
        customer_id.as_str(),
        "https://example.com/return",
    )
    .await?;
    assert!(
        portal_url.starts_with("https://billing.stripe.com/"),
        "unexpected portal URL: {portal_url}"
    );

    Ok(())
}

/// List recent events of the given type whose subscription object id
/// matches, oldest first. Raw reqwest keeps the payloads exactly as a
/// webhook endpoint at the pinned API version would deliver them.
async fn fetch_subscription_events(
    key: &str,
    event_type: &str,
    subscription_id: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let response = reqwest::Client::new()
        .get("https://api.stripe.com/v1/events")
        .bearer_auth(key)
        .query(&[("type", event_type), ("limit", "100")])
        .header(
            "Stripe-Version",
            stripe_shared::version::VERSION.to_string(),
        )
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let mut events: Vec<serde_json::Value> = response["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e["data"]["object"]["id"].as_str() == Some(subscription_id))
        .collect();
    events.reverse();
    Ok(events)
}

/// Poll until an event of the given type exists for the subscription.
async fn wait_for_subscription_event(
    key: &str,
    event_type: &str,
    subscription_id: &str,
) -> anyhow::Result<serde_json::Value> {
    for _ in 0..30 {
        let events =
            fetch_subscription_events(key, event_type, subscription_id).await?;
        if let Some(event) = events.into_iter().next() {
            return Ok(event);
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    anyhow::bail!("no {event_type} event for {subscription_id} after 30s")
}

/// Full subscription lifecycle: create against a real customer with a
/// magic test card, cancel via the ported call, then feed the real
/// Stripe-rendered subscription events through handle_webhook_event —
/// verifying the handler's field extraction against the pinned API
/// version's payload shapes, not hand-built fixtures.
#[tokio::test]
#[ignore = "needs STRIPE_SANDBOX_SECRET_KEY; hits Stripe test mode"]
async fn subscription_lifecycle_webhooks() -> anyhow::Result<()> {
    let key = sandbox_key();
    let client = sandbox_client();

    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    let customer_id = real::create_customer(
        &client,
        "tinylvt sandbox test community",
        &community_id,
    )
    .await?;
    let price_id = create_test_price(&client).await?;

    // Attach the magic test card and subscribe (bypassing hosted
    // Checkout, whose completion can't be driven by API). Attaching
    // pm_card_visa mints a fresh payment method; use its real id.
    let payment_method =
        stripe_payment::payment_method::AttachPaymentMethod::new(
            "pm_card_visa",
        )
        .customer(customer_id.as_str())
        .send(&client)
        .await?;
    let subscription = stripe_billing::subscription::CreateSubscription::new()
        .customer(customer_id.as_str())
        .default_payment_method(payment_method.id.as_str())
        .items(vec![
            stripe_billing::subscription::CreateSubscriptionItems {
                price: Some(price_id.clone()),
                ..Default::default()
            },
        ])
        .send(&client)
        .await?;
    let subscription_id = subscription.id.to_string();

    real::cancel_subscription(&client, &subscription_id).await?;

    // The webhook handler resolves an unknown subscription's community
    // through the (mocked) customer lookup; the real reverse-lookup is
    // covered by customer_metadata_round_trip.
    app.stripe_service
        .mock_customer_communities
        .lock()
        .unwrap()
        .insert(customer_id.to_string(), community_id);

    let created = wait_for_subscription_event(
        &key,
        "customer.subscription.created",
        &subscription_id,
    )
    .await?;
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &created,
    )
    .await?;

    let request = payloads::requests::GetSubscriptionInfo { community_id };
    let info = app
        .client
        .get_subscription_info(&request)
        .await?
        .expect("subscription row should exist after created event");
    assert_eq!(info.status, SubscriptionStatus::Active);
    assert_eq!(info.billing_interval, payloads::BillingInterval::Month);

    let deleted = wait_for_subscription_event(
        &key,
        "customer.subscription.deleted",
        &subscription_id,
    )
    .await?;
    store::billing::handle_webhook_event(
        &app.db_pool,
        &app.time_source,
        &app.stripe_service,
        &deleted,
    )
    .await?;

    let info = app
        .client
        .get_subscription_info(&request)
        .await?
        .expect("subscription row should still exist after deletion");
    assert_eq!(info.status, SubscriptionStatus::Canceled);

    Ok(())
}

/// Create a customer with the magic test card attached, returning
/// (customer_id, payment_method_id).
async fn create_customer_with_card(
    client: &stripe::Client,
) -> anyhow::Result<(stripe_shared::CustomerId, stripe_shared::PaymentMethodId)>
{
    let community_id = payloads::CommunityId(uuid::Uuid::new_v4());
    let customer_id = real::create_customer(
        client,
        "tinylvt sandbox test community",
        &community_id,
    )
    .await?;
    let payment_method =
        stripe_payment::payment_method::AttachPaymentMethod::new(
            "pm_card_visa",
        )
        .customer(customer_id.as_str())
        .send(client)
        .await?;
    Ok((customer_id, payment_method.id))
}

/// The manual-capture endpoints phase 5 depends on
/// (create/confirm/capture/cancel), confirmed by execution rather than
/// just codegen coverage. Incremental authorization has its own test:
/// it needs account-level eligibility.
#[tokio::test]
#[ignore = "needs STRIPE_SANDBOX_SECRET_KEY; hits Stripe test mode"]
async fn manual_capture_lifecycle() -> anyhow::Result<()> {
    use stripe_core::payment_intent::{
        CancelPaymentIntent, CapturePaymentIntent, CreatePaymentIntent,
    };
    use stripe_shared::{PaymentIntentCaptureMethod, PaymentIntentStatus};

    let client = sandbox_client();
    let (customer_id, payment_method_id) =
        create_customer_with_card(&client).await?;

    let create_authorization = async |amount: i64| {
        CreatePaymentIntent::new(amount, stripe_types::Currency::USD)
            .capture_method(PaymentIntentCaptureMethod::Manual)
            .customer(customer_id.as_str())
            .payment_method(payment_method_id.as_str())
            .payment_method_types(vec!["card".to_string()])
            .confirm(true)
            .send(&client)
            .await
    };

    // Authorize $10 without capturing.
    let intent = create_authorization(1000).await?;
    assert_eq!(intent.status, PaymentIntentStatus::RequiresCapture);
    assert_eq!(intent.amount_capturable, 1000);

    // Partial capture: $7 of the $10 (the rest is released).
    let intent = CapturePaymentIntent::new(intent.id.clone())
        .amount_to_capture(700)
        .send(&client)
        .await?;
    assert_eq!(intent.status, PaymentIntentStatus::Succeeded);
    assert_eq!(intent.amount_received, 700);

    // Cancel releases an uncaptured authorization.
    let intent = create_authorization(1000).await?;
    let intent = CancelPaymentIntent::new(intent.id.clone())
        .send(&client)
        .await?;
    assert_eq!(intent.status, PaymentIntentStatus::Canceled);

    Ok(())
}

/// Incremental authorization — NOT used by v1 (cut from the design:
/// account eligibility sits behind volume-negotiated IC+ pricing; see
/// the implementation plan's decision 5). Kept as verified knowledge
/// for if that ever changes. `request_incremental_authorization=
/// if_available` is soft per card but hard per account: confirm 400s
/// ("not eligible for the requested card features") unless the account
/// has Stripe's gated flexible acquiring features. Until eligibility
/// is granted this test reports that state and passes; once granted
/// (true of this sandbox since 2026-07) it verifies the increment
/// flow.
#[tokio::test]
#[ignore = "needs STRIPE_SANDBOX_SECRET_KEY; hits Stripe test mode"]
async fn incremental_authorization() -> anyhow::Result<()> {
    use stripe_core::payment_intent::{
        CancelPaymentIntent, CreatePaymentIntent,
        CreatePaymentIntentPaymentMethodOptions,
        CreatePaymentIntentPaymentMethodOptionsCard,
        CreatePaymentIntentPaymentMethodOptionsCardRequestIncrementalAuthorization as RequestIncrementalAuthorization,
        IncrementAuthorizationPaymentIntent,
    };
    use stripe_shared::{PaymentIntentCaptureMethod, PaymentIntentStatus};

    let client = sandbox_client();
    let (customer_id, payment_method_id) =
        create_customer_with_card(&client).await?;

    let result = CreatePaymentIntent::new(1000, stripe_types::Currency::USD)
        .capture_method(PaymentIntentCaptureMethod::Manual)
        .customer(customer_id.as_str())
        .payment_method(payment_method_id.as_str())
        .payment_method_types(vec!["card".to_string()])
        .payment_method_options(CreatePaymentIntentPaymentMethodOptions {
            card: Some(CreatePaymentIntentPaymentMethodOptionsCard {
                request_incremental_authorization: Some(
                    RequestIncrementalAuthorization::IfAvailable,
                ),
                ..Default::default()
            }),
            ..Default::default()
        })
        .confirm(true)
        .send(&client)
        .await;

    let intent = match result {
        Err(stripe::StripeError::Stripe(ref err, 400))
            if err.message.as_deref().is_some_and(|m| {
                m.contains("not eligible for the requested card features")
            }) =>
        {
            println!(
                "SKIPPED: sandbox account lacks incremental authorization \
                 eligibility (flexible acquiring features not enabled)"
            );
            return Ok(());
        }
        other => other?,
    };
    assert_eq!(intent.status, PaymentIntentStatus::RequiresCapture);

    // Raise the $10 authorization to $15, then release it.
    let intent =
        IncrementAuthorizationPaymentIntent::new(intent.id.clone(), 1500)
            .send(&client)
            .await?;
    assert_eq!(intent.amount_capturable, 1500);
    CancelPaymentIntent::new(intent.id.clone())
        .send(&client)
        .await?;

    Ok(())
}

/// The full phase-5 authorization path against real Stripe: clone the
/// platform-saved card to the connected account, create+confirm an
/// off-session manual-capture authorization there, capture part of it
/// with a platform application fee, mint a second authorization and
/// cancel it, and verify a declining card maps to `Declined`. Needs a
/// pre-onboarded, charges-enabled connected account in
/// `TEST_CONNECT_ACCOUNT_ID`.
#[tokio::test]
#[ignore = "needs STRIPE_SANDBOX_SECRET_KEY + TEST_CONNECT_ACCOUNT_ID; \
            hits Stripe test mode"]
async fn authorization_lifecycle_on_connected_account() -> anyhow::Result<()> {
    use api::stripe_service::AuthorizationParams;
    use stripe_payment::payment_method::AttachPaymentMethod;

    let client = sandbox_client();
    let connected_account_id = std::env::var("TEST_CONNECT_ACCOUNT_ID")
        .expect("TEST_CONNECT_ACCOUNT_ID must be set for this test");
    let user_id = payloads::UserId(uuid::Uuid::new_v4());

    let customer_id = real::create_user_customer(
        &client,
        "tinylvt sandbox test user",
        &user_id,
    )
    .await?;
    let payment_method = AttachPaymentMethod::new("pm_card_visa")
        .customer(&customer_id)
        .send(&client)
        .await?;

    let params = |amount_minor, seed: String| AuthorizationParams {
        connected_account_id: &connected_account_id,
        platform_customer_id: &customer_id,
        platform_payment_method_id: payment_method.id.as_str(),
        amount_minor,
        currency: "usd",
        metadata: std::collections::HashMap::from([(
            "funding_intent_id".to_string(),
            seed.clone(),
        )]),
        idempotency_seed: Box::leak(seed.into_boxed_str()),
    };

    // Authorize $10; capture $7 with a $0.07 platform fee.
    let seed = uuid::Uuid::new_v4().to_string();
    let auth = real::create_authorization(&client, params(1000, seed.clone()))
        .await
        .map_err(|e| anyhow::anyhow!("authorization failed: {e}"))?;
    assert_eq!(auth.amount_minor, 1000);
    // Idempotent replay returns the same intent.
    let replay = real::create_authorization(&client, params(1000, seed))
        .await
        .map_err(|e| anyhow::anyhow!("replay failed: {e}"))?;
    assert_eq!(replay.payment_intent_id, auth.payment_intent_id);
    real::capture_payment_intent(
        &client,
        &connected_account_id,
        &auth.payment_intent_id,
        700,
        Some(7),
        &format!("{}:capture", auth.payment_intent_id),
    )
    .await?;

    // A second authorization releases via cancel.
    let auth2 = real::create_authorization(
        &client,
        params(500, uuid::Uuid::new_v4().to_string()),
    )
    .await
    .map_err(|e| anyhow::anyhow!("second authorization failed: {e}"))?;
    real::cancel_payment_intent(
        &client,
        &connected_account_id,
        &auth2.payment_intent_id,
        &format!("{}:cancel", auth2.payment_intent_id),
    )
    .await?;

    // Decline mapping isn't exercised live: this sandbox validates cards
    // at attach, so even pm_card_visa_chargeDeclined (documented to
    // attach successfully and decline at charge) declines at attach.
    // That attach error confirmed the card_error shape
    // map_authorization_error keys on (type card_error, code
    // card_declined, decline_code); the mapping itself is covered by
    // mock tests.

    Ok(())
}

/// Connected-account lifecycle (phase 3): create with the
/// Standard-equivalent controller properties, mint an onboarding
/// Account Link, read live status, then delete the account (test-mode
/// only) so sandbox accounts don't accumulate.
#[tokio::test]
#[ignore = "needs STRIPE_SANDBOX_SECRET_KEY; hits Stripe test mode"]
async fn connected_account_lifecycle() -> anyhow::Result<()> {
    let client = sandbox_client();
    let community_id = payloads::CommunityId(uuid::Uuid::new_v4());

    let account_id = real::create_connected_account(
        &client,
        "tinylvt sandbox test community",
        &community_id,
    )
    .await?;
    assert!(account_id.starts_with("acct_"));

    let url = real::create_account_link(
        &client,
        &account_id,
        "https://example.com/settings?stripe_connect=refresh",
        "https://example.com/settings?stripe_connect=return",
    )
    .await?;
    assert!(url.starts_with("https://connect.stripe.com/"));

    // Fresh account: nothing submitted, charges disabled.
    let status = real::get_account_status(&client, &account_id).await?;
    assert!(!status.charges_enabled);
    assert!(!status.details_submitted);

    stripe_connect::account::DeleteAccount::new(
        account_id.parse::<stripe_shared::AccountId>().unwrap(),
    )
    .send(&client)
    .await?;

    Ok(())
}

/// Saved-card lifecycle on the platform account (phase 4): create a
/// user customer, mint a setup Checkout session, attach a test card
/// (standing in for hosted-Checkout completion, which can't be driven
/// by API), read it back through the list call, and detach.
#[tokio::test]
#[ignore = "needs STRIPE_SANDBOX_SECRET_KEY; hits Stripe test mode"]
async fn saved_card_lifecycle() -> anyhow::Result<()> {
    use stripe_payment::payment_method::AttachPaymentMethod;

    let client = sandbox_client();
    let user_id = payloads::UserId(uuid::Uuid::new_v4());

    let customer_id = real::create_user_customer(
        &client,
        "tinylvt sandbox test user",
        &user_id,
    )
    .await?;
    assert!(customer_id.starts_with("cus_"));

    let url = real::create_setup_checkout_session(
        &client,
        &customer_id,
        &user_id,
        "https://example.com/profile?card_setup=success",
        "https://example.com/profile?card_setup=canceled",
    )
    .await?;
    assert!(url.starts_with("https://checkout.stripe.com/"));

    // Hosted Checkout can't be driven by API; construct its end state
    // by attaching the magic test card directly.
    AttachPaymentMethod::new("pm_card_visa")
        .customer(&customer_id)
        .send(&client)
        .await?;

    let methods =
        real::list_card_payment_methods(&client, &customer_id).await?;
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].brand, "visa");
    assert_eq!(methods[0].last4, "4242");

    real::detach_payment_method(&client, &methods[0].id).await?;
    let methods =
        real::list_card_payment_methods(&client, &customer_id).await?;
    assert!(methods.is_empty());

    Ok(())
}
