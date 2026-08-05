//! Stripe integration service: subscription billing, Connect account
//! onboarding, saved-card payment profiles, and the PaymentIntent and
//! Checkout calls behind backed-credits funding and purchases.
//!
//! The actual Stripe API calls live in [`real`], which is compiled
//! unconditionally so the env-gated sandbox tests (tests/api/stripe_sandbox.rs)
//! can exercise them against Stripe test mode even though the test build mocks
//! `StripeService`. The service methods are cfg-paired: real builds delegate
//! to [`real`], mock-stripe builds return canned values and record calls.

use anyhow::{Context, Result};
#[cfg(any(test, not(feature = "mock-stripe")))]
use secrecy::ExposeSecret;
use secrecy::SecretBox;

/// Live status of a connected account, as read from Stripe.
#[derive(Debug, Clone, Default)]
pub struct AccountStatus {
    pub charges_enabled: bool,
    pub details_submitted: bool,
    /// The account's settlement currency (lowercase ISO code), once known.
    pub default_currency: Option<String>,
}

/// A card payment method as read from Stripe: the id plus the display
/// metadata persisted into `user_payment_profiles`.
#[derive(Debug, Clone, PartialEq)]
pub struct CardPaymentMethod {
    pub id: String,
    pub brand: String,
    pub last4: String,
    pub exp_month: i16,
    pub exp_year: i16,
}

/// Inputs for creating a manual-capture card authorization on a community's
/// connected account (a direct charge backed by the member's platform-saved
/// card, cloned per Stripe's direct-charges-multiple-accounts guidance).
pub struct AuthorizationParams<'a> {
    pub connected_account_id: &'a str,
    pub platform_customer_id: &'a str,
    pub platform_payment_method_id: &'a str,
    /// Amount in the currency's minor units.
    pub amount_minor: i64,
    /// Lowercase ISO currency code.
    pub currency: &'a str,
    /// Attached to the PaymentIntent for reverse lookup (webhook intent
    /// adoption, reconciliation): funding intent/row ids, auction, member,
    /// community.
    pub metadata: std::collections::HashMap<String, String>,
    /// Seed for the Stripe idempotency keys of the two calls this makes
    /// (payment-method clone, then create+confirm). Derived from the
    /// pending intent row's id, so a crashed operation's retry converges
    /// on the same Stripe objects.
    pub idempotency_seed: &'a str,
}

/// A confirmed authorization: the PaymentIntent id, Stripe's response
/// amount (authoritative for `authorized_amount`), and the charge's
/// capture deadline.
#[derive(Debug, Clone)]
pub struct PaymentIntentAuth {
    pub payment_intent_id: String,
    pub amount_minor: i64,
    /// Stripe's per-charge capture deadline
    /// (`latest_charge.payment_method_details.card.capture_before`) as an
    /// epoch second — authoritative over any locally assumed hold
    /// window, since the window varies by network and transaction type
    /// (Visa merchant-initiated: 4d18h; most others: 7d). None if
    /// Stripe omitted it (callers fall back to the assumed 4-day
    /// floor).
    pub capture_before_epoch: Option<i64>,
}

/// Card decline details from a refused confirm. `payment_intent_id` is the
/// intent Stripe created before refusing, when one exists — persisted so
/// the canceled intent row still links to the Stripe object.
#[derive(Debug, Clone)]
pub struct DeclineInfo {
    pub code: Option<String>,
    pub decline_code: Option<String>,
    pub message: Option<String>,
    pub payment_intent_id: Option<String>,
}

impl DeclineInfo {
    /// The most specific code to surface or record: the card network's
    /// `decline_code` when present, else Stripe's error code. Exception:
    /// a 3DS demand carries `code: authentication_required` alongside a
    /// less useful decline code (`authentication_not_handled`), and the
    /// UI keys the resolve-via-checkout path off the error code, so it
    /// wins.
    pub fn best_code(&self) -> Option<&str> {
        if self.code.as_deref() == Some("authentication_required") {
            return self.code.as_deref();
        }
        self.decline_code.as_deref().or(self.code.as_deref())
    }
}

/// A PaymentIntent's live status as reported by Stripe, parsed into a
/// closed enum at the service boundary so consumers match named
/// variants instead of raw strings, and so convergence can match the
/// (local × live) product exhaustively. `Missing` is
/// retrieve-returns-nothing (`resource_missing` — intent ids are
/// account-scoped, so an intent minted on a since-replaced account
/// reads as missing); `Unknown` is a status this build doesn't
/// recognize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivePiStatus {
    RequiresPaymentMethod,
    RequiresConfirmation,
    RequiresAction,
    Processing,
    RequiresCapture,
    Succeeded,
    Canceled,
    Missing,
    Unknown,
}

impl LivePiStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RequiresPaymentMethod => "requires_payment_method",
            Self::RequiresConfirmation => "requires_confirmation",
            Self::RequiresAction => "requires_action",
            Self::Processing => "processing",
            Self::RequiresCapture => "requires_capture",
            Self::Succeeded => "succeeded",
            Self::Canceled => "canceled",
            Self::Missing => "missing",
            Self::Unknown => "unknown",
        }
    }
}

impl From<&stripe_shared::PaymentIntentStatus> for LivePiStatus {
    fn from(status: &stripe_shared::PaymentIntentStatus) -> Self {
        use stripe_shared::PaymentIntentStatus as S;
        match status {
            S::RequiresPaymentMethod => Self::RequiresPaymentMethod,
            S::RequiresConfirmation => Self::RequiresConfirmation,
            S::RequiresAction => Self::RequiresAction,
            S::Processing => Self::Processing,
            S::RequiresCapture => Self::RequiresCapture,
            S::Succeeded => Self::Succeeded,
            S::Canceled => Self::Canceled,
            // `Unknown(_)`, plus the non_exhaustive wildcard.
            _ => Self::Unknown,
        }
    }
}

impl std::str::FromStr for LivePiStatus {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "requires_payment_method" => Self::RequiresPaymentMethod,
            "requires_confirmation" => Self::RequiresConfirmation,
            "requires_action" => Self::RequiresAction,
            "processing" => Self::Processing,
            "requires_capture" => Self::RequiresCapture,
            "succeeded" => Self::Succeeded,
            "canceled" => Self::Canceled,
            other => {
                tracing::warn!(status = other, "unrecognized PI status");
                Self::Unknown
            }
        })
    }
}

impl std::fmt::Display for LivePiStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a PaymentIntent `cancellation_reason` means Stripe's own
/// expiry of an aged-out uncaptured hold (observed: `expired`;
/// `automatic` kept from older docs) rather than a deliberate cancel.
pub fn is_expiry_reason(reason: Option<&str>) -> bool {
    matches!(reason, Some("expired") | Some("automatic"))
}

/// A PaymentIntent's live state at Stripe, for reconciliation's
/// read-only cross-check, the stuck-purchase probe, and webhook
/// adoption's retrieve-then-decide.
#[derive(Debug, Clone)]
pub struct RetrievedPaymentIntent {
    pub status: LivePiStatus,
    pub amount_minor: i64,
    pub amount_capturable_minor: i64,
    pub amount_received_minor: i64,
    pub cancellation_reason: Option<String>,
    /// The per-charge capture deadline (epoch seconds), the
    /// authoritative capture window — present once a manual-capture
    /// confirm succeeded (retrieved with `latest_charge` expanded).
    pub capture_before_epoch: Option<i64>,
}

/// A PaymentIntent located by metadata, for the orphaned-hold sweep —
/// enough to decide whether a hold exists and cancel it.
#[derive(Debug, Clone)]
pub struct FoundPaymentIntent {
    pub payment_intent_id: String,
    pub status: LivePiStatus,
    pub cancellation_reason: Option<String>,
}

/// Which flow a payment-mode Checkout session serves. Carries the
/// flow's row id, which rides in both the session and PaymentIntent
/// metadata (so webhooks route back to the row) and seeds the
/// idempotency key.
pub enum SessionMode<'a> {
    /// An immediate-capture credit purchase; the optional platform fee
    /// applies at payment.
    Purchase {
        purchase_id: &'a payloads::CreditPurchaseId,
        application_fee_minor: Option<i64>,
    },
    /// A manual-capture funding authorization for an unsaved card:
    /// completing the session mints the auction's card hold. No
    /// platform fee is set at mint — settlement's capture applies it on
    /// the captured amount (decision 11). Cards only (wallets surface
    /// as cards): the hold model reads the card-specific
    /// `capture_before` window, and other manual-capture methods (bank
    /// debits especially) lack guaranteed-funds semantics — a
    /// "captured" bank debit can still be returned days later.
    Funding {
        intent_id: &'a payloads::FundingIntentId,
    },
}

impl SessionMode<'_> {
    /// The metadata key webhooks resolve the row by.
    fn metadata_key(&self) -> &'static str {
        match self {
            Self::Purchase { .. } => "credit_purchase_id",
            Self::Funding { .. } => "funding_intent_id",
        }
    }

    /// The row id as a string: the metadata value and idempotency seed.
    fn id(&self) -> String {
        match self {
            Self::Purchase { purchase_id, .. } => purchase_id.to_string(),
            Self::Funding { intent_id } => intent_id.to_string(),
        }
    }

    /// Short flow label for logs and mock URLs.
    fn kind(&self) -> &'static str {
        match self {
            Self::Purchase { .. } => "purchase",
            Self::Funding { .. } => "funding",
        }
    }
}

