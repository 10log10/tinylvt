//! Stripe integration service for subscription billing.

use anyhow::{Context, Result};
use secrecy::SecretBox;
#[cfg(not(feature = "mock-stripe"))]
use {secrecy::ExposeSecret, std::collections::HashMap};

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
    ) -> Result<stripe::CustomerId> {
        let mut metadata = HashMap::new();
        metadata.insert("community_id".to_string(), community_id.to_string());

        let customer = stripe::Customer::create(
            &self.client,
            stripe::CreateCustomer {
                name: Some(community_name),
                metadata: Some(metadata),
                ..Default::default()
            },
        )
        .await
        .context("Failed to create Stripe customer")?;

        tracing::info!(
            customer_id = %customer.id,
            %community_id,
            "Stripe customer created"
        );
        Ok(customer.id)
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn create_customer(
        &self,
        _community_name: &str,
        _community_id: &payloads::CommunityId,
    ) -> Result<stripe::CustomerId> {
        Ok("cus_mock_test123".parse().unwrap())
    }

    /// Create a Stripe Checkout Session for a subscription.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn create_checkout_session(
        &self,
        customer_id: &stripe::CustomerId,
        price_id: &str,
        community_id: &payloads::CommunityId,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<String> {
        let mut metadata = HashMap::new();
        metadata.insert("community_id".to_string(), community_id.to_string());

        let mut params = stripe::CreateCheckoutSession::new();
        params.customer = Some(customer_id.clone());
        params.mode = Some(stripe::CheckoutSessionMode::Subscription);
        params.success_url = Some(success_url);
        params.cancel_url = Some(cancel_url);
        params.metadata = Some(metadata);
        params.line_items =
            Some(vec![stripe::CreateCheckoutSessionLineItems {
                price: Some(price_id.to_string()),
                quantity: Some(1),
                ..Default::default()
            }]);

        let session = stripe::CheckoutSession::create(&self.client, params)
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

    #[cfg(feature = "mock-stripe")]
    pub async fn create_checkout_session(
        &self,
        _customer_id: &stripe::CustomerId,
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
        let customer_id: stripe::CustomerId =
            customer_id.parse().context("Invalid customer ID")?;
        let mut params = stripe::CreateBillingPortalSession::new(customer_id);
        params.return_url = Some(return_url);

        let session =
            stripe::BillingPortalSession::create(&self.client, params)
                .await
                .context("Failed to create portal session")?;

        tracing::info!("Portal session created");
        Ok(session.url)
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
        let id: stripe::CustomerId =
            customer_id.parse().context("Invalid customer ID")?;
        let customer = stripe::Customer::retrieve(&self.client, &id, &[])
            .await
            .context("Failed to retrieve Stripe customer")?;
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
        let id: stripe::SubscriptionId =
            subscription_id.parse().context("Invalid subscription ID")?;
        let params = stripe::CancelSubscription::default();
        stripe::Subscription::cancel(&self.client, &id, params)
            .await
            .context("Failed to cancel Stripe subscription")?;
        tracing::info!(
            %subscription_id,
            "Stripe subscription canceled"
        );
        Ok(())
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
