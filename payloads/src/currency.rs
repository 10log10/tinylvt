//! Stripe-backed auction funding types (backed_credits mode): the
//! real-currency denomination allow-list, platform fee derivation, and the
//! id/enum types mirrored from the funding_intents table.

use derive_more::Display;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
#[cfg(feature = "use-sqlx")]
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct FundingIntentId(pub Uuid);

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type, FromRow), sqlx(transparent))]
pub struct CreditPurchaseId(pub Uuid);

/// Lifecycle of a funding intent (a manual-capture Stripe PaymentIntent
/// backing auction bids). Non-terminal states are worker instructions or
/// in-flight markers; terminal states (Captured, Canceled, Expired, Failed)
/// are Stripe facts. See the funding_intents schema comments for the
/// transition graph.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type))]
#[cfg_attr(
    feature = "use-sqlx",
    sqlx(type_name = "funding_intent_status", rename_all = "snake_case")
)]
pub enum FundingIntentStatus {
    Pending,
    /// Unsaved-card flow: a Checkout session is minted and awaiting the
    /// member's payment; adoption promotes it to Authorized.
    CheckoutCreated,
    Authorized,
    CapturePending,
    Captured,
    ReleasePending,
    Superseded,
    Canceled,
    Expired,
    Failed,
}

impl FundingIntentStatus {
    /// Whether the intent has reached a Stripe fact no worker will act
    /// on further. Source of truth for the non-terminal SQL literal
    /// lists (worker selection, the wind-down guard).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Captured | Self::Canceled | Self::Expired | Self::Failed
        )
    }
}

/// Who initiated an authorization: scopes age-cancel notifications (system
/// auths re-auth silently, member auths notify once) and labels hold history
/// ("automatic pre-authorization" vs "you authorized $50").
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type))]
#[cfg_attr(
    feature = "use-sqlx",
    sqlx(type_name = "funding_intent_origin", rename_all = "snake_case")
)]
pub enum FundingIntentOrigin {
    ScheduledPreauth,
    MemberPreauth,
    BidFlow,
    /// Unsaved-card Checkout authorization: member-present, nothing
    /// stored, no card-charge grant involved.
    MemberCheckout,
}

/// Whether top-up credit purchases are enabled. Off until Stripe's
/// stored-value sign-off; flipping it is a code change and redeploy,
/// which keeps the UI and API in agreement by construction. Debt
/// settlement (an exact-amount charge that never takes the balance
/// positive — payment for services rendered, not stored value) is
/// deliberately not gated by this.
pub const CREDIT_PURCHASES_ENABLED: bool = false;

/// What a credit purchase is for. Debt settlement is a permanently
/// distinct action, not a pre-approval artifact: its amount is
/// validated to exactly clear the member's effective debt (balance plus
/// pending captures), so members who don't want to store value can
/// settle without buying credits.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type))]
#[cfg_attr(
    feature = "use-sqlx",
    sqlx(type_name = "purchase_kind", rename_all = "snake_case")
)]
pub enum PurchaseKind {
    TopUp,
    DebtSettlement,
}

/// Lifecycle of a credit purchase's Checkout session. `Created` means
/// the session was minted but payment hasn't completed (abandoned
/// sessions become `Expired`); `Processing` means a delayed method
/// (e.g. ACH) is settling — shown as pending, credits not yet issued.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(Type))]
#[cfg_attr(
    feature = "use-sqlx",
    sqlx(type_name = "purchase_status", rename_all = "snake_case")
)]
pub enum PurchaseStatus {
    Created,
    Processing,
    Succeeded,
    Failed,
    Expired,
}

impl PurchaseStatus {
    /// All variants, for deriving status lists.
    pub const ALL: [Self; 5] = [
        Self::Created,
        Self::Processing,
        Self::Succeeded,
        Self::Failed,
        Self::Expired,
    ];

    /// Whether the purchase's workflow is finished. Source of truth for
    /// the non-terminal status lists ([`Self::non_terminal`]).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Expired)
    }

    /// The non-terminal statuses, derived from [`Self::is_terminal`].
    /// Bind with `status = ANY($n)` instead of hand-kept SQL literal
    /// lists so the lists can't drift from the enum.
    pub fn non_terminal() -> Vec<Self> {
        Self::ALL.into_iter().filter(|s| !s.is_terminal()).collect()
    }
}

/// A community's Stripe Connect standing, for the settings UI.
/// `OnboardingIncomplete` covers both never-finished onboarding and
/// outstanding requirements (details submitted but charges still
/// disabled) — both resolve through a fresh onboarding link.
/// `Disconnected` means the community revoked the platform's access from
/// their Stripe dashboard; reconnecting goes through the connect flow
/// again.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize,
)]
pub enum StripeConnectStatus {
    NotConnected,
    OnboardingIncomplete,
    ChargesEnabled,
    Disconnected,
}

