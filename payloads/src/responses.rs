use crate::{CommunityId, InviteId, UserId};
use jiff::Timestamp;
#[cfg(feature = "use-sqlx")]
use jiff_sqlx::Timestamp as SqlxTs;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// User identification bundled with display information
///
/// This is the standard way to reference users in API responses.
/// The frontend should display display_name (if present) or username,
/// but use user_id for any API calls that reference the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
pub struct UserIdentity {
    pub user_id: UserId,
    pub username: String,
    /// Community-specific display name (if set for this community)
    pub display_name: Option<String>,
}

/// Summary of a [`crate::requests::BulkActivateMembers`] operation.
///
/// `unmatched` echoes back only identifiers the caller supplied, so it does
/// not disclose any member's email.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkActivateMembersResult {
    /// Number of members set to active. Includes members that were already
    /// active but named in the list.
    pub activated_count: usize,
    /// Identifiers from the request that matched no member of this community.
    pub unmatched: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Community {
    pub id: CommunityId,
    pub name: String,
    pub description: Option<String>,
    pub community_image_id: Option<crate::SiteImageId>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub currency: crate::CurrencySettings,
    /// Whether the community can charge cards (backed_credits mode
    /// with a connected, charges-enabled, non-deauthorized Stripe
    /// account) — gates member-visible card surfaces like credit
    /// purchases.
    pub card_payments_enabled: bool,
}

/// A community invite that has been issued from a given community.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
pub struct IssuedCommunityInvite {
    pub id: InviteId,
    pub new_member_email: Option<String>,
    pub single_use: bool,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
    pub created_at: Timestamp,
}

/// Details about a community invite, excluding the target community id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
pub struct CommunityInviteReceived {
    pub id: InviteId,
    pub community_name: String,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
    pub created_at: Timestamp,
}

/// Details about a community member for a community one is a part of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityMember {
    pub user: UserIdentity,
    pub role: crate::Role,
    pub is_active: bool,
    /// Balance is included if user is coleader+ or
    /// balances_visible_to_members is true
    pub balance: Option<rust_decimal::Decimal>,
}

/// Community information with the current user's role in that community.
/// This is used by the get_communities endpoint to provide role information
/// so the frontend can show/hide controls based on permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityWithRole {
    pub community: Community,
    /// The current user's role in this community
    pub user_role: crate::Role,
    /// Whether the current user is active in this community
    pub user_is_active: bool,
}

impl std::ops::Deref for CommunityWithRole {
    type Target = Community;

