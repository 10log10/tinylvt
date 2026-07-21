//! Client-facing API error variants, serialized as JSON in error response
//! bodies so clients can match on exact variants rather than status codes
//! or display strings.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{AuctionParamsError, PermissionLevel};

/// A client-facing API error. The server serializes this as the error
/// response body; the client deserializes it back so callers can match on
/// the exact variant. The adjacent tagging gives every body a uniform
/// shape: `{"code": "...", "details": ...}`, with `details` only present
/// for variants that carry data.
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "code", content = "details")]
pub enum ApiError {
    #[error("Invalid username: {0}")]
    InvalidUsername(String),
    #[error("That username is already taken")]
    UsernameTaken,
    #[error("Invalid email: {0}")]
    InvalidEmail(String),
    #[error("An account with that email already exists")]
    EmailTaken,
    #[error("Invalid password: {0}")]
    InvalidPassword(String),
    #[error("Email not yet verified")]
    UnverifiedEmail,
    #[error("Moderator permissions required")]
    RequiresModeratorPermissions,
    #[error("Coleader permissions required")]
    RequiresColeaderPermissions,
    #[error("Leader permissions required")]
    RequiresLeaderPermissions,
    #[error("Cannot delete space with auction history")]
    SpaceHasAuctionHistory,
    #[error("Mismatched invite email")]
    MismatchedInviteEmail,
    #[error("Field too long")]
    FieldTooLong,
    #[error("Invalid invite")]
    InvalidInvite,
    #[error("Already a member of this community")]
    AlreadyMember,
    #[error("Member not found")]
    MemberNotFound,
    #[error("Cannot remove yourself from community")]
    CannotRemoveSelf,
    #[error("Cannot remove user with higher role")]
    CannotRemoveHigherRole,
    #[error("Cannot change role of this user")]
    CannotChangeRole,
    #[error("Cannot change own role")]
    CannotChangeSelfRole,
    #[error("Cannot promote to leader")]
    CannotPromoteToLeader,
    #[error(
        "Cannot leave community as leader (must transfer leadership first)"
    )]
    LeaderMustTransferFirst,
    #[error("Orphaned account not found")]
    OrphanedAccountNotFound,
    #[error(
        "Cannot resolve orphaned account with locked balance from \
         outstanding bids"
    )]
    OrphanedAccountHasLockedBalance,
    #[error("No active members to distribute balance to")]
    NoActiveMembersForDistribution,
    #[error("Span too large: {0}")]
    SpanTooLarge(String),
    #[error("A space with the name '{name}' already exists in this site")]
    SpaceNameNotUnique { name: String },
    #[error("Insufficient permissions. Required: {required:?}")]
    InsufficientPermissions { required: PermissionLevel },
    #[error("Auction not found")]
    AuctionNotFound,
    #[error("Auction has already started")]
    AuctionAlreadyStarted,
    #[error("Auction has already ended")]
    AuctionAlreadyEnded,
    #[error("Only canceled auctions can be permanently deleted")]
    AuctionNotCanceled,
    #[error("Auction start time must be in the future")]
    AuctionStartNotInFuture,
    #[error("Auction start time cannot be in the past")]
    AuctionStartInPast,
    #[error("Possession start must be before possession end")]
    InvalidPossessionPeriod,
    #[error("Invalid auction parameters: {0}")]
    InvalidAuctionParams(AuctionParamsError),
    #[error("Round space result not found")]
    RoundSpaceResultNotFound,
    #[error("Bid not found")]
    BidNotFound,
    #[error("Round has ended")]
    RoundEnded,
    #[error("Auction round not found")]
    AuctionRoundNotFound,
    #[error("Round has not started yet")]
    RoundNotStarted,
    #[error("User not found")]
    UserNotFound,
    #[error("Community not found")]
    CommunityNotFound,
    #[error("Site not found")]
    SiteNotFound,
    #[error("Space not found")]
    SpaceNotFound,
    #[error("Site image not found")]
    SiteImageNotFound,
    #[error("Image too large. Maximum size is 1MB, received {size} bytes")]
    ImageTooLarge { size: usize },
    #[error(
        "Invalid image format. File must be a valid image (JPEG, PNG, etc.)"
    )]
    InvalidImageFormat,
    #[error(
        "Site description too long. Maximum is {max} characters, received {size}"
    )]
    SiteDescriptionTooLong { size: usize, max: usize },
    #[error(
        "Space description too long. Maximum is {max} characters, received {size}"
    )]
    SpaceDescriptionTooLong { size: usize, max: usize },
    #[error(
        "Community description too long. Maximum is {max} characters, received {size}"
    )]
    CommunityDescriptionTooLong { size: usize, max: usize },
    #[error("Site name too long. Maximum is {max} characters, received {size}")]
    SiteNameTooLong { size: usize, max: usize },
    #[error(
        "Space name too long. Maximum is {max} characters, received {size}"
    )]
    SpaceNameTooLong { size: usize, max: usize },
    #[error("Eligibility points must be a finite, non-negative number")]
    InvalidEligibilityPoints,
    #[error(
        "Journal note too long. Maximum is {max} characters, received {size}"
    )]
    JournalNoteTooLong { size: usize, max: usize },
    #[error("Invalid timezone: {timezone}")]
    InvalidTimezone { timezone: String },
    #[error("Community invite not found")]
    CommunityInviteNotFound,
    #[error("Open hours not found")]
    OpenHoursNotFound,
    #[error("Auction params not found")]
    AuctionParamsNotFound,
    #[error(
        "Exceeds eligibility. Available: {available}, Required: {required}"
    )]
    ExceedsEligibility { available: f64, required: f64 },
    #[error("Cannot bid on a space you are already winning")]
    AlreadyWinningSpace,
    #[error("Space is not available for bidding")]
    SpaceNotAvailable,
    #[error("Space has been deleted")]
    SpaceDeleted,
    #[error("Site has been deleted")]
    SiteDeleted,
    #[error("User value not found")]
    UserValueNotFound,
    #[error("Proxy bidding settings not found")]
    ProxyBiddingNotFound,
    #[error("Token not found")]
    TokenNotFound,
    #[error("Invalid token action")]
    InvalidTokenAction,
    #[error("Token already used")]
    TokenAlreadyUsed,
    #[error("Token expired")]
    TokenExpired,
    #[error("Cannot delete user who is a leader of a community")]
    UserIsLeader,
    #[error("Account not found")]
    AccountNotFound,
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error(
        "Amount has finer resolution than the currency's {minor_units} minor \
         units"
    )]
    AmountNotQuantized { minor_units: i16 },
    #[error(
        "Spaces have reserve prices finer than the currency's {minor_units} \
         minor units: {space_names}"
    )]
    UnquantizedReservePrices {
        minor_units: i16,
        space_names: String,
    },
    #[error("Amount must be positive")]
    AmountMustBePositive,
    #[error("Amount must be non-zero")]
    AmountMustBeNonZero,
    #[error(
        "Negative amounts are only allowed for distribution corrections \
         in DistributedClearing mode targeting all active members"
    )]
    NegativeTreasuryAmountNotAllowed,
    #[error("Invalid treasury operation for this currency mode")]
    InvalidTreasuryOperation,
    #[error("Invalid credit limit operation for this currency mode")]
    InvalidCreditLimitOperation,
    #[error("Currency mode cannot be changed after community creation")]
    CurrencyModeImmutable,
    #[error("This mode is under construction")]
    CurrencyModeUnderConstruction,
    #[error("Invalid currency name (max 50 characters)")]
    InvalidCurrencyName,
    #[error("Invalid currency symbol (max 5 characters)")]
    InvalidCurrencySymbol,
    #[error("Journal entry lines must sum to zero, got {0}")]
    JournalLinesDoNotSumToZero(Decimal),
    #[error("Duplicate account in journal entry")]
    DuplicateAccountInJournalEntry,
    #[error("Cannot reset balances while auctions are active")]
    CannotResetDuringActiveAuction,
    #[error("Cannot delete site with financial history")]
    SiteHasFinancialHistory,
    #[error("Community already has an active subscription")]
    AlreadySubscribed,
    #[error("No subscription found for this community")]
    NoSubscriptionFound,
    #[error(
        "Subscription payment is past due. \
         Please update your payment method."
    )]
    SubscriptionPastDue,
    #[error(
        "Storage limit exceeded. Current: {current} bytes, limit: {limit} \
         bytes, estimated after operation: {estimated_size_after_operation} \
         bytes"
    )]
    StorageLimitExceeded {
        current: i64,
        limit: i64,
        estimated_size_after_operation: i64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(err: &ApiError) -> ApiError {
        let json = serde_json::to_string(err).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn json_round_trip() {
        // unit variant
        let unit = ApiError::InsufficientBalance;
        assert_eq!(
            serde_json::to_value(&unit).unwrap(),
            serde_json::json!({"code": "InsufficientBalance"})
        );
        assert_eq!(round_trip(&unit), unit);

        // newtype variant
        let newtype = ApiError::InvalidUsername("bad name".into());
        assert_eq!(
            serde_json::to_value(&newtype).unwrap(),
            serde_json::json!({
                "code": "InvalidUsername",
                "details": "bad name",
            })
        );
        assert_eq!(round_trip(&newtype), newtype);

        // struct variant
        let strct = ApiError::ImageTooLarge { size: 123 };
        assert_eq!(
            serde_json::to_value(&strct).unwrap(),
            serde_json::json!({
                "code": "ImageTooLarge",
                "details": {"size": 123},
            })
        );
        assert_eq!(round_trip(&strct), strct);
    }
}
