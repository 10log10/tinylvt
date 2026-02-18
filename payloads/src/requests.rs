use crate::{CommunityId, CurrencySettings};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub const EMAIL_MAX_LEN: usize = 255;
pub const USERNAME_MIN_LEN: usize = 3;
pub const USERNAME_MAX_LEN: usize = 30;
pub const DISPLAY_NAME_MAX_LEN: usize = 255;

/// Validation result for usernames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsernameValidation {
    Valid,
    TooShort,
    TooLong,
    InvalidCharacters,
    MustStartWithLetter,
}

impl UsernameValidation {
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    pub fn error_message(&self) -> Option<&'static str> {
        match self {
            Self::Valid => None,
            Self::TooShort => Some("Username must be at least 3 characters"),
            Self::TooLong => Some("Username must be at most 30 characters"),
            Self::InvalidCharacters => Some(
                "Username can only contain letters, numbers, and underscores",
            ),
            Self::MustStartWithLetter => {
                Some("Username must start with a letter")
            }
        }
    }
}

/// Validate a username.
///
/// Rules:
/// - 3-30 characters
/// - ASCII letters, numbers, and underscores only
/// - Must start with a letter
pub fn validate_username(username: &str) -> UsernameValidation {
    if username.len() < USERNAME_MIN_LEN {
        return UsernameValidation::TooShort;
    }
    if username.len() > USERNAME_MAX_LEN {
        return UsernameValidation::TooLong;
    }

    let mut chars = username.chars();

    // First character must be a letter
    if let Some(first) = chars.next()
        && !first.is_ascii_alphabetic()
    {
        return UsernameValidation::MustStartWithLetter;
    }

    // Rest must be alphanumeric or underscore
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return UsernameValidation::InvalidCharacters;
        }
    }

    UsernameValidation::Valid
}

#[derive(Serialize, Deserialize)]
pub struct LoginCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateAccount {
    pub email: String,
    pub username: String,
    pub password: String,
}

pub const COMMUNITY_NAME_MAX_LEN: usize = 255;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCommunity {
    pub name: String,
    pub currency: CurrencySettings,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCurrencyConfig {
    pub community_id: CommunityId,
    pub currency: CurrencySettings,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InviteCommunityMember {
    pub community_id: CommunityId,
    pub new_member_email: Option<String>,
    pub single_use: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteInvite {
    pub community_id: CommunityId,
    pub invite_id: crate::InviteId,
}

/// An empty schedule can be used to delete the schedule entirely.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetMembershipSchedule {
    pub community_id: CommunityId,
    pub schedule: Vec<crate::MembershipSchedule>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveMember {
    pub community_id: CommunityId,
    pub member_user_id: crate::UserId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeMemberRole {
    pub community_id: CommunityId,
    pub member_user_id: crate::UserId,
    pub new_role: crate::Role,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeaveCommunity {
    pub community_id: CommunityId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrphanedAccounts {
    pub community_id: CommunityId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveOrphanedBalance {
    pub community_id: CommunityId,
    pub orphaned_account_id: crate::AccountId,
    pub note: Option<String>,
    pub idempotency_key: crate::IdempotencyKey,
}

/// Details about a community member for a community one is a part of.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSite {
    pub site_id: crate::SiteId,
    pub site_details: crate::Site,
}

/// Details about a community member for a community one is a part of.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSpace {
    pub space_id: crate::SpaceId,
    pub space_details: crate::Space,
}

/// Batch update multiple spaces at once
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSpaces {
    pub spaces: Vec<UpdateSpace>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserValue {
    pub space_id: crate::SpaceId,
    pub value: Decimal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UseProxyBidding {
    pub auction_id: crate::AuctionId,
    pub max_items: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ForgotPassword {
    pub email: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ResetPassword {
    pub token: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ResendVerificationEmail {
    pub email: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VerifyEmail {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateProfile {
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSiteImage {
    pub community_id: CommunityId,
    pub name: String,
    pub image_data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSiteImage {
    pub id: crate::SiteImageId,
    pub name: Option<String>,
    pub image_data: Option<Vec<u8>>,
}

// Currency operations

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCreditLimitOverride {
    pub community_id: crate::CommunityId,
    pub member_user_id: crate::UserId,
    pub credit_limit_override: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMemberCreditLimitOverride {
    pub community_id: crate::CommunityId,
    pub member_user_id: crate::UserId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemberActiveStatus {
    pub community_id: crate::CommunityId,
    pub member_user_id: crate::UserId,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetMemberCurrencyInfo {
    pub community_id: crate::CommunityId,
    /// If None, returns info for the authenticated user.
    /// If Some, returns info for specified user (coleader+ only).
    pub member_user_id: Option<crate::UserId>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetMemberTransactions {
    pub community_id: crate::CommunityId,
    /// If None, returns transactions for the authenticated user.
    /// If Some, returns transactions for specified user (coleader+ only).
    pub member_user_id: Option<crate::UserId>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTransfer {
    pub community_id: crate::CommunityId,
    pub to_user_id: crate::UserId,
    pub amount: Decimal,
    pub note: Option<String>,
    pub idempotency_key: crate::IdempotencyKey,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTreasuryAccount {
    pub community_id: crate::CommunityId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTreasuryTransactions {
    pub community_id: crate::CommunityId,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TreasuryCreditOperation {
    pub community_id: crate::CommunityId,
    pub recipient: crate::TreasuryRecipient,
    pub amount_per_recipient: Decimal,
    pub note: Option<String>,
    pub idempotency_key: crate::IdempotencyKey,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResetAllBalances {
    pub community_id: crate::CommunityId,
    pub note: Option<String>,
}
