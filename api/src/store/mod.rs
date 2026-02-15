//! Database store module for TinyLVT API
//!
//! ## Design Decisions
//!
//! ### Token Management
//! - **Auto-generated UUIDs**: The database automatically generates UUIDs
//!   for tokens using `DEFAULT gen_random_uuid()`. This ensures consistent
//!   UUID generation and reduces network overhead.
//! - **Single-use tokens**: All tokens (email verification, password
//!   reset) are marked as used after consumption and cannot be reused.
//! - **Time-based expiration**: Tokens have database-enforced expiration
//!   times. Email verification tokens expire after 24 hours, password
//!   reset tokens after 1 hour.
//!
//! ### Time Source Dependency
//! - **Mocked time for testing**: Functions that need current time
//!   (`consume_token`, `cleanup_expired_tokens`) accept a `TimeSource`
//!   parameter instead of creating their own. This allows time to be
//!   mocked during tests.
//! - **Consistent time handling**: All time-sensitive operations use the
//!   same `TimeSource` instance passed from the application routes.
//!
//! ### Database Triggers
//! - **Auto-updated timestamps**: The database has triggers that
//!   automatically update `updated_at` fields, so application code doesn't
//!   need to manually set these values.
//! - **Consistent audit trail**: All modifications are tracked at the
//!   database level for reliability.
//!
//! ### Type Safety
//! - **TokenId with sqlx::Type**: TokenId implements sqlx::Type, so it
//!   can be used directly with sqlx queries without accessing the inner
//!   UUID value (`.0`).
//! - **UserId binding**: Similar pattern for all ID types to ensure type
//!   safety at the query level.

use derive_more::Display;
use jiff::Span;
use jiff::{Timestamp, civil::Time};
use jiff_sqlx::{Span as SqlxSpan, Timestamp as SqlxTs};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{FromRow, Type};
use sqlx_postgres::types::PgInterval;
use uuid::Uuid;

use payloads::{
    AuctionId, AuctionRoundId, CommunityId, InviteId, OptionalTimestamp,
    PermissionLevel, Role, SiteId, SiteImageId, SpaceId, UserId,
    responses::{self, Community},
};

use crate::time::TimeSource;

pub mod auction;
pub mod community;
pub mod currency;
pub mod login;
pub mod proxy_bidding;
pub mod site;
pub mod space;

pub use auction::*;
pub use community::*;
pub use login::*;
pub use proxy_bidding::*;
pub use site::*;
pub use space::*;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize, Type,
)]
#[sqlx(type_name = "token_action", rename_all = "snake_case")]
pub enum TokenAction {
    EmailVerification,
    PasswordReset,
}

impl From<Space> for payloads::Space {
    fn from(space: Space) -> Self {
        Self {
            site_id: space.site_id,
            name: space.name,
            description: space.description,
            eligibility_points: space.eligibility_points,
            is_available: space.is_available,
            site_image_id: space.site_image_id,
        }
    }
}

impl From<Space> for payloads::responses::Space {
    fn from(space: Space) -> Self {
        Self {
            space_id: space.id,
            created_at: space.created_at,
            updated_at: space.updated_at,
            deleted_at: space.deleted_at,
            space_details: space.into(),
        }
    }
}

/// A complete user row that stays in the backend.
#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub email_verified: bool,
    pub balance: Decimal,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
    #[sqlx(try_from = "OptionalTimestamp")]
    pub deleted_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, sqlx::Type, FromRow)]
#[sqlx(transparent)]
pub struct TokenId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct Token {
    pub id: TokenId,
    pub user_id: UserId,
    pub action: TokenAction,
    pub used: bool,
    #[sqlx(try_from = "SqlxTs")]
    pub expires_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, FromRow)]
pub struct CommunityMember {
    pub community_id: CommunityId,
    pub user_id: UserId,
    pub role: Role,
    pub is_active: bool,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

/// A type that can only exist if the interior CommunityMember has been
/// validated to exist.
pub struct ValidatedMember(CommunityMember);

#[derive(Debug, Clone, FromRow)]
pub struct CommunityInvite {
    pub id: InviteId,
    pub community_id: CommunityId,
    pub email: Option<String>,
    pub single_use: bool,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, sqlx::FromRow)]