    fn deref(&self) -> &Self::Target {
        &self.community
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrphanedAccount {
    pub account: crate::Account,
    pub previous_owner: Option<UserIdentity>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrphanedAccountsList {
    pub orphaned_accounts: Vec<OrphanedAccount>,
}

/// Details about a community member for a community one is a part of.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub site_id: crate::SiteId,
    pub site_details: crate::Site,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub deleted_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Space {
    pub space_id: crate::SpaceId,
    pub space_details: crate::Space,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub deleted_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateSpaceResult {
    pub space: Space,
    /// True if copy-on-write was performed (space had auction history +
    /// nontrivial changes)
    pub was_copied: bool,
    /// If was_copied is true, this contains the old space ID that was
    /// soft-deleted
    pub old_space_id: Option<crate::SpaceId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Auction {
    pub auction_id: crate::AuctionId,
    pub auction_details: crate::Auction,
    pub end_at: Option<Timestamp>,
    pub was_canceled: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Auction {
    /// Derive the auction's lifecycle status at the given time. This is the
    /// single source of truth for status display across the UI.
    pub fn status(&self, now: Timestamp) -> crate::AuctionStatus {
        use crate::AuctionStatus;
        if self.was_canceled {
            AuctionStatus::Canceled
        } else if self.end_at.is_some() {
            AuctionStatus::Concluded
        } else {
            match self.auction_details.start_at {
                None => AuctionStatus::NotScheduled,
                Some(start_at) if start_at <= now => AuctionStatus::Ongoing,
                Some(_) => AuctionStatus::Upcoming,
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuctionRound {
    pub round_id: crate::AuctionRoundId,
    pub round_details: crate::AuctionRound,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserValue {
    pub space_id: crate::SpaceId,
    pub value: Decimal,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UseProxyBidding {
    pub auction_id: crate::AuctionId,
    pub max_items: i32,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: UserId,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub email_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessMessage {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
pub struct SiteImage {
    pub id: crate::SiteImageId,
    pub community_id: crate::CommunityId,
    pub name: String,
    pub image_data: Vec<u8>,
    pub mime_type: String,
    pub file_size: i64,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
    pub created_at: Timestamp,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
    pub updated_at: Timestamp,
}

/// Lightweight site image info without the actual image data.
/// Used for listing images where the actual data is fetched via URL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
pub struct SiteImageInfo {
    pub id: crate::SiteImageId,
    pub community_id: crate::CommunityId,
    pub name: String,
    pub mime_type: String,
    pub file_size: i64,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
    pub created_at: Timestamp,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
    pub updated_at: Timestamp,
}

/// Display metadata for a user's saved card; the card itself lives in
/// Stripe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedCard {
    pub brand: String,
    pub last4: String,
    pub exp_month: i16,
    pub exp_year: i16,
}

/// The user's platform-wide payment settings: their saved card (if
/// any) and authorization sizing strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPaymentProfile {
    pub card: Option<SavedCard>,
    /// TRUE = budget holds (authorizations cover the member's auction
    /// budget); FALSE = minimum start with catch-up-or-double raises.
    pub budget_holds: bool,
}

/// A community's Stripe Connect standing (stripe-backed backed_credits
/// mode). `settlement_currency_mismatch` carries the connected account's
/// settlement currency (lowercase ISO code) when it doesn't match the
/// community's denomination — surfaced so the coleader can fix the
/// account before charges run in the wrong currency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommunityStripeStatus {
    pub status: crate::StripeConnectStatus,
    pub settlement_currency_mismatch: Option<String>,
}

/// The requesting member's funding state in one auction (stripe-backed
/// backed_credits mode). `commitment` is the derived amount their
/// standing/pending bids commit; `balance_backing` is the balance available to
/// this auction (balance plus pending captures, less other live auctions'
/// balance commitments — the bid gate's balance term); `authorized` is the live
/// card authorization's amount (zero when none), expiring at `capture_before`.
/// Total headroom = balance_backing + authorized − commitment. `card_available`
/// reports whether the card path can extend backing here (community
/// charges-enabled, card saved, charges granted) — the funding-regime
/// indicator's input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuctionFunding {
    pub balance_backing: Decimal,
    pub commitment: Decimal,
    pub authorized: Decimal,
    pub capture_before: Option<Timestamp>,
    pub card_available: CardAvailability,
    /// Amount of an open unsaved-card Checkout authorization awaiting
    /// the member's payment (its completion webhook activates it), for
    /// the "authorization in progress" display after the redirect
    /// return.
    pub checkout_pending: Option<Decimal>,
    /// The member's post-settlement card charge for this auction, present
    /// once conclusion has marked their authorization for capture:
    /// the amount and where it stands (`CapturePending` while the worker
    /// completes, then `Captured`, or `Failed` if it will never be
    /// collected — the amount then remains due on their balance).
    pub capture: Option<CaptureState>,
    /// Set while the auction is live and the member's latest card-hold
    /// attempt was declined: automatic holds are paused (the scheduler
    /// skips the member until a new authorization succeeds), so the UI
    /// surfaces the pause and offers the unsaved-card checkout as the
    /// recovery path.
    pub decline_pause: Option<FundingDeclinePause>,
    /// What a strategy-sized "place hold now" would hold right now — the
    /// same sizing the pre-authorize endpoint computes for a request
    /// without an explicit amount, surfaced so the button can preview
    /// its effect (raise, reduce, or already covered). None once the
    /// auction has ended or when the target is zero.
    pub preauth_target: Option<Decimal>,
}

/// A recorded decline pausing automatic card holds (see
/// [`AuctionFunding::decline_pause`]). `code` is the Stripe decline or
/// error code when one was reported — `authentication_required` means
/// the issuer wants 3DS, which only the checkout flow can run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundingDeclinePause {
    pub code: Option<String>,
}

/// A settlement capture's amount and progress (see
/// [`AuctionFunding::capture`]).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CaptureState {
    pub amount: Decimal,
    pub status: crate::FundingIntentStatus,
}

/// Whether card-backed bidding can operate for this member in this
/// community, and if not, the first missing prerequisite (the UI prompts
/// for it in-context).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardAvailability {
    Available,
    /// The community has no charges-enabled Stripe account; bidding is
    /// balance-only for everyone.
    CommunityNotChargesEnabled,
    /// The member has no saved card.
    NoSavedCard,
    /// The member hasn't granted this community permission to charge
    /// their card.
    NotGranted,
}

/// Currency information for a member account
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberCurrencyInfo {
    pub account_id: crate::AccountId,
    pub balance: Decimal,
    pub credit_limit: Option<Decimal>,
    pub commitment: Decimal,
    pub available_credit: Option<Decimal>,
    /// Settlement captures still being collected (backed_credits mode;
    /// zero elsewhere). These are expected to settle, so the member's
    /// effective balance is `balance + pending_captures` — a negative
    /// balance is only real debt to repay beyond that sum.
    pub pending_captures: Decimal,
}

/// A credit purchase for member display: the pending list (a Checkout
/// payment still settling, e.g. ACH) and purchase history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreditPurchase {
    pub id: crate::CreditPurchaseId,
    pub kind: crate::PurchaseKind,
    pub status: crate::PurchaseStatus,
    pub amount: Decimal,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberCreditLimitOverride {
    pub credit_limit_override: Option<Decimal>,
}

/// Represents a participant in a transaction (member or treasury)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionParty {
    Member(UserIdentity),
    Treasury,
}

/// A line in a transaction showing who sent/received currency
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionLine {
    pub party: TransactionParty,
    /// Positive = received, Negative = sent
    pub amount: Decimal,
}

/// Transaction history entry for display to members
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberTransaction {
    pub entry_type: crate::EntryType,
    pub auction_id: Option<crate::AuctionId>,
    pub note: Option<String>,
    pub created_at: Timestamp,
    /// Lines in the transaction relevant to the requesting user
    /// (typically shows who they sent to or received from)
    pub lines: Vec<TransactionLine>,
}

/// Result of resetting all member balances to zero
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceResetResult {
    /// Number of member accounts affected
    pub accounts_reset: usize,
    /// Total amount transferred to treasury
    pub total_transferred: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformStats {
    pub auctions_held: i64,
    pub spaces_allocated: i64,
}
