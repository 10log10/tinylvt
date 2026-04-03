//! Billing types shared between backend and frontend.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Subscription tier for a community.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum SubscriptionTier {
    #[default]
    Free,
    Paid,
}

impl SubscriptionTier {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Free => "Free tier",
            Self::Paid => "Paid tier",
        }
    }
}

/// Subscription status for billing.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "use-sqlx",
    sqlx(type_name = "subscription_status", rename_all = "snake_case")
)]
pub enum SubscriptionStatus {
    #[default]
    Active,
    PastDue,
    Canceled,
    Unpaid,
}

/// Billing interval for subscriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "use-sqlx",
    sqlx(type_name = "billing_interval", rename_all = "snake_case")
)]
pub enum BillingInterval {
    Month,
    Year,
}

impl BillingInterval {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Month => "Monthly",
            Self::Year => "Annual",
        }
    }

    /// Price in cents (USD).
    pub fn price_cents(&self) -> i64 {
        match self {
            Self::Month => 500, // $5/month
            Self::Year => 5000, // $50/year
        }
    }
}

/// Storage limits for a subscription tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierLimits {
    pub storage_bytes: i64,
}

impl TierLimits {
    pub fn for_tier(tier: SubscriptionTier) -> Self {
        match tier {
            SubscriptionTier::Paid => TierLimits {
                storage_bytes: 2_000_000_000, // 2 GB
            },
            SubscriptionTier::Free => TierLimits {
                storage_bytes: 50_000_000, // 50 MB
            },
        }
    }
}

/// Storage usage breakdown for a community.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
pub struct StorageUsage {
    pub image_bytes: i64,
    pub member_bytes: i64,
    pub space_bytes: i64,
    pub auction_bytes: i64,
    pub transaction_bytes: i64,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "jiff_sqlx::Timestamp"))]
    pub calculated_at: Timestamp,
}

impl StorageUsage {
    pub fn total_bytes(&self) -> i64 {
        self.image_bytes
            + self.member_bytes
            + self.space_bytes
            + self.auction_bytes
            + self.transaction_bytes
    }
}

/// Storage usage with tier limits for API response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityStorageUsage {
    pub usage: StorageUsage,
    pub tier: SubscriptionTier,
    pub limits: TierLimits,
}

impl CommunityStorageUsage {
    /// Calculate usage as a percentage of the limit (0.0 to 100.0+).
    pub fn usage_percentage(&self) -> f64 {
        if self.limits.storage_bytes == 0 {
            return 0.0;
        }
        (self.usage.total_bytes() as f64 / self.limits.storage_bytes as f64)
            * 100.0
    }
}

/// Subscription details for API responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
pub struct SubscriptionInfo {
    pub status: SubscriptionStatus,
    pub billing_interval: BillingInterval,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "jiff_sqlx::Timestamp"))]
    pub current_period_end: Timestamp,
    pub cancel_at_period_end: bool,
}

/// Response from creating a Stripe Checkout session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSessionResponse {
    pub checkout_url: String,
}