/// Inputs for creating a payment-mode Checkout session on a community's
/// connected account (a direct charge); [`SessionMode`] carries the
/// per-flow differences.
pub struct CheckoutSessionParams<'a> {
    pub account_id: &'a str,
    pub mode: SessionMode<'a>,
    /// Amount in the currency's minor units.
    pub amount_minor: i64,
    /// Lowercase ISO currency code.
    pub currency: &'a str,
    /// Line-item description shown on Stripe's payment page.
    pub description: &'a str,
    pub success_url: &'a str,
    pub cancel_url: &'a str,
}

/// A minted purchase Checkout session: the id is stored on the purchase
/// row for webhook matching; the URL is the member's redirect target.
#[derive(Debug, Clone)]
pub struct PurchaseCheckoutSession {
    pub session_id: String,
    pub url: String,
}

/// Outcome of expiring a Checkout session. `Expired` covers both a
/// fresh expiry and a session Stripe had already expired — either way
/// the session can never be completed. `Completed` means the customer
/// already paid: the session can't be killed and its payment is in
/// flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpireSessionOutcome {
    Expired,
    Completed,
}

/// A Checkout session's live status as reported by Stripe, parsed into
/// a closed enum at the service boundary (mirroring [`LivePiStatus`]).
/// `Complete` means the customer submitted payment — the session's
/// PaymentIntent carries the collection state from there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveSessionStatus {
    Open,
    Complete,
    Expired,
    /// Retrieve-returns-nothing (`resource_missing`): session ids are
    /// account-scoped, so a session minted on a since-replaced account
    /// reads as missing. Mapped by callers from a `None` retrieve, like
    /// [`LivePiStatus::Missing`].
    Missing,
    Unknown,
}

impl LiveSessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Complete => "complete",
            Self::Expired => "expired",
            Self::Missing => "missing",
            Self::Unknown => "unknown",
        }
    }
}

impl std::str::FromStr for LiveSessionStatus {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "open" => Self::Open,
            "complete" => Self::Complete,
            "expired" => Self::Expired,
            other => {
                tracing::warn!(status = other, "unrecognized session status");
                Self::Unknown
            }
        })
    }
}

impl std::fmt::Display for LiveSessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A Checkout session's live state at Stripe, for the purchase
/// converge's retrieve-then-decide (webhook pokes and the
/// pre-settlement probe).
#[derive(Debug, Clone)]
pub struct RetrievedCheckoutSession {
    pub status: LiveSessionStatus,
    /// The session's PaymentIntent, present once payment was submitted.
    pub payment_intent_id: Option<String>,
}

/// Classified error from an account-scoped Stripe intent call
/// (create/cancel/capture). Each variant prescribes the caller's move,
/// so no call site re-derives its own subset from error prose:
///
/// - `Declined`: a card refusal (including the 3DS demand off-session confirms
///   surface via `error_on_requires_action`). Actionable by the member: decline
///   stamp, pause, notification.
/// - `StateConflict`: `payment_intent_unexpected_state` — the intent exists but
///   is not in the state the request needs; `live_status` is its embedded
///   current status. The caller hands the pair to convergence instead of
///   retrying.
/// - `PermanentRequest`: `invalid_request_error` and kin (`amount_too_large`,
///   malformed params, a missing object on a mutation). Retrying is futile; the
///   caller terminalizes its row.
/// - `AccountGone`: the connected account is deleted or disconnected (Stripe
///   reports no structured code; matched by prose, shared with the
///   account-status paths via [`is_account_gone_message`]).
/// - `Transient`: network errors, 5xx, rate limits. State untouched; retry on
///   backoff.
#[derive(Debug)]
pub enum StripeCallError {
    Declined(DeclineInfo),
    StateConflict {
        live_status: LivePiStatus,
        cancellation_reason: Option<String>,
    },
    PermanentRequest(anyhow::Error),
    AccountGone(anyhow::Error),
    Transient(anyhow::Error),
}

impl std::fmt::Display for StripeCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Declined(d) => write!(
                f,
                "card declined (code {:?}, decline_code {:?})",
                d.code, d.decline_code
            ),
            Self::StateConflict {
                live_status,
                cancellation_reason,
            } => write!(
                f,
                "intent state conflict (live status {live_status}, \
                 cancellation reason {cancellation_reason:?})"
            ),
            Self::PermanentRequest(e) => {
                write!(f, "permanent request error: {e:#}")
            }
            Self::AccountGone(e) => {
                write!(f, "connected account gone: {e:#}")
            }
            Self::Transient(e) => write!(f, "transient Stripe error: {e:#}"),
        }
    }
}

impl std::error::Error for StripeCallError {}

/// Whether a Stripe error message means the connected account is gone
/// from the platform's perspective — deleted, or disconnected from the
/// community's dashboard. Stripe reports no structured code for these,
/// so match its messages: revoked access reads "...does not have
/// access to account 'acct_...' (or that account does not exist)...",
/// and account-link creation for a dead account reads "...an account
/// that is not connected to your platform or does not exist.". Both
/// arise only on account-scoped calls, so the phrases can't collide
/// with unrelated errors (a missing object reads "No such ...").
pub(crate) fn is_account_gone_message(msg: &str) -> bool {
    let msg = msg.to_lowercase();
    msg.contains("does not have access")
        || msg.contains("does not exist")
        || msg.contains("not connected to your platform")
}

/// A mock PaymentIntent's state, for asserting on the authorization
/// machinery in tests. Statuses mirror Stripe's: `requires_capture`
/// (live hold), `requires_payment_method` (declined confirm),
/// `succeeded`, `canceled`.
#[cfg(feature = "mock-stripe")]
#[derive(Debug, Clone)]
pub struct MockPaymentIntent {
    pub account_id: String,
    pub amount_minor: i64,
    pub status: String,
    pub metadata: std::collections::HashMap<String, String>,
    pub amount_received: i64,
    pub application_fee_minor: Option<i64>,
    /// The per-charge capture deadline this authorization reported
    /// (from `mock_capture_before_epoch` at creation); None falls back
    /// to the assumed hold-window floor.
    pub capture_before_epoch: Option<i64>,
    /// Why the intent is `canceled` (Stripe's field): our mock cancel
    /// sets "requested_by_customer"; tests simulating a Stripe-side
    /// expiry set the status to "canceled" with reason "expired".
    pub cancellation_reason: Option<String>,
    /// The idempotency key of the capture that succeeded this intent.
    /// Like Stripe, only a replay under the same key returns the stored
    /// success; a distinct-key capture of a `succeeded` intent errors
    /// (`payment_intent_unexpected_state`).
    pub capture_idempotency_key: Option<String>,
}

/// A mock Checkout session's state, for the purchase-flow tests.
/// Statuses mirror Stripe's: `open` (payable), `complete` (paid —
/// `payment_intent_id` carries the collecting intent), `expired`.
/// Tests stage a member's payment by setting `complete` plus the
/// intent id out-of-band.
#[cfg(feature = "mock-stripe")]
#[derive(Debug, Clone)]
pub struct MockCheckoutSession {
    pub account_id: String,
    pub status: String,
    pub payment_intent_id: Option<String>,
    /// What creation charged into the session, recorded so tests can
    /// assert the amount, denomination, and platform fee actually reach
    /// Stripe (a fee regression must fail a test, not just the books).
    pub amount_minor: i64,
    pub currency: String,
    pub application_fee_minor: Option<i64>,
    /// Whether the session's PaymentIntent authorizes rather than
    /// charges (funding sessions), recorded for the same reason as the
    /// amounts: a funding session minted without manual capture would
    /// charge members instead of holding, and must fail a test.
    pub manual_capture: bool,
}

