//! Stripe integration service for subscription billing.
//!
//! The actual Stripe API calls live in [`real`], which is compiled
//! unconditionally so the env-gated sandbox tests (tests/api/stripe_sandbox.rs)
//! can exercise them against Stripe test mode even though the test build mocks
//! `StripeService`. The service methods are cfg-paired: real builds delegate
//! to [`real`], mock-stripe builds return canned values and record calls.

use anyhow::{Context, Result};
#[cfg(not(feature = "mock-stripe"))]
use secrecy::ExposeSecret;
use secrecy::SecretBox;

pub struct StripeService {
    #[cfg(not(feature = "mock-stripe"))]
    client: stripe::Client,
    #[allow(dead_code)]
    webhook_secret: SecretBox<String>,
    /// Mock: maps stripe customer IDs to community IDs.
    /// Tests populate this before sending webhook events.
    #[cfg(feature = "mock-stripe")]
    pub mock_customer_communities: std::sync::Mutex<
        std::collections::HashMap<String, payloads::CommunityId>,
    >,
    /// Mock: records subscription IDs that were canceled.
    #[cfg(feature = "mock-stripe")]
    pub mock_canceled_subscriptions: std::sync::Mutex<Vec<String>>,
}

impl StripeService {
    #[cfg(not(feature = "mock-stripe"))]
    pub fn new(
        api_key: SecretBox<String>,
        webhook_secret: SecretBox<String>,
    ) -> Self {
        let client = stripe::Client::new(api_key.expose_secret());
        Self {
            client,
            webhook_secret,
        }
    }