/// A real-currency denomination supported by the stripe-backed
/// backed_credits mode. Communities in this mode must use an allow-listed
/// denomination exactly (name = ISO code, symbol and minor units as listed);
/// cent-exact quantization is what keeps app amounts and Stripe charges equal
/// by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denomination {
    /// ISO 4217 code, used as the community's currency name.
    pub iso: &'static str,
    pub symbol: &'static str,
    pub minor_units: i16,
    /// Stripe's minimum charge amount for this currency (assumes the
    /// connected account settles in it). Captures below it are settled from
    /// balance or forgiven rather than charged; it is also the initial hold
    /// size for members on the minimum-start hold strategy
    /// (`users.budget_holds = FALSE`).
    pub stripe_min_charge: Decimal,
    /// Stripe's maximum charge amount for this currency (8 digits of minor
    /// units for USD). Requested amounts are validated against it up front
    /// and automatic hold sizing clamps to it — an over-limit amount
    /// committed into an order would fail at Stripe with a permanent
    /// `amount_too_large` on every replay.
    pub stripe_max_charge: Decimal,
}

pub const DENOMINATIONS: &[Denomination] = &[
    Denomination {
        iso: "USD",
        symbol: "$",
        minor_units: 2,
        stripe_min_charge: rust_decimal::dec!(0.50),
        stripe_max_charge: rust_decimal::dec!(999999.99),
    },
    Denomination {
        iso: "EUR",
        symbol: "€",
        minor_units: 2,
        stripe_min_charge: rust_decimal::dec!(0.50),
        stripe_max_charge: rust_decimal::dec!(999999.99),
    },
    Denomination {
        iso: "GBP",
        symbol: "£",
        minor_units: 2,
        stripe_min_charge: rust_decimal::dec!(0.30),
        stripe_max_charge: rust_decimal::dec!(999999.99),
    },
];

/// Look up an allow-listed denomination by ISO code.
pub fn denomination(iso: &str) -> Option<&'static Denomination> {
    DENOMINATIONS.iter().find(|d| d.iso == iso)
}

/// Platform fee rate on stripe_payment charges (captures and purchases).
/// Applied Stripe-side only, via `application_fee_amount` — never as a
/// ledger entry; members always pay face value.
pub const PLATFORM_FEE_RATE: Decimal = rust_decimal::dec!(0.01);

/// Platform fee for a charge amount, rounded down to the currency's minor
/// units. Call sites skip the `application_fee_amount` parameter entirely
/// when this returns zero.
pub fn platform_fee(amount: Decimal, minor_units: i16) -> Decimal {
    (amount * PLATFORM_FEE_RATE).round_dp_with_strategy(
        minor_units as u32,
        rust_decimal::RoundingStrategy::ToZero,
    )
}

/// Convert an amount to Stripe's integer minor units (e.g. $12.34 → 1234).
/// Returns None if the amount has finer resolution than the currency's
/// minor units — amounts reaching Stripe must already be quantized, so a
/// None here is a caller bug, not a rounding opportunity.
pub fn to_minor_units(amount: Decimal, minor_units: i16) -> Option<i64> {
    // checked_mul: `Decimal`'s `*` panics on overflow, and `amount` can
    // be request input here.
    let scaled =
        amount.checked_mul(Decimal::from(10i64.pow(minor_units as u32)))?;
    if scaled.fract() != Decimal::ZERO {
        return None;
    }
    scaled.to_i64()
}

/// Convert Stripe's integer minor units back to an amount
/// (e.g. 1234 → $12.34).
pub fn from_minor_units(minor: i64, minor_units: i16) -> Decimal {
    Decimal::from(minor) / Decimal::from(10i64.pow(minor_units as u32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn denomination_lookup() {
        let usd = denomination("USD").unwrap();
        assert_eq!(usd.symbol, "$");
        assert_eq!(usd.minor_units, 2);
        assert_eq!(denomination("EUR").unwrap().symbol, "€");
        assert_eq!(denomination("GBP").unwrap().symbol, "£");
        assert!(denomination("usd").is_none());
        assert!(denomination("JPY").is_none());
    }

    #[test]
    fn minor_unit_round_trip() {
        assert_eq!(to_minor_units(dec!(12.34), 2), Some(1234));
        assert_eq!(to_minor_units(dec!(0.50), 2), Some(50));
        assert_eq!(to_minor_units(dec!(150), 0), Some(150));
        // Sub-minor-unit resolution is a caller bug, not roundable
        assert_eq!(to_minor_units(dec!(12.345), 2), None);
        assert_eq!(from_minor_units(1234, 2), dec!(12.34));
        assert_eq!(from_minor_units(150, 0), dec!(150));
    }

    #[test]
    fn platform_fee_rounds_down_to_minor_units() {
        // 1% of $10.00 lands exactly on the grain
        assert_eq!(platform_fee(dec!(10.00), 2), dec!(0.10));
        // 1% of $10.99 = $0.1099, floored to the cent
        assert_eq!(platform_fee(dec!(10.99), 2), dec!(0.10));
        // Sub-dollar charges floor to zero (call sites then skip the
        // fee parameter entirely)
        assert_eq!(platform_fee(dec!(0.99), 2), dec!(0.00));
        // Zero-minor-unit currencies floor to whole units
        assert_eq!(platform_fee(dec!(150), 0), dec!(1));
    }
}