pub struct StripeService {
    #[cfg(not(feature = "mock-stripe"))]
    client: stripe::Client,
    #[allow(dead_code)]
    webhook_secret: SecretBox<String>,
    /// Signing secret for the Connect webhook endpoint (events from
    /// connected accounts arrive on a separate endpoint with its own
    /// secret).
    #[allow(dead_code)]
    connect_webhook_secret: SecretBox<String>,
    /// Mock: maps stripe customer IDs to community IDs.
    /// Tests populate this before sending webhook events.
    #[cfg(feature = "mock-stripe")]
    pub mock_customer_communities: std::sync::Mutex<
        std::collections::HashMap<String, payloads::CommunityId>,
    >,
    /// Mock: records subscription IDs that were canceled.
    #[cfg(feature = "mock-stripe")]
    pub mock_canceled_subscriptions: std::sync::Mutex<Vec<String>>,
    /// Mock: connected accounts created, as (account id, community id).
    #[cfg(feature = "mock-stripe")]
    pub mock_connected_accounts:
        std::sync::Mutex<Vec<(String, payloads::CommunityId)>>,
    /// Mock: per-account status returned by `get_account_status`; absent
    /// entries read as the post-creation default (nothing submitted,
    /// charges disabled, no currency).
    #[cfg(feature = "mock-stripe")]
    pub mock_account_status:
        std::sync::Mutex<std::collections::HashMap<String, AccountStatus>>,
    /// Mock: connected account ids the platform has lost access to
    /// (deleted or disconnected in Stripe). Account-scoped calls for
    /// these ids fail with Stripe's real error messages.
    #[cfg(feature = "mock-stripe")]
    pub mock_gone_accounts: std::sync::Mutex<std::collections::HashSet<String>>,
    /// Mock: platform customers created for users, as
    /// (customer id, user id).
    #[cfg(feature = "mock-stripe")]
    pub mock_user_customers: std::sync::Mutex<Vec<(String, payloads::UserId)>>,
    /// Mock: card payment methods per customer, newest first (what
    /// `list_card_payment_methods` returns). Tests populate this to
    /// simulate a completed Checkout setup session.
    #[cfg(feature = "mock-stripe")]
    pub mock_customer_payment_methods: std::sync::Mutex<
        std::collections::HashMap<String, Vec<CardPaymentMethod>>,
    >,
    /// Mock: payment method ids that were detached.
    #[cfg(feature = "mock-stripe")]
    pub mock_detached_payment_methods: std::sync::Mutex<Vec<String>>,
    /// Mock: PaymentIntents by id (`pi_mock_{n}`), the authorization
    /// state machine tests assert on and drive webhook events from.
    #[cfg(feature = "mock-stripe")]
    pub mock_payment_intents:
        std::sync::Mutex<std::collections::HashMap<String, MockPaymentIntent>>,
    /// Mock: `create_authorization` idempotency replays, seed → the
    /// recorded original outcome, mirroring Stripe's replay-by-key
    /// semantics: a replay returns the stored original response
    /// verbatim, even if the intent's live status has since changed
    /// (e.g. the hold was canceled out-of-band after a confirm that
    /// crashed pre-commit — the replayed success is how production
    /// activates a dead hold, converging via webhook/reconciliation).
    #[cfg(feature = "mock-stripe")]
    pub mock_intent_replays: std::sync::Mutex<
        std::collections::HashMap<
            String,
            Result<PaymentIntentAuth, DeclineInfo>,
        >,
    >,
    /// Mock: platform payment method ids that decline authorization,
    /// mapped to the decline code (e.g. "insufficient_funds"). The
    /// special value "authentication_required" simulates a 3DS demand.
    #[cfg(feature = "mock-stripe")]
    pub mock_declining_payment_methods:
        std::sync::Mutex<std::collections::HashMap<String, String>>,
    /// Mock: the per-charge `capture_before` (epoch seconds) fresh
    /// authorizations report — tests set this to simulate cards with
    /// short capture windows; None (default) omits the field so the
    /// caller's assumed-floor fallback applies. Stored on the intent at
    /// creation so replays return the same window.
    #[cfg(feature = "mock-stripe")]
    pub mock_capture_before_epoch: std::sync::Mutex<Option<i64>>,
    /// Mock: purchase Checkout sessions by id (`cs_mock_{purchase}`),
    /// inserted `open` at creation; `expire_checkout_session` and tests
    /// mutate the status (see [`MockCheckoutSession`]).
    #[cfg(feature = "mock-stripe")]
    pub mock_sessions: std::sync::Mutex<
        std::collections::HashMap<String, MockCheckoutSession>,
    >,
}

impl StripeService {
    #[cfg(not(feature = "mock-stripe"))]
    pub fn new(
        api_key: SecretBox<String>,
        webhook_secret: SecretBox<String>,
        connect_webhook_secret: SecretBox<String>,
    ) -> Self {
        let client = stripe::Client::new(api_key.expose_secret());
        Self {
            client,
            webhook_secret,
            connect_webhook_secret,
        }
    }

    #[cfg(feature = "mock-stripe")]
    pub fn new(
        _api_key: SecretBox<String>,
        webhook_secret: SecretBox<String>,
        connect_webhook_secret: SecretBox<String>,
    ) -> Self {
        Self {
            webhook_secret,
            connect_webhook_secret,
            mock_customer_communities: std::sync::Mutex::new(
                std::collections::HashMap::new(),
            ),
            mock_canceled_subscriptions: std::sync::Mutex::new(Vec::new()),
            mock_connected_accounts: std::sync::Mutex::new(Vec::new()),
            mock_account_status: std::sync::Mutex::new(
                std::collections::HashMap::new(),
            ),
            mock_gone_accounts: std::sync::Mutex::new(
                std::collections::HashSet::new(),
            ),
            mock_user_customers: std::sync::Mutex::new(Vec::new()),
            mock_customer_payment_methods: std::sync::Mutex::new(
                std::collections::HashMap::new(),
            ),
            mock_detached_payment_methods: std::sync::Mutex::new(Vec::new()),
            mock_payment_intents: std::sync::Mutex::new(
                std::collections::HashMap::new(),
            ),
            mock_intent_replays: std::sync::Mutex::new(
                std::collections::HashMap::new(),
            ),
            mock_declining_payment_methods: std::sync::Mutex::new(
                std::collections::HashMap::new(),
            ),
            mock_capture_before_epoch: std::sync::Mutex::new(None),
            mock_sessions: std::sync::Mutex::new(
                std::collections::HashMap::new(),
            ),
        }
    }