    #[cfg(feature = "mock-stripe")]
    pub fn new(
        _api_key: SecretBox<String>,
        webhook_secret: SecretBox<String>,
    ) -> Self {
        Self {
            webhook_secret,
            mock_customer_communities: std::sync::Mutex::new(
                std::collections::HashMap::new(),
            ),
            mock_canceled_subscriptions: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Create a Stripe customer for a community.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn create_customer(
        &self,
        community_name: &str,
        community_id: &payloads::CommunityId,
    ) -> Result<stripe_shared::CustomerId> {
        real::create_customer(&self.client, community_name, community_id).await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn create_customer(
        &self,
        _community_name: &str,
        _community_id: &payloads::CommunityId,
    ) -> Result<stripe_shared::CustomerId> {
        Ok("cus_mock_test123".into())
    }

    /// Create a Stripe Checkout Session for a subscription.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn create_checkout_session(
        &self,
        customer_id: &stripe_shared::CustomerId,
        price_id: &str,
        community_id: &payloads::CommunityId,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<String> {
        real::create_checkout_session(
            &self.client,
            customer_id,
            price_id,
            community_id,
            success_url,
            cancel_url,
        )
        .await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn create_checkout_session(
        &self,
        _customer_id: &stripe_shared::CustomerId,
        _price_id: &str,
        _community_id: &payloads::CommunityId,
        _success_url: &str,
        _cancel_url: &str,
    ) -> Result<String> {
        Ok("https://checkout.stripe.com/test/mock_session".to_string())
    }

    /// Create a Stripe Billing Portal session.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn create_portal_session(
        &self,
        customer_id: &str,
        return_url: &str,
    ) -> Result<String> {
        real::create_portal_session(&self.client, customer_id, return_url).await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn create_portal_session(
        &self,
        _customer_id: &str,
        _return_url: &str,
    ) -> Result<String> {
        Ok("https://billing.stripe.com/test/mock_portal".to_string())
    }

    /// Fetch the community_id from a Stripe customer's
    /// metadata. Used by webhook handlers that receive a
    /// customer ID but need the community association.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn get_customer_community_id(
        &self,
        customer_id: &str,
    ) -> Result<payloads::CommunityId> {
        real::get_customer_community_id(&self.client, customer_id).await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn get_customer_community_id(
        &self,
        customer_id: &str,
    ) -> Result<payloads::CommunityId> {
        let map = self.mock_customer_communities.lock().unwrap();
        map.get(customer_id).copied().ok_or_else(|| {
            anyhow::anyhow!("Mock: no community_id for customer {customer_id}")
        })
    }

    /// Cancel a Stripe subscription immediately.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn cancel_subscription(
        &self,
        subscription_id: &str,
    ) -> Result<()> {
        real::cancel_subscription(&self.client, subscription_id).await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn cancel_subscription(
        &self,
        subscription_id: &str,
    ) -> Result<()> {
        self.mock_canceled_subscriptions
            .lock()
            .unwrap()
            .push(subscription_id.to_string());
        Ok(())
    }

    /// Verify a Stripe webhook signature and return the raw
    /// JSON payload. We parse the JSON ourselves rather than
    /// relying on async-stripe's Event deserialization, which
    /// is tightly coupled to a specific Stripe API version.
    #[cfg(not(feature = "mock-stripe"))]
    pub fn verify_webhook(
        &self,
        payload: &str,
        signature: &str,
        time_source: &crate::time::TimeSource,
    ) -> Result<serde_json::Value> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        // Parse "t=<timestamp>,v1=<sig>" from header
        let mut timestamp = None;
        let mut v1_sig = None;
        for part in signature.split(',') {
            let part = part.trim();
            if let Some(t) = part.strip_prefix("t=") {
                timestamp = Some(t);
            } else if let Some(v) = part.strip_prefix("v1=") {
                v1_sig = Some(v);
            }
        }
        let timestamp: i64 = timestamp
            .ok_or_else(|| anyhow::anyhow!("Missing timestamp in signature"))?
            .parse()
            .context("Invalid timestamp")?;
        let v1_sig =
            v1_sig.ok_or_else(|| anyhow::anyhow!("Missing v1 in signature"))?;

        // Verify HMAC-SHA256
        let signed_payload = format!("{}.{}", timestamp, payload);
        let secret = self.webhook_secret.expose_secret().as_bytes();
        let mut mac = Hmac::<Sha256>::new_from_slice(secret)
            .context("Invalid webhook secret")?;
        mac.update(signed_payload.as_bytes());
        let expected = hex::decode(v1_sig).context("Invalid hex in v1")?;
        mac.verify_slice(&expected)
            .map_err(|_| anyhow::anyhow!("Bad signature"))?;

        // Check timestamp freshness (5 minute tolerance)
        let now = time_source.now().as_second();
        if (now - timestamp).abs() > 300 {
            anyhow::bail!("Webhook timestamp too old: {timestamp}");
        }

        serde_json::from_str(payload).context("Invalid JSON in webhook payload")
    }

    #[cfg(feature = "mock-stripe")]
    pub fn verify_webhook(
        &self,
        payload: &str,
        _signature: &str,
        _time_source: &crate::time::TimeSource,
    ) -> Result<serde_json::Value> {
        serde_json::from_str(payload).context("Failed to parse webhook payload")
    }
}

/// The real Stripe API calls, as plain functions over a
/// `stripe::Client` so they compile (and are testable against the
/// Stripe sandbox) regardless of the mock-stripe feature.
pub mod real {
    use std::collections::HashMap;

    use anyhow::{Context, Result};

    /// Create a Stripe customer for a community.
    pub async fn create_customer(
        client: &stripe::Client,
        community_name: &str,
        community_id: &payloads::CommunityId,
    ) -> Result<stripe_shared::CustomerId> {
        let metadata = HashMap::from([(
            "community_id".to_string(),
            community_id.to_string(),
        )]);

        let customer = stripe_core::customer::CreateCustomer::new()
            .name(community_name)
            .metadata(metadata)
            .send(client)
            .await
            .context("Failed to create Stripe customer")?;

        tracing::info!(
            customer_id = %customer.id,
            %community_id,
            "Stripe customer created"
        );
        Ok(customer.id)
    }

    /// Create a Stripe Checkout Session for a subscription.
    pub async fn create_checkout_session(
        client: &stripe::Client,
        customer_id: &stripe_shared::CustomerId,
        price_id: &str,
        community_id: &payloads::CommunityId,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<String> {
        use stripe_checkout::checkout_session::{
            CreateCheckoutSession, CreateCheckoutSessionLineItems,
        };

        let metadata = HashMap::from([(
            "community_id".to_string(),
            community_id.to_string(),
        )]);

        let session = CreateCheckoutSession::new()
            .customer(customer_id.as_str())
            .mode(stripe_shared::CheckoutSessionMode::Subscription)
            .success_url(success_url)
            .cancel_url(cancel_url)
            .metadata(metadata)
            .line_items(vec![CreateCheckoutSessionLineItems {
                price: Some(price_id.to_string()),
                quantity: Some(1),
                ..Default::default()
            }])
            .send(client)
            .await
            .context("Failed to create Checkout session")?;

        let url = session
            .url
            .ok_or_else(|| anyhow::anyhow!("Checkout session has no URL"))?;

        tracing::info!(
            session_id = %session.id,
            %community_id,
            "Checkout session created"
        );
        Ok(url)
    }

    /// Create a Stripe Billing Portal session.
    pub async fn create_portal_session(
        client: &stripe::Client,
        customer_id: &str,
        return_url: &str,
    ) -> Result<String> {
        use stripe_billing::billing_portal_session::CreateBillingPortalSession;

        let session = CreateBillingPortalSession::new()
            .customer(customer_id)
            .return_url(return_url)
            .send(client)
            .await
            .context("Failed to create portal session")?;

        tracing::info!("Portal session created");
        Ok(session.url)
    }

    /// Fetch the community_id from a Stripe customer's metadata.
    pub async fn get_customer_community_id(
        client: &stripe::Client,
        customer_id: &str,
    ) -> Result<payloads::CommunityId> {
        use stripe_core::customer::{
            RetrieveCustomer, RetrieveCustomerReturned,
        };

        let customer = RetrieveCustomer::new(customer_id)
            .send(client)
            .await
            .context("Failed to retrieve Stripe customer")?;
        let customer = match customer {
            RetrieveCustomerReturned::Customer(c) => c,
            RetrieveCustomerReturned::DeletedCustomer(_) => {
                anyhow::bail!("Customer {customer_id} is deleted")
            }
        };
        let community_id_str = customer
            .metadata
            .as_ref()
            .and_then(|m| m.get("community_id"))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Customer {customer_id} missing community_id in metadata"
                )
            })?;
        community_id_str
            .parse()
            .context("Invalid community_id in customer metadata")
    }

    /// Cancel a Stripe subscription immediately.
    pub async fn cancel_subscription(
        client: &stripe::Client,
        subscription_id: &str,
    ) -> Result<()> {
        stripe_billing::subscription::CancelSubscription::new(subscription_id)
            .send(client)
            .await
            .context("Failed to cancel Stripe subscription")?;
        tracing::info!(
            %subscription_id,
            "Stripe subscription canceled"
        );
        Ok(())
    }
}