#[sqlx(transparent)]
pub struct CommunityMembershipScheduleId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct CommunityMembershipSchedule {
    pub id: CommunityMembershipScheduleId,
    pub community_id: CommunityId,
    #[sqlx(try_from = "SqlxTs")]
    pub start_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub end_at: Timestamp,
    pub email: String,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, sqlx::FromRow)]
#[sqlx(transparent)]
pub struct AuctionParamsId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct AuctionParams {
    pub id: AuctionParamsId,
    #[sqlx(try_from = "SqlxSpan")]
    pub round_duration: Span,
    pub bid_increment: Decimal,
    pub activity_rule_params: Json<payloads::ActivityRuleParams>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

impl From<AuctionParams> for payloads::AuctionParams {
    fn from(params: AuctionParams) -> Self {
        Self {
            round_duration: params.round_duration,
            bid_increment: params.bid_increment,
            activity_rule_params: params.activity_rule_params.0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, sqlx::Type, sqlx::FromRow)]
#[sqlx(transparent)]
pub struct OpenHoursId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct OpenHours {
    pub id: OpenHoursId,
}

#[derive(Debug, Clone, FromRow)]
pub struct OpenHoursWeekday {
    pub open_hours_id: OpenHoursId,
    pub day_of_week: i16,
    #[sqlx(try_from = "jiff_sqlx::Time")]
    pub open_time: Time,
    #[sqlx(try_from = "jiff_sqlx::Time")]
    pub close_time: Time,
}

#[derive(Debug, Clone, FromRow)]
pub struct Site {
    pub id: SiteId,
    pub community_id: CommunityId,
    pub name: String,
    pub description: Option<String>,
    pub default_auction_params_id: AuctionParamsId,
    #[sqlx(try_from = "SqlxSpan")]
    pub possession_period: Span,
    #[sqlx(try_from = "SqlxSpan")]
    pub auction_lead_time: Span,
    #[sqlx(try_from = "SqlxSpan")]
    pub proxy_bidding_lead_time: Span,
    pub open_hours_id: Option<OpenHoursId>,
    pub auto_schedule: bool,
    pub site_image_id: Option<SiteImageId>,
    pub timezone: Option<String>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
    #[sqlx(try_from = "OptionalTimestamp")]
    pub deleted_at: Option<Timestamp>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Space {
    pub id: SpaceId,
    pub site_id: SiteId,
    pub name: String,
    pub description: Option<String>,
    pub eligibility_points: f64,
    pub is_available: bool,
    pub site_image_id: Option<SiteImageId>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
    #[sqlx(try_from = "OptionalTimestamp")]
    pub deleted_at: Option<Timestamp>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Auction {
    pub id: AuctionId,
    pub site_id: SiteId,
    #[sqlx(try_from = "SqlxTs")]
    pub possession_start_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub possession_end_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub start_at: Timestamp,
    #[sqlx(try_from = "OptionalTimestamp")]
    pub end_at: Option<Timestamp>,
    pub auction_params_id: AuctionParamsId,
    pub scheduler_failure_count: i32,
    #[sqlx(try_from = "OptionalTimestamp")]
    pub scheduler_last_failed_at: Option<Timestamp>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

impl Auction {
    // Helper to convert to response type with params
    pub fn with_params(
        self,
        params: AuctionParams,
    ) -> payloads::responses::Auction {
        payloads::responses::Auction {
            auction_id: self.id,
            auction_details: payloads::Auction {
                site_id: self.site_id,
                possession_start_at: self.possession_start_at,
                possession_end_at: self.possession_end_at,
                start_at: self.start_at,
                auction_params: params.into(),
            },
            created_at: self.created_at,
            updated_at: self.updated_at,
            end_at: self.end_at,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct AuctionRound {
    pub id: AuctionRoundId,
    pub auction_id: AuctionId,
    pub round_num: i32,
    #[sqlx(try_from = "SqlxTs")]
    pub start_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub end_at: Timestamp,
    pub eligibility_threshold: f64, // fractional eligibility; 0-1
    #[sqlx(try_from = "OptionalTimestamp")]
    pub proxy_bidding_last_processed_at: Option<Timestamp>,
    pub proxy_bidding_failure_count: i32,
    #[sqlx(try_from = "OptionalTimestamp")]
    pub proxy_bidding_last_failed_at: Option<Timestamp>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

impl AuctionRound {
    pub fn into_response(self) -> payloads::responses::AuctionRound {
        payloads::responses::AuctionRound {
            round_id: self.id,
            round_details: payloads::AuctionRound {
                auction_id: self.auction_id,
                round_num: self.round_num,
                start_at: self.start_at,
                end_at: self.end_at,
                eligibility_threshold: self.eligibility_threshold,
            },
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct RoundSpaceResult {
    pub space_id: SpaceId,
    pub round_id: AuctionRoundId,
    pub winning_user_id: UserId,
    pub value: rust_decimal::Decimal,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserEligibility {
    pub user_id: UserId,
    pub round_id: AuctionRoundId,
    pub eligibility: f64,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserValue {
    pub user_id: UserId,
    pub space_id: SpaceId,
    pub value: Decimal,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

impl From<UserValue> for payloads::responses::UserValue {
    fn from(value: UserValue) -> Self {
        Self {
            space_id: value.space_id,
            value: value.value,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct UseProxyBidding {
    pub user_id: UserId,
    pub auction_id: AuctionId,
    pub max_items: i32,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

impl From<UseProxyBidding> for payloads::responses::UseProxyBidding {
    fn from(value: UseProxyBidding) -> Self {
        Self {
            auction_id: value.auction_id,
            max_items: value.max_items,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct AuditLogId(pub Uuid);

/// Database-level Community struct that matches the communities table schema
#[derive(Debug, Clone, FromRow)]
struct DbCommunity {
    id: CommunityId,
    name: String,
    new_members_default_active: bool,
    #[sqlx(try_from = "SqlxTs")]
    created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    updated_at: Timestamp,
    currency_mode: payloads::CurrencyMode,
    default_credit_limit: Option<Decimal>,
    currency_name: String,
    currency_symbol: String,
    currency_minor_units: i16,
    debts_callable: bool,
    balances_visible_to_members: bool,
    allowance_amount: Option<Decimal>,
    #[sqlx(try_from = "payloads::OptionalSpan")]
    allowance_period: Option<jiff::Span>,
    #[sqlx(try_from = "payloads::OptionalTimestamp")]
    allowance_start: Option<Timestamp>,
}

impl TryFrom<DbCommunity> for Community {
    type Error = StoreError;

    fn try_from(db: DbCommunity) -> Result<Self, Self::Error> {
        let currency =
            currency::currency_settings_from_db(currency::CurrencySettingsDb {
                mode: db.currency_mode,
                default_credit_limit: db.default_credit_limit,
                debts_callable: db.debts_callable,
                allowance_amount: db.allowance_amount,
                allowance_period: db.allowance_period,
                allowance_start: db.allowance_start,
                currency_name: db.currency_name,
                currency_symbol: db.currency_symbol,
                currency_minor_units: db.currency_minor_units,
                balances_visible_to_members: db.balances_visible_to_members,
                new_members_default_active: db.new_members_default_active,
            })
            .ok_or(StoreError::InvalidCurrencyConfiguration)?;

        Ok(Community {
            id: db.id,
            name: db.name,
            created_at: db.created_at,
            updated_at: db.updated_at,
            currency,
        })
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct AuditLog {
    pub id: AuditLogId,
    pub actor_id: Option<UserId>,
    pub action: String,
    pub target_table: Option<String>,
    pub target_id: Option<Uuid>,
    pub details: Option<serde_json::Value>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
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
    #[error("No active members to distribute balance to")]
    NoActiveMembersForDistribution,
    #[error("Span too large")]
    SpanTooLarge(Box<Span>),
    #[error("A space with the name '{name}' already exists in this site")]
    SpaceNameNotUnique { name: String },
    #[error("Unique constraint violation")]
    NotUnique(#[source] sqlx::Error),
    #[error("Database error")]
    Database(#[source] sqlx::Error),
    #[error("Unexpected error")]
    UnexpectedError(#[from] anyhow::Error),
    #[error("Insufficient permissions. Required: {required:?}")]
    InsufficientPermissions { required: PermissionLevel },
    #[error("Auction not found")]
    AuctionNotFound,
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
    #[error("Community invite not found")]
    CommunityInviteNotFound,
    #[error("Open hours not found")]
    OpenHoursNotFound,
    #[error("Auction params not found")]
    AuctionParamsNotFound,
    #[error("No eligibility found for the user")]
    NoEligibility,
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
    #[error("Amount must be positive")]
    AmountMustBePositive,
    #[error("Invalid treasury operation for this currency mode")]
    InvalidTreasuryOperation,
    #[error("Invalid credit limit operation for this currency mode")]
    InvalidCreditLimitOperation,
    #[error("Database invariant violation: invalid account ownership")]
    InvalidAccountOwnership,
    #[error("Database invariant violation: invalid currency configuration")]
    InvalidCurrencyConfiguration,
    #[error("Currency mode cannot be changed after community creation")]
    CurrencyModeImmutable,
    #[error("Invalid currency name (max 50 characters)")]
    InvalidCurrencyName,
    #[error("Invalid currency symbol (max 5 characters)")]
    InvalidCurrencySymbol,
    #[error("Journal entry lines must sum to zero, got {0}")]
    JournalLinesDoNotSumToZero(rust_decimal::Decimal),
    #[error("Duplicate account in journal entry")]
    DuplicateAccountInJournalEntry,
    #[error("Cannot reset balances while auctions are active")]
    CannotResetDuringActiveAuction,
    #[error("Cannot delete site with financial history")]
    SiteHasFinancialHistory,
}

/// Convert a space name unique constraint violation into a more specific error.
/// If the error is a unique violation on the spaces_site_id_name_unique index,
/// returns SpaceNameNotUnique. Otherwise returns the original error.
fn map_space_name_unique_error(e: sqlx::Error, space_name: &str) -> StoreError {
    if let sqlx::Error::Database(db_err) = &e
        && db_err.is_unique_violation()
    {
        // Check if this is the spaces_site_id_name_unique constraint
        if let Some(constraint) = db_err.constraint()
            && constraint == "spaces_site_id_name_unique"
        {
            return StoreError::SpaceNameNotUnique {
                name: space_name.to_string(),
            };
        }
    }
    e.into()
}

impl From<sqlx::Error> for StoreError {
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db_err) = &e
            && db_err.is_unique_violation()
        {
            return StoreError::NotUnique(e);
        }
        StoreError::Database(e)
    }
}

/// Balance a jiff::Span according to the units that can be stored in a Postgres
/// interval.
pub fn span_to_interval(span: &Span) -> Result<PgInterval, StoreError> {
    span_to_interval_opt(span).ok_or(StoreError::SpanTooLarge(Box::new(*span)))
}

fn span_to_interval_opt(span: &Span) -> Option<PgInterval> {
    let microseconds = span
        .get_milliseconds()
        .checked_add(span.get_milliseconds().checked_mul(1_000)?)?
        .checked_add(span.get_seconds().checked_mul(1_000_000)?)?
        .checked_add(span.get_minutes().checked_mul(60 * 1_000_000)?)?
        .checked_add(
            (span.get_hours() as i64).checked_mul(60 * 60 * 1_000_000)?,
        )?;
    let days = span
        .get_days()
        .checked_add(span.get_weeks().checked_mul(7)?)?;
    let months = span
        .get_months()
        .checked_add((span.get_years() as i32).checked_mul(12)?)?;
    Some(PgInterval {
        microseconds,
        days,
        months,
    })
}