    /// Fail like the real API when the connected account is in
    /// `mock_gone_accounts`: every account-scoped call on a deleted or
    /// disconnected account rejects with the access-revoked message
    /// (recognized by [`is_account_gone_message`]), so tests
    /// interleaving deauthorization with intent and session operations
    /// exercise the behavior production exhibits.
    #[cfg(feature = "mock-stripe")]
    fn check_account_not_gone(&self, account_id: &str) -> Result<()> {
        if self.mock_gone_accounts.lock().unwrap().contains(account_id) {
            // Stripe's real message for revoked/deleted account access.
            anyhow::bail!(
                "error reported by stripe: The provided key \
                 'sk_test_...' does not have access to account \
                 '{account_id}' (or that account does not exist). \
                 Application access may have been revoked."
            );
        }
        Ok(())
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

    /// Create a connected account for a community. Controller properties
    /// are the Standard-equivalent set (design doc, Connect account
    /// configuration): the community is merchant of record with its own
    /// full dashboard, pays its own fees, and Stripe holds
    /// liability/collects requirements.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn create_connected_account(
        &self,
        community_name: &str,
        community_id: &payloads::CommunityId,
    ) -> Result<String> {
        real::create_connected_account(
            &self.client,
            community_name,
            community_id,
        )
        .await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn create_connected_account(
        &self,
        _community_name: &str,
        community_id: &payloads::CommunityId,
    ) -> Result<String> {
        let mut accounts = self.mock_connected_accounts.lock().unwrap();
        let id = format!("acct_mock_{}", accounts.len() + 1);
        accounts.push((id.clone(), *community_id));
        Ok(id)
    }

    /// Create an onboarding Account Link for a connected account.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn create_account_link(
        &self,
        account_id: &str,
        refresh_url: &str,
        return_url: &str,
    ) -> Result<String> {
        real::create_account_link(
            &self.client,
            account_id,
            refresh_url,
            return_url,
        )
        .await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn create_account_link(
        &self,
        account_id: &str,
        _refresh_url: &str,
        _return_url: &str,
    ) -> Result<String> {
        if self.mock_gone_accounts.lock().unwrap().contains(account_id) {
            // Stripe's real message for a deleted/disconnected account.
            anyhow::bail!(
                "error reported by stripe: You requested an account \
                 link for an account that is not connected to your \
                 platform or does not exist."
            );
        }
        // `.invalid` (RFC 2606) never resolves: following the mock URL in
        // a browser fails fast instead of landing on a real Stripe page
        // (a real-domain mock URL once bounced a logged-in dev-server
        // session into an unrelated account's dashboard onboarding).
        Ok(format!(
            "https://mock-stripe.invalid/onboarding/{account_id}"
        ))
    }

    /// Fetch a connected account's live status.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn get_account_status(
        &self,
        account_id: &str,
    ) -> Result<AccountStatus> {
        real::get_account_status(&self.client, account_id).await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn get_account_status(
        &self,
        account_id: &str,
    ) -> Result<AccountStatus> {
        self.check_account_not_gone(account_id)?;
        Ok(self
            .mock_account_status
            .lock()
            .unwrap()
            .get(account_id)
            .cloned()
            .unwrap_or_default())
    }

    /// Create a platform-level Stripe customer for a user (the anchor
    /// their saved card attaches to; cloned to connected accounts at
    /// charge time in later phases).
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn create_user_customer(
        &self,
        username: &str,
        user_id: &payloads::UserId,
    ) -> Result<String> {
        real::create_user_customer(&self.client, username, user_id).await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn create_user_customer(
        &self,
        _username: &str,
        user_id: &payloads::UserId,
    ) -> Result<String> {
        let mut customers = self.mock_user_customers.lock().unwrap();
        let id = format!("cus_user_mock_{}", customers.len() + 1);
        customers.push((id.clone(), *user_id));
        Ok(id)
    }

    /// Create a Checkout session in setup mode (platform account) for
    /// saving a card; 3DS/SCA happens on Stripe's page.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn create_setup_checkout_session(
        &self,
        customer_id: &str,
        user_id: &payloads::UserId,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<String> {
        real::create_setup_checkout_session(
            &self.client,
            customer_id,
            user_id,
            success_url,
            cancel_url,
        )
        .await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn create_setup_checkout_session(
        &self,
        customer_id: &str,
        _user_id: &payloads::UserId,
        _success_url: &str,
        _cancel_url: &str,
    ) -> Result<String> {
        Ok(format!("https://mock-stripe.invalid/setup/{customer_id}"))
    }

    /// Create a payment-mode Checkout session on the community's
    /// connected account (a direct charge): immediate-capture for a
    /// credit purchase, manual-capture for an unsaved-card funding
    /// authorization. The mode's row id rides in both the session and
    /// PaymentIntent metadata so webhooks route back to the row, and
    /// seeds the idempotency key. Purchases leave payment method types
    /// to the account's dashboard configuration (ACH and other delayed
    /// methods included); funding sessions restrict to cards (see
    /// [`SessionMode::Funding`]).
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn create_payment_checkout_session(
        &self,
        params: CheckoutSessionParams<'_>,
    ) -> Result<PurchaseCheckoutSession> {
        real::create_payment_checkout_session(&self.client, params).await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn create_payment_checkout_session(
        &self,
        params: CheckoutSessionParams<'_>,
    ) -> Result<PurchaseCheckoutSession> {
        self.check_account_not_gone(params.account_id)?;
        let id = params.mode.id();
        let (application_fee_minor, manual_capture) = match params.mode {
            SessionMode::Purchase {
                application_fee_minor,
                ..
            } => (application_fee_minor, false),
            SessionMode::Funding { .. } => (None, true),
        };
        let session_id = format!("cs_mock_{id}");
        self.mock_sessions.lock().unwrap().insert(
            session_id.clone(),
            MockCheckoutSession {
                account_id: params.account_id.to_string(),
                status: "open".to_string(),
                payment_intent_id: None,
                amount_minor: params.amount_minor,
                currency: params.currency.to_string(),
                application_fee_minor,
                manual_capture,
            },
        );
        Ok(PurchaseCheckoutSession {
            session_id,
            url: format!(
                "https://mock-stripe.invalid/{}/{id}",
                params.mode.kind()
            ),
        })
    }

    /// Expire an open Checkout session on a connected account. Stripe
    /// only expires `open` sessions, atomically against completion: a
    /// session the customer already paid reports `Completed` instead.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn expire_checkout_session(
        &self,
        account_id: &str,
        session_id: &str,
    ) -> Result<ExpireSessionOutcome> {
        real::expire_checkout_session(&self.client, account_id, session_id)
            .await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn expire_checkout_session(
        &self,
        account_id: &str,
        session_id: &str,
    ) -> Result<ExpireSessionOutcome> {
        self.check_account_not_gone(account_id)?;
        let mut sessions = self.mock_sessions.lock().unwrap();
        let session =
            sessions.entry(session_id.to_string()).or_insert_with(|| {
                // Fabricated fallback for tests that expire sessions
                // seeded outside the mock; charge details unknown.
                MockCheckoutSession {
                    account_id: account_id.to_string(),
                    status: "open".to_string(),
                    payment_intent_id: None,
                    amount_minor: 0,
                    currency: String::new(),
                    application_fee_minor: None,
                    manual_capture: false,
                }
            });
        if session.status == "complete" {
            return Ok(ExpireSessionOutcome::Completed);
        }
        session.status = "expired".to_string();
        Ok(ExpireSessionOutcome::Expired)
    }

    /// Retrieve a Checkout session's live state on a connected account
    /// (read-only), or None when no such session exists on that account
    /// (`resource_missing` — e.g. a since-replaced account). The
    /// purchase converge's session leg.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn retrieve_checkout_session(
        &self,
        account_id: &str,
        session_id: &str,
    ) -> Result<Option<RetrievedCheckoutSession>> {
        real::retrieve_checkout_session(&self.client, account_id, session_id)
            .await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn retrieve_checkout_session(
        &self,
        account_id: &str,
        session_id: &str,
    ) -> Result<Option<RetrievedCheckoutSession>> {
        self.check_account_not_gone(account_id)?;
        let sessions = self.mock_sessions.lock().unwrap();
        Ok(sessions
            .get(session_id)
            .filter(|s| s.account_id == account_id)
            .map(|s| RetrievedCheckoutSession {
                status: s.status.parse().unwrap(),
                payment_intent_id: s.payment_intent_id.clone(),
            }))
    }

    /// List a customer's card payment methods, newest first.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn list_card_payment_methods(
        &self,
        customer_id: &str,
    ) -> Result<Vec<CardPaymentMethod>> {
        real::list_card_payment_methods(&self.client, customer_id).await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn list_card_payment_methods(
        &self,
        customer_id: &str,
    ) -> Result<Vec<CardPaymentMethod>> {
        Ok(self
            .mock_customer_payment_methods
            .lock()
            .unwrap()
            .get(customer_id)
            .cloned()
            .unwrap_or_default())
    }

    /// Detach a payment method from its customer.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn detach_payment_method(
        &self,
        payment_method_id: &str,
    ) -> Result<()> {
        real::detach_payment_method(&self.client, payment_method_id).await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn detach_payment_method(
        &self,
        payment_method_id: &str,
    ) -> Result<()> {
        for methods in self
            .mock_customer_payment_methods
            .lock()
            .unwrap()
            .values_mut()
        {
            methods.retain(|m| m.id != payment_method_id);
        }
        self.mock_detached_payment_methods
            .lock()
            .unwrap()
            .push(payment_method_id.to_string());
        Ok(())
    }

    /// Create and confirm a manual-capture authorization on a connected
    /// account: clone the member's platform payment method to the account,
    /// then create+confirm an off-session PaymentIntent with it. A card
    /// refusal (including a 3DS demand) returns
    /// `StripeCallError::Declined`.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn create_authorization(
        &self,
        params: AuthorizationParams<'_>,
    ) -> Result<PaymentIntentAuth, StripeCallError> {
        real::create_authorization(&self.client, params).await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn create_authorization(
        &self,
        params: AuthorizationParams<'_>,
    ) -> Result<PaymentIntentAuth, StripeCallError> {
        self.check_account_not_gone(params.connected_account_id)
            .map_err(StripeCallError::AccountGone)?;
        // Replay by idempotency seed: the stored original response,
        // like Stripe — not a re-derivation from the intent's live
        // status (see the `mock_intent_replays` field docs).
        let replayed = self
            .mock_intent_replays
            .lock()
            .unwrap()
            .get(params.idempotency_seed)
            .cloned();
        if let Some(outcome) = replayed {
            return outcome.map_err(StripeCallError::Declined);
        }

        let decline = self
            .mock_declining_payment_methods
            .lock()
            .unwrap()
            .get(params.platform_payment_method_id)
            .cloned();
        let capture_before_epoch =
            *self.mock_capture_before_epoch.lock().unwrap();
        let mut intents = self.mock_payment_intents.lock().unwrap();
        let id = format!("pi_mock_{}", intents.len() + 1);
        let declined = decline.is_some();
        intents.insert(
            id.clone(),
            MockPaymentIntent {
                account_id: params.connected_account_id.to_string(),
                amount_minor: params.amount_minor,
                status: if declined {
                    "requires_payment_method".to_string()
                } else {
                    "requires_capture".to_string()
                },
                metadata: params.metadata,
                amount_received: 0,
                application_fee_minor: None,
                capture_before_epoch,
                cancellation_reason: None,
                capture_idempotency_key: None,
            },
        );
        let outcome = match decline {
            Some(decline_code) => {
                // "authentication_required" is an error code, not an
                // issuer decline code; mirror Stripe's shape for it
                // (a real 3DS refusal carries both the error code and
                // the "authentication_not_handled" decline code).
                let (code, decline_code) =
                    if decline_code == "authentication_required" {
                        (
                            decline_code,
                            Some("authentication_not_handled".to_string()),
                        )
                    } else {
                        ("card_declined".to_string(), Some(decline_code))
                    };
                Err(DeclineInfo {
                    code: Some(code),
                    decline_code,
                    message: Some("mock decline".to_string()),
                    payment_intent_id: Some(id),
                })
            }
            None => Ok(PaymentIntentAuth {
                payment_intent_id: id,
                amount_minor: params.amount_minor,
                capture_before_epoch,
            }),
        };
        self.mock_intent_replays
            .lock()
            .unwrap()
            .insert(params.idempotency_seed.to_string(), outcome.clone());
        outcome.map_err(StripeCallError::Declined)
    }

    /// Cancel a PaymentIntent on a connected account, releasing its
    /// hold. An intent not in a cancelable state (already canceled — a
    /// missed webhook; or captured out-of-band) returns
    /// `StateConflict` with its live status.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn cancel_payment_intent(
        &self,
        connected_account_id: &str,
        payment_intent_id: &str,
        idempotency_key: &str,
    ) -> Result<(), StripeCallError> {
        real::cancel_payment_intent(
            &self.client,
            connected_account_id,
            payment_intent_id,
            idempotency_key,
        )
        .await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn cancel_payment_intent(
        &self,
        connected_account_id: &str,
        payment_intent_id: &str,
        _idempotency_key: &str,
    ) -> Result<(), StripeCallError> {
        self.check_account_not_gone(connected_account_id)
            .map_err(StripeCallError::AccountGone)?;
        let mut intents = self.mock_payment_intents.lock().unwrap();
        let intent = intents.get_mut(payment_intent_id).ok_or_else(|| {
            StripeCallError::PermanentRequest(anyhow::anyhow!(
                "Mock: no such intent {payment_intent_id}"
            ))
        })?;
        match intent.status.as_str() {
            "requires_capture" | "requires_payment_method" => {
                intent.status = "canceled".to_string();
                intent.cancellation_reason =
                    Some("requested_by_customer".to_string());
                Ok(())
            }
            // Not cancelable (already canceled — a replay of our own
            // cancel or a Stripe-side cancel/expiry the test staged —
            // or captured): payment_intent_unexpected_state at real
            // Stripe, with the live status embedded.
            other => Err(StripeCallError::StateConflict {
                live_status: other.parse().unwrap(),
                cancellation_reason: intent.cancellation_reason.clone(),
            }),
        }
    }

    /// Capture (part of) an authorized PaymentIntent on a connected
    /// account. `application_fee_minor` is the platform fee; None skips
    /// the parameter (fee floored to zero). An intent not awaiting
    /// capture (canceled/expired — the hold is gone; or already
    /// succeeded — captured out-of-band) returns `StateConflict` with
    /// its live status.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn capture_payment_intent(
        &self,
        connected_account_id: &str,
        payment_intent_id: &str,
        amount_minor: i64,
        application_fee_minor: Option<i64>,
        idempotency_key: &str,
    ) -> Result<(), StripeCallError> {
        real::capture_payment_intent(
            &self.client,
            connected_account_id,
            payment_intent_id,
            amount_minor,
            application_fee_minor,
            idempotency_key,
        )
        .await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn capture_payment_intent(
        &self,
        connected_account_id: &str,
        payment_intent_id: &str,
        amount_minor: i64,
        application_fee_minor: Option<i64>,
        idempotency_key: &str,
    ) -> Result<(), StripeCallError> {
        self.check_account_not_gone(connected_account_id)
            .map_err(StripeCallError::AccountGone)?;
        let mut intents = self.mock_payment_intents.lock().unwrap();
        let intent = intents.get_mut(payment_intent_id).ok_or_else(|| {
            StripeCallError::PermanentRequest(anyhow::anyhow!(
                "Mock: no such intent {payment_intent_id}"
            ))
        })?;
        match intent.status.as_str() {
            "requires_capture" => {
                if amount_minor > intent.amount_minor {
                    // amount_too_large: invalid_request_error at Stripe.
                    return Err(StripeCallError::PermanentRequest(
                        anyhow::anyhow!(
                            "Mock: capture exceeds authorized \
                             amount"
                        ),
                    ));
                }
                intent.status = "succeeded".to_string();
                intent.amount_received = amount_minor;
                intent.application_fee_minor = application_fee_minor;
                intent.capture_idempotency_key =
                    Some(idempotency_key.to_string());
                Ok(())
            }
            // Replay of our own capture, keyed like Stripe: same key
            // returns the stored success; a distinct key on a succeeded
            // intent falls to the StateConflict arm below
            // (payment_intent_unexpected_state at real Stripe).
            "succeeded"
                if intent.capture_idempotency_key.as_deref()
                    == Some(idempotency_key) =>
            {
                if intent.amount_received != amount_minor {
                    // IdempotencyError at real Stripe.
                    return Err(StripeCallError::PermanentRequest(
                        anyhow::anyhow!(
                            "Mock: idempotency key reused with \
                             different amount"
                        ),
                    ));
                }
                Ok(())
            }
            // Not awaiting capture: canceled/expired (hold gone,
            // webhook missed) or succeeded under a distinct key
            // (captured out-of-band).
            other => Err(StripeCallError::StateConflict {
                live_status: other.parse().unwrap(),
                cancellation_reason: intent.cancellation_reason.clone(),
            }),
        }
    }

    /// Retrieve a PaymentIntent's live state on a connected account
    /// (read-only; no idempotency key), or None when no such intent
    /// exists on that account (`resource_missing` — intent ids are
    /// account-scoped, so an intent minted on a since-replaced account
    /// reads as missing). Reconciliation's cross-check of local intent
    /// rows against Stripe truth, and the stuck-purchase probe.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn retrieve_payment_intent(
        &self,
        connected_account_id: &str,
        payment_intent_id: &str,
    ) -> Result<Option<RetrievedPaymentIntent>> {
        real::retrieve_payment_intent(
            &self.client,
            connected_account_id,
            payment_intent_id,
        )
        .await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn retrieve_payment_intent(
        &self,
        connected_account_id: &str,
        payment_intent_id: &str,
    ) -> Result<Option<RetrievedPaymentIntent>> {
        // Gone accounts error rather than read as missing — only
        // `resource_missing` maps to None in the real call.
        self.check_account_not_gone(connected_account_id)?;
        let intents = self.mock_payment_intents.lock().unwrap();
        Ok(intents
            .get(payment_intent_id)
            .filter(|intent| intent.account_id == connected_account_id)
            .map(|intent| RetrievedPaymentIntent {
                status: intent.status.parse().unwrap(),
                amount_minor: intent.amount_minor,
                amount_capturable_minor: if intent.status == "requires_capture"
                {
                    intent.amount_minor
                } else {
                    0
                },
                amount_received_minor: intent.amount_received,
                cancellation_reason: intent.cancellation_reason.clone(),
                capture_before_epoch: intent.capture_before_epoch,
            }))
    }

    /// Find the PaymentIntent on a connected account whose metadata
    /// carries `key` = `value`, or None if no such intent exists
    /// (read-only). Real Stripe: lists intents over the given created
    /// window (epoch-second bounds) and filters client-side — the
    /// Search API would be one indexed call but is unavailable in
    /// sandboxes, and candidates are rare enough that listing a bounded
    /// window is fine. The orphaned-hold sweep's existence probe.
    #[cfg(not(feature = "mock-stripe"))]
    pub async fn find_payment_intent_by_metadata(
        &self,
        connected_account_id: &str,
        key: &str,
        value: &str,
        created_gte_epoch: i64,
        created_lte_epoch: i64,
    ) -> Result<Option<FoundPaymentIntent>> {
        real::find_payment_intent_by_metadata(
            &self.client,
            connected_account_id,
            key,
            value,
            created_gte_epoch,
            created_lte_epoch,
        )
        .await
    }

    #[cfg(feature = "mock-stripe")]
    pub async fn find_payment_intent_by_metadata(
        &self,
        connected_account_id: &str,
        key: &str,
        value: &str,
        _created_gte_epoch: i64,
        _created_lte_epoch: i64,
    ) -> Result<Option<FoundPaymentIntent>> {
        self.check_account_not_gone(connected_account_id)?;
        // The mock has no created timestamps; account + metadata match
        // is the discriminating filter tests rely on.
        let intents = self.mock_payment_intents.lock().unwrap();
        Ok(intents
            .iter()
            .find(|(_, intent)| {
                intent.account_id == connected_account_id
                    && intent.metadata.get(key).is_some_and(|v| v == value)
            })
            .map(|(id, intent)| FoundPaymentIntent {
                payment_intent_id: id.clone(),
                status: intent.status.parse().unwrap(),
                cancellation_reason: intent.cancellation_reason.clone(),
            }))
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
        verify_signed_payload(
            &self.webhook_secret,
            payload,
            signature,
            time_source,
        )
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

    /// Verify a Connect-endpoint webhook signature (separate signing
    /// secret) and return the raw JSON payload.
    #[cfg(not(feature = "mock-stripe"))]
    pub fn verify_connect_webhook(
        &self,
        payload: &str,
        signature: &str,
        time_source: &crate::time::TimeSource,
    ) -> Result<serde_json::Value> {
        verify_signed_payload(
            &self.connect_webhook_secret,
            payload,
            signature,
            time_source,
        )
    }

    #[cfg(feature = "mock-stripe")]
    pub fn verify_connect_webhook(
        &self,
        payload: &str,
        _signature: &str,
        _time_source: &crate::time::TimeSource,
    ) -> Result<serde_json::Value> {
        serde_json::from_str(payload).context("Failed to parse webhook payload")
    }
}

/// Verify a Stripe webhook signature ("t=<ts>,v1=<hmac>") against a
/// signing secret and return the parsed JSON payload. During secret
/// rotation Stripe signs with both secrets and the header carries
/// multiple v1 entries, so the payload is accepted if any of them
/// verifies. Compiled in test builds (which enable mock-stripe) so
/// unit tests can exercise it.
#[cfg(any(test, not(feature = "mock-stripe")))]
fn verify_signed_payload(
    secret: &SecretBox<String>,
    payload: &str,
    signature: &str,
    time_source: &crate::time::TimeSource,
) -> Result<serde_json::Value> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    // Parse "t=<timestamp>,v1=<sig>[,v1=<sig>...]" from header
    let mut timestamp = None;
    let mut v1_sigs = Vec::new();
    for part in signature.split(',') {
        let part = part.trim();
        if let Some(t) = part.strip_prefix("t=") {
            timestamp = Some(t);
        } else if let Some(v) = part.strip_prefix("v1=") {
            v1_sigs.push(v);
        }
    }
    let timestamp: i64 = timestamp
        .ok_or_else(|| anyhow::anyhow!("Missing timestamp in signature"))?
        .parse()
        .context("Invalid timestamp")?;
    if v1_sigs.is_empty() {
        anyhow::bail!("Missing v1 in signature");
    }

    // Verify HMAC-SHA256; a malformed (non-hex) entry is treated as
    // non-matching rather than an error, since another entry may match.
    let signed_payload = format!("{}.{}", timestamp, payload);
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.expose_secret().as_bytes())
            .context("Invalid webhook secret")?;
    mac.update(signed_payload.as_bytes());
    let verified = v1_sigs.iter().any(|sig| {
        hex::decode(sig)
            .is_ok_and(|expected| mac.clone().verify_slice(&expected).is_ok())
    });
    if !verified {
        anyhow::bail!("Bad signature");
    }

    // Check timestamp freshness (5 minute tolerance)
    let now = time_source.now().as_second();
    if (now - timestamp).abs() > 300 {
        anyhow::bail!("Webhook timestamp too old: {timestamp}");
    }

    serde_json::from_str(payload).context("Invalid JSON in webhook payload")
}

/// The real Stripe API calls, as plain functions over a
/// `stripe::Client` so they compile (and are testable against the
/// Stripe sandbox) regardless of the mock-stripe feature.
pub mod real {
    use std::collections::HashMap;

    use anyhow::{Context, Result};
    use stripe::StripeRequest as _;

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

    /// Create a connected account with the Standard-equivalent controller
    /// properties (design doc, Connect account configuration).
    pub async fn create_connected_account(
        client: &stripe::Client,
        community_name: &str,
        community_id: &payloads::CommunityId,
    ) -> Result<String> {
        use stripe_connect::account::{
            CreateAccount, CreateAccountController,
            CreateAccountControllerFees, CreateAccountControllerFeesPayer,
            CreateAccountControllerLosses,
            CreateAccountControllerLossesPayments,
            CreateAccountControllerRequirementCollection,
            CreateAccountControllerStripeDashboard,
            CreateAccountControllerStripeDashboardType,
        };

        let metadata = HashMap::from([(
            "community_id".to_string(),
            community_id.to_string(),
        )]);

        let account = CreateAccount::new()
            .controller(CreateAccountController {
                fees: Some(CreateAccountControllerFees {
                    payer: Some(CreateAccountControllerFeesPayer::Account),
                }),
                losses: Some(CreateAccountControllerLosses {
                    payments: Some(
                        CreateAccountControllerLossesPayments::Stripe,
                    ),
                }),
                requirement_collection: Some(
                    CreateAccountControllerRequirementCollection::Stripe,
                ),
                stripe_dashboard: Some(
                    CreateAccountControllerStripeDashboard {
                        type_: Some(
                            CreateAccountControllerStripeDashboardType::Full,
                        ),
                    },
                ),
            })
            .business_profile(
                stripe_connect::account::CreateAccountBusinessProfile {
                    name: Some(community_name.to_string()),
                    ..Default::default()
                },
            )
            .metadata(metadata)
            .send(client)
            .await
            .context("Failed to create connected account")?;

        tracing::info!(
            account_id = %account.id,
            %community_id,
            "Stripe connected account created"
        );
        Ok(account.id.to_string())
    }

    /// Create an onboarding Account Link for a connected account.
    pub async fn create_account_link(
        client: &stripe::Client,
        account_id: &str,
        refresh_url: &str,
        return_url: &str,
    ) -> Result<String> {
        use stripe_connect::account_link::{
            CreateAccountLink, CreateAccountLinkType,
        };

        let link = CreateAccountLink::new(
            account_id,
            CreateAccountLinkType::AccountOnboarding,
        )
        .refresh_url(refresh_url)
        .return_url(return_url)
        .send(client)
        .await
        .context("Failed to create account link")?;

        tracing::info!(%account_id, "Stripe account link created");
        Ok(link.url)
    }

    /// Fetch a connected account's live status.
    pub async fn get_account_status(
        client: &stripe::Client,
        account_id: &str,
    ) -> Result<super::AccountStatus> {
        let account = stripe_connect::account::RetrieveAccount::new(
            parse_account_id(account_id)?,
        )
        .send(client)
        .await
        .context("Failed to retrieve connected account")?;

        Ok(super::AccountStatus {
            charges_enabled: account.charges_enabled.unwrap_or(false),
            details_submitted: account.details_submitted.unwrap_or(false),
            default_currency: account.default_currency.map(|c| c.to_string()),
        })
    }

    /// Create a platform-level Stripe customer for a user.
    pub async fn create_user_customer(
        client: &stripe::Client,
        username: &str,
        user_id: &payloads::UserId,
    ) -> Result<String> {
        let metadata =
            HashMap::from([("user_id".to_string(), user_id.to_string())]);

        let customer = stripe_core::customer::CreateCustomer::new()
            .name(username)
            .metadata(metadata)
            .send(client)
            .await
            .context("Failed to create Stripe customer for user")?;

        tracing::info!(
            customer_id = %customer.id,
            %user_id,
            "Stripe customer created for user"
        );
        Ok(customer.id.to_string())
    }

    /// Create a Checkout session in setup mode for saving a card.
    pub async fn create_setup_checkout_session(
        client: &stripe::Client,
        customer_id: &str,
        user_id: &payloads::UserId,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<String> {
        use stripe_checkout::checkout_session::{
            CreateCheckoutSession, CreateCheckoutSessionPaymentMethodTypes,
        };

        let metadata =
            HashMap::from([("user_id".to_string(), user_id.to_string())]);

        // Card only (wallets surface as card payment methods): the
        // stored method's job is off-session authorizations, which need
        // a hold-capable method. This also satisfies setup mode's
        // requirement of either explicit method types or a currency.
        let session = CreateCheckoutSession::new()
            .customer(customer_id)
            .mode(stripe_shared::CheckoutSessionMode::Setup)
            .payment_method_types(vec![
                CreateCheckoutSessionPaymentMethodTypes::Card,
            ])
            .success_url(success_url)
            .cancel_url(cancel_url)
            .metadata(metadata)
            .send(client)
            .await
            .context("Failed to create setup Checkout session")?;

        let url = session
            .url
            .ok_or_else(|| anyhow::anyhow!("Checkout session has no URL"))?;

        tracing::info!(
            session_id = %session.id,
            %user_id,
            "setup Checkout session created"
        );
        Ok(url)
    }

    /// Create a payment-mode Checkout session as a direct charge on the
    /// connected account (see the service method's docs); the mode
    /// decides capture method, platform fee, and payment method types.
    pub async fn create_payment_checkout_session(
        client: &stripe::Client,
        params: super::CheckoutSessionParams<'_>,
    ) -> Result<super::PurchaseCheckoutSession> {
        use super::SessionMode;
        use std::str::FromStr;
        use stripe_checkout::checkout_session::{
            CreateCheckoutSession, CreateCheckoutSessionLineItems,
            CreateCheckoutSessionLineItemsPriceData,
            CreateCheckoutSessionPaymentIntentData,
            CreateCheckoutSessionPaymentIntentDataCaptureMethod,
            CreateCheckoutSessionPaymentMethodTypes, ProductData,
        };

        let currency = stripe_types::Currency::from_str(params.currency)
            .expect("Currency::from_str is infallible");
        let id = params.mode.id();
        let metadata = HashMap::from([(
            params.mode.metadata_key().to_string(),
            id.clone(),
        )]);

        let mut price_data =
            CreateCheckoutSessionLineItemsPriceData::new(currency);
        price_data.product_data = Some(ProductData::new(params.description));
        price_data.unit_amount = Some(params.amount_minor);
        let mut line_item = CreateCheckoutSessionLineItems::new();
        line_item.price_data = Some(price_data);
        line_item.quantity = Some(1);

        let mut intent_data = CreateCheckoutSessionPaymentIntentData::new();
        intent_data.metadata = Some(metadata.clone());
        let mut request = CreateCheckoutSession::new()
            .mode(stripe_shared::CheckoutSessionMode::Payment)
            .success_url(params.success_url)
            .cancel_url(params.cancel_url)
            .metadata(metadata);
        match params.mode {
            SessionMode::Purchase {
                application_fee_minor,
                ..
            } => {
                intent_data.application_fee_amount = application_fee_minor;
            }
            SessionMode::Funding { .. } => {
                intent_data.capture_method = Some(
                    CreateCheckoutSessionPaymentIntentDataCaptureMethod::Manual,
                );
                // Cards only; see `SessionMode::Funding`.
                request = request.payment_method_types(vec![
                    CreateCheckoutSessionPaymentMethodTypes::Card,
                ]);
            }
        }

        let session = request
            .line_items(vec![line_item])
            .payment_intent_data(intent_data)
            .customize()
            .account_id(parse_account_id(params.account_id)?)
            .request_strategy(idempotent(&format!("{id}:checkout")))
            .send(client)
            .await
            .context("Failed to create payment Checkout session")?;

        let url = session
            .url
            .ok_or_else(|| anyhow::anyhow!("Checkout session has no URL"))?;

        tracing::info!(
            session_id = %session.id,
            kind = params.mode.kind(),
            %id,
            "payment Checkout session created"
        );
        Ok(super::PurchaseCheckoutSession {
            session_id: session.id.to_string(),
            url,
        })
    }

    /// Expire an open Checkout session on a connected account (see the
    /// service method docs). A failed expire is disambiguated by
    /// retrieving the session — its status, not the error message, says
    /// whether the session completed or was already expired.
    pub async fn expire_checkout_session(
        client: &stripe::Client,
        account_id: &str,
        session_id: &str,
    ) -> Result<super::ExpireSessionOutcome> {
        use stripe_checkout::checkout_session::{
            ExpireCheckoutSession, RetrieveCheckoutSession,
        };

        let account_id = parse_account_id(account_id)?;
        let expire_err = match ExpireCheckoutSession::new(session_id)
            .customize()
            .account_id(account_id.clone())
            .send(client)
            .await
        {
            Ok(_) => {
                tracing::info!(session_id, "Checkout session expired");
                return Ok(super::ExpireSessionOutcome::Expired);
            }
            Err(e) => e,
        };
        let session = RetrieveCheckoutSession::new(session_id)
            .customize()
            .account_id(account_id)
            .send(client)
            .await
            .context("Failed to retrieve Checkout session after expire")?;
        match session.status {
            Some(stripe_shared::CheckoutSessionStatus::Complete) => {
                tracing::info!(
                    session_id,
                    "Checkout session already completed; cannot expire"
                );
                Ok(super::ExpireSessionOutcome::Completed)
            }
            Some(stripe_shared::CheckoutSessionStatus::Expired) => {
                Ok(super::ExpireSessionOutcome::Expired)
            }
            _ => Err(anyhow::Error::from(expire_err)
                .context("Failed to expire Checkout session")),
        }
    }

    /// Retrieve a Checkout session's live state on a connected account,
    /// or None when the session doesn't exist there (see the service
    /// method docs).
    pub async fn retrieve_checkout_session(
        client: &stripe::Client,
        account_id: &str,
        session_id: &str,
    ) -> Result<Option<super::RetrievedCheckoutSession>> {
        use stripe_checkout::checkout_session::RetrieveCheckoutSession;

        let result = RetrieveCheckoutSession::new(session_id)
            .customize()
            .account_id(parse_account_id(account_id)?)
            .send(client)
            .await;
        let session = match result {
            Ok(session) => session,
            Err(e) if resource_missing(&e) => return Ok(None),
            Err(e) => {
                return Err(anyhow::Error::from(e)
                    .context("Failed to retrieve Checkout session"));
            }
        };
        let status = match session.status {
            Some(stripe_shared::CheckoutSessionStatus::Open) => {
                super::LiveSessionStatus::Open
            }
            Some(stripe_shared::CheckoutSessionStatus::Complete) => {
                super::LiveSessionStatus::Complete
            }
            Some(stripe_shared::CheckoutSessionStatus::Expired) => {
                super::LiveSessionStatus::Expired
            }
            _ => super::LiveSessionStatus::Unknown,
        };
        Ok(Some(super::RetrievedCheckoutSession {
            status,
            payment_intent_id: session
                .payment_intent
                .as_ref()
                .map(|pi| pi.id().to_string()),
        }))
    }

    /// List a customer's card payment methods, newest first (Stripe's
    /// default ordering is most recent first).
    pub async fn list_card_payment_methods(
        client: &stripe::Client,
        customer_id: &str,
    ) -> Result<Vec<super::CardPaymentMethod>> {
        use stripe_payment::payment_method::{
            ListPaymentMethod, ListPaymentMethodType,
        };

        let methods = ListPaymentMethod::new()
            .customer(customer_id)
            .type_(ListPaymentMethodType::Card)
            .send(client)
            .await
            .context("Failed to list payment methods")?;

        Ok(methods
            .data
            .into_iter()
            .filter_map(|pm| {
                let card = pm.card?;
                Some(super::CardPaymentMethod {
                    id: pm.id.to_string(),
                    brand: card.brand,
                    last4: card.last4,
                    exp_month: card.exp_month as i16,
                    exp_year: card.exp_year as i16,
                })
            })
            .collect())
    }

    /// Detach a payment method from its customer.
    pub async fn detach_payment_method(
        client: &stripe::Client,
        payment_method_id: &str,
    ) -> Result<()> {
        stripe_payment::payment_method::DetachPaymentMethod::new(
            payment_method_id
                .parse::<stripe_shared::PaymentMethodId>()
                .map_err(|_| {
                    anyhow::anyhow!(
                        "Invalid payment method id: {payment_method_id}"
                    )
                })?,
        )
        .send(client)
        .await
        .context("Failed to detach payment method")?;
        tracing::info!(payment_method_id, "payment method detached");
        Ok(())
    }

    /// Create and confirm a manual-capture authorization on a connected
    /// account (see the service method docs).
    pub async fn create_authorization(
        client: &stripe::Client,
        params: super::AuthorizationParams<'_>,
    ) -> Result<super::PaymentIntentAuth, super::StripeCallError> {
        use std::str::FromStr;

        use stripe_core::payment_intent::{
            CreatePaymentIntent, CreatePaymentIntentOffSession,
        };
        use stripe_payment::payment_method::CreatePaymentMethod;

        use super::StripeCallError;

        let account_id = params
            .connected_account_id
            .parse::<stripe_shared::AccountId>()
            .map_err(|_| {
                StripeCallError::PermanentRequest(anyhow::anyhow!(
                    "Invalid account id: {}",
                    params.connected_account_id
                ))
            })?;

        // Clone the platform payment method to the connected account
        // (direct-charges-multiple-accounts). Replays by key return the
        // same cloned method.
        let cloned = CreatePaymentMethod::new()
            .customer(params.platform_customer_id)
            .payment_method(params.platform_payment_method_id)
            .customize()
            .account_id(account_id.clone())
            .request_strategy(idempotent(&format!(
                "{}:clone",
                params.idempotency_seed
            )))
            .send(client)
            .await
            .map_err(|e| classify(e, "Failed to clone payment method"))?;

        let currency = stripe_types::Currency::from_str(params.currency)
            .expect("Currency::from_str is infallible");
        let intent = CreatePaymentIntent::new(params.amount_minor, currency)
            .capture_method(stripe_shared::PaymentIntentCaptureMethod::Manual)
            .confirm(true)
            .off_session(CreatePaymentIntentOffSession::Bool(true))
            // Off-session: a 3DS demand must fail the confirm (surfacing
            // as a card error) rather than park the intent in
            // requires_action.
            .error_on_requires_action(true)
            .payment_method(cloned.id.as_str())
            .payment_method_types(vec!["card".to_string()])
            .metadata(params.metadata)
            // Expand the charge for its per-charge capture deadline —
            // the authoritative capture window. Windows genuinely vary:
            // Visa gives merchant-initiated transactions exactly 4d18h
            // (since 2024-04) while most networks give 7d, and our
            // off-session saved-card confirms are MITs. (A 2026-07
            // test-mode probe returned 7.000d, consistent with a
            // freshly-attached card classifying as customer-initiated —
            // don't trust test mode here; read the field.)
            .expand(vec!["latest_charge".to_string()])
            .customize()
            .account_id(account_id)
            .request_strategy(idempotent(&format!(
                "{}:auth",
                params.idempotency_seed
            )))
            .send(client)
            .await
            .map_err(|e| classify(e, "Failed to confirm intent"))?;

        if intent.status != stripe_shared::PaymentIntentStatus::RequiresCapture
        {
            return Err(StripeCallError::StateConflict {
                live_status: (&intent.status).into(),
                cancellation_reason: intent
                    .cancellation_reason
                    .map(|r| r.as_str().to_string()),
            });
        }

        let capture_before_epoch = charge_capture_before(&intent.latest_charge);
        if capture_before_epoch.is_none() {
            // The fallback (7d from confirm) can overestimate a shorter
            // window; worth noticing if it starts happening.
            tracing::warn!(
                payment_intent_id = %intent.id,
                "confirmed authorization without capture_before; falling \
                 back to the standard hold window"
            );
        }

        tracing::info!(
            payment_intent_id = %intent.id,
            amount_minor = intent.amount,
            capture_before_epoch,
            "card authorization confirmed"
        );
        Ok(super::PaymentIntentAuth {
            payment_intent_id: intent.id.to_string(),
            amount_minor: intent.amount,
            capture_before_epoch,
        })
    }

    /// Cancel a PaymentIntent on a connected account (see the service
    /// method docs).
    pub async fn cancel_payment_intent(
        client: &stripe::Client,
        connected_account_id: &str,
        payment_intent_id: &str,
        idempotency_key: &str,
    ) -> Result<(), super::StripeCallError> {
        stripe_core::payment_intent::CancelPaymentIntent::new(
            parse_intent_id(payment_intent_id)
                .map_err(super::StripeCallError::PermanentRequest)?,
        )
        .customize()
        .account_id(
            parse_account_id(connected_account_id)
                .map_err(super::StripeCallError::PermanentRequest)?,
        )
        .request_strategy(idempotent(idempotency_key))
        .send(client)
        .await
        .map_err(|e| classify(e, "Failed to cancel PaymentIntent"))?;
        tracing::info!(payment_intent_id, "PaymentIntent canceled");
        Ok(())
    }

    /// Capture (part of) an authorized PaymentIntent on a connected
    /// account (see the service method docs).
    pub async fn capture_payment_intent(
        client: &stripe::Client,
        connected_account_id: &str,
        payment_intent_id: &str,
        amount_minor: i64,
        application_fee_minor: Option<i64>,
        idempotency_key: &str,
    ) -> Result<(), super::StripeCallError> {
        let mut request =
            stripe_core::payment_intent::CapturePaymentIntent::new(
                parse_intent_id(payment_intent_id)
                    .map_err(super::StripeCallError::PermanentRequest)?,
            )
            .amount_to_capture(amount_minor);
        if let Some(fee) = application_fee_minor {
            request = request.application_fee_amount(fee);
        }
        request
            .customize()
            .account_id(
                parse_account_id(connected_account_id)
                    .map_err(super::StripeCallError::PermanentRequest)?,
            )
            .request_strategy(idempotent(idempotency_key))
            .send(client)
            .await
            .map_err(|e| classify(e, "Failed to capture PaymentIntent"))?;
        tracing::info!(
            payment_intent_id,
            amount_minor,
            "PaymentIntent captured"
        );
        Ok(())
    }

    /// Retrieve a PaymentIntent's live state on a connected account, or
    /// None when it doesn't exist there (see the service method docs).
    pub async fn retrieve_payment_intent(
        client: &stripe::Client,
        connected_account_id: &str,
        payment_intent_id: &str,
    ) -> Result<Option<super::RetrievedPaymentIntent>> {
        let result = stripe_core::payment_intent::RetrievePaymentIntent::new(
            parse_intent_id(payment_intent_id)?,
        )
        // For the per-charge capture deadline, as at confirm time.
        .expand(vec!["latest_charge".to_string()])
        .customize()
        .account_id(parse_account_id(connected_account_id)?)
        .send(client)
        .await;
        let intent = match result {
            Ok(intent) => intent,
            Err(e) if resource_missing(&e) => return Ok(None),
            Err(e) => {
                return Err(anyhow::Error::from(e)
                    .context("Failed to retrieve PaymentIntent"));
            }
        };
        let capture_before_epoch = charge_capture_before(&intent.latest_charge);
        Ok(Some(super::RetrievedPaymentIntent {
            status: (&intent.status).into(),
            amount_minor: intent.amount,
            amount_capturable_minor: intent.amount_capturable,
            amount_received_minor: intent.amount_received,
            cancellation_reason: intent
                .cancellation_reason
                .map(|r| r.as_str().to_string()),
            capture_before_epoch,
        }))
    }

    /// Find the PaymentIntent on a connected account whose metadata
    /// carries `key` = `value` (see the service method docs). Pages
    /// through the created window manually via `starting_after`.
    pub async fn find_payment_intent_by_metadata(
        client: &stripe::Client,
        connected_account_id: &str,
        key: &str,
        value: &str,
        created_gte_epoch: i64,
        created_lte_epoch: i64,
    ) -> Result<Option<super::FoundPaymentIntent>> {
        let account_id = parse_account_id(connected_account_id)?;
        let created =
            stripe_types::RangeQueryTs::Bounds(stripe_types::RangeBoundsTs {
                gte: Some(created_gte_epoch),
                lte: Some(created_lte_epoch),
                ..Default::default()
            });
        let mut starting_after: Option<String> = None;
        loop {
            let mut request =
                stripe_core::payment_intent::ListPaymentIntent::new()
                    .created(created)
                    .limit(100);
            if let Some(after) = &starting_after {
                request = request.starting_after(after.clone());
            }
            let page = request
                .customize()
                .account_id(account_id.clone())
                .send(client)
                .await
                .context("Failed to list PaymentIntents")?;
            let last_id = page.data.last().map(|i| i.id.to_string());
            for intent in page.data {
                if intent.metadata.get(key).is_some_and(|v| v == value) {
                    return Ok(Some(super::FoundPaymentIntent {
                        payment_intent_id: intent.id.to_string(),
                        status: (&intent.status).into(),
                        cancellation_reason: intent
                            .cancellation_reason
                            .map(|r| r.as_str().to_string()),
                    }));
                }
            }
            match (page.has_more, last_id) {
                (true, Some(id)) => starting_after = Some(id),
                _ => return Ok(None),
            }
        }
    }

    fn parse_account_id(id: &str) -> Result<stripe_shared::AccountId> {
        id.parse()
            .map_err(|_| anyhow::anyhow!("Invalid account id: {id}"))
    }

    fn idempotent(key: &str) -> stripe::RequestStrategy {
        stripe::RequestStrategy::Idempotent(
            stripe::IdempotencyKey::new(key)
                .expect("idempotency keys are non-empty and short"),
        )
    }

    /// Whether a request was rejected because the object doesn't exist
    /// on the account (`resource_missing`).
    fn resource_missing(e: &stripe::StripeError) -> bool {
        matches!(
            e,
            stripe::StripeError::Stripe(api_errors, _)
                if matches!(
                    api_errors.code,
                    Some(stripe::ApiErrorsCode::ResourceMissing)
                )
        )
    }

    /// Classify a failed account-scoped intent call into the
    /// [`super::StripeCallError`] taxonomy — the one place error shape
    /// is interpreted, so every call site handles the same set of
    /// classes. See the enum docs for the classes; the notable
    /// mappings: card errors (including the `authentication_required`
    /// 3DS demand under `error_on_requires_action`) become `Declined`;
    /// `payment_intent_unexpected_state` embeds the current object, so
    /// `StateConflict` needs no extra retrieve; rate limits and lock
    /// timeouts are transient even though Stripe types them
    /// `invalid_request_error`.
    fn classify(
        e: stripe::StripeError,
        context: &'static str,
    ) -> super::StripeCallError {
        use super::StripeCallError as E;

        let stripe::StripeError::Stripe(ref api_errors, status) = e else {
            // Transport, timeout, deserialize, config: nothing
            // conclusive happened at Stripe; retry on backoff.
            return E::Transient(anyhow::Error::from(e).context(context));
        };

        if api_errors.type_ == stripe::ApiErrorsType::CardError {
            return E::Declined(super::DeclineInfo {
                code: api_errors.code.as_ref().map(|c| c.as_str().to_string()),
                decline_code: api_errors.decline_code.clone(),
                message: api_errors.message.clone(),
                payment_intent_id: api_errors
                    .payment_intent
                    .as_ref()
                    .map(|pi| pi.id.to_string()),
            });
        }
        if matches!(
            api_errors.code,
            Some(stripe::ApiErrorsCode::PaymentIntentUnexpectedState)
        ) {
            let (live_status, cancellation_reason) =
                match api_errors.payment_intent.as_ref() {
                    Some(pi) => (
                        (&pi.status).into(),
                        pi.cancellation_reason
                            .as_ref()
                            .map(|r| r.as_str().to_string()),
                    ),
                    None => (super::LivePiStatus::Unknown, None),
                };
            return E::StateConflict {
                live_status,
                cancellation_reason,
            };
        }
        if api_errors
            .message
            .as_deref()
            .is_some_and(super::is_account_gone_message)
        {
            return E::AccountGone(anyhow::Error::from(e).context(context));
        }
        if matches!(
            api_errors.code,
            Some(
                stripe::ApiErrorsCode::RateLimit
                    | stripe::ApiErrorsCode::LockTimeout
            )
        ) || status == 429
            || status >= 500
        {
            return E::Transient(anyhow::Error::from(e).context(context));
        }
        match api_errors.type_ {
            stripe::ApiErrorsType::InvalidRequestError
            | stripe::ApiErrorsType::IdempotencyError => {
                E::PermanentRequest(anyhow::Error::from(e).context(context))
            }
            _ => E::Transient(anyhow::Error::from(e).context(context)),
        }
    }

    /// The per-charge capture deadline off an expanded `latest_charge`
    /// (`payment_method_details.card.capture_before`), when present.
    fn charge_capture_before(
        latest_charge: &Option<stripe_types::Expandable<stripe_shared::Charge>>,
    ) -> Option<i64> {
        match latest_charge {
            Some(stripe_types::Expandable::Object(charge)) => charge
                .payment_method_details
                .as_ref()
                .and_then(|d| d.card.as_ref())
                .and_then(|c| c.capture_before),
            _ => None,
        }
    }

    fn parse_intent_id(id: &str) -> Result<stripe_shared::PaymentIntentId> {
        id.parse()
            .map_err(|_| anyhow::anyhow!("Invalid payment intent id: {id}"))
    }
}

#[cfg(test)]
mod tests {
    use hmac::{Hmac, Mac};
    use secrecy::SecretBox;
    use sha2::Sha256;

    use super::verify_signed_payload;
    use crate::time::TimeSource;

    const PAYLOAD: &str = r#"{"id":"evt_test"}"#;

    fn secret(s: &str) -> SecretBox<String> {
        SecretBox::new(Box::new(s.to_string()))
    }

    fn time_source() -> TimeSource {
        #[cfg(feature = "mock-time")]
        return TimeSource::new(
            jiff::Timestamp::from_second(1_700_000_000).unwrap(),
        );
        #[cfg(not(feature = "mock-time"))]
        TimeSource::new()
    }

    fn sign(secret: &str, timestamp: i64, payload: &str) -> String {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(format!("{timestamp}.{payload}").as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// During secret rotation Stripe signs with both the old and new
    /// secrets, sending one v1 entry per secret. A matching entry must
    /// be accepted regardless of position; the first-position case is
    /// the regression test for the old last-v1-wins parsing.
    #[test]
    fn rotation_accepts_matching_v1_in_any_position() {
        let time_source = time_source();
        let ts = time_source.now().as_second();
        let good = sign("whsec_current", ts, PAYLOAD);
        let other = sign("whsec_rotated_away", ts, PAYLOAD);

        for header in [
            format!("t={ts},v1={good},v1={other}"),
            format!("t={ts},v1={other},v1={good}"),
        ] {
            let parsed = verify_signed_payload(
                &secret("whsec_current"),
                PAYLOAD,
                &header,
                &time_source,
            )
            .expect("matching v1 entry should verify");
            assert_eq!(parsed["id"], "evt_test");
        }
    }

    #[test]
    fn rejects_when_no_v1_matches() {
        let time_source = time_source();
        let ts = time_source.now().as_second();
        let wrong = sign("whsec_wrong", ts, PAYLOAD);
        let header = format!("t={ts},v1={wrong},v1=nothex");

        let err = verify_signed_payload(
            &secret("whsec_current"),
            PAYLOAD,
            &header,
            &time_source,
        )
        .expect_err("no entry should verify");
        assert!(err.to_string().contains("Bad signature"));
    }
}
