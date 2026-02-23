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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Community {
    pub id: CommunityId,
    pub name: String,
    pub description: Option<String>,
    pub community_image_id: Option<crate::SiteImageId>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub currency: crate::CurrencySettings,
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
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub balance: Decimal,
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
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
    pub created_at: Timestamp,
    #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
    pub updated_at: Timestamp,
}

/// Currency information for a member account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberCurrencyInfo {
    pub account_id: crate::AccountId,
    pub balance: Decimal,
    pub credit_limit: Option<Decimal>,
    pub locked_balance: Decimal,
    pub available_credit: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
