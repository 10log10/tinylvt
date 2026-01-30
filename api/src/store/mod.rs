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

use anyhow::Context;
use derive_more::Display;
use jiff::Span;
use jiff::{Timestamp, civil::Time};
use jiff_sqlx::ToSqlx;
use jiff_sqlx::{Span as SqlxSpan, Timestamp as SqlxTs};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{FromRow, PgPool, Postgres, Row, Transaction, Type};
use sqlx_postgres::types::PgInterval;
use tracing::Level;
use uuid::Uuid;

use payloads::{
    AuctionId, AuctionRoundId, Bid, CommunityId, InviteId, OptionalTimestamp,
    PermissionLevel, Role, SiteId, SiteImageId, SpaceId, UserId, requests,
    responses::{self, Community},
};

use crate::time::TimeSource;

pub mod currency;

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

/// Calculate the total eligibility points required for a set of spaces
async fn calculate_total_eligibility_points(
    spaces: &[SpaceId],
    pool: &PgPool,
) -> Result<f64, StoreError> {
    let spaces =
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = ANY($1)")
            .bind(spaces)
            .fetch_all(pool)
            .await?;

    Ok(spaces.iter().map(|space| space.eligibility_points).sum())
}

/// Get a user's eligibility for a specific auction round
pub async fn get_eligibility(
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Option<f64>, StoreError> {
    // Verify the round exists and get auction info
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => StoreError::Database(e),
    })?;

    // Validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    // Get user's eligibility for this round
    let eligibility = sqlx::query_scalar::<_, f64>(
        "SELECT eligibility FROM user_eligibilities 
        WHERE round_id = $1 AND user_id = $2",
    )
    .bind(round_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(eligibility)
}

/// List a user's eligibility for all rounds after round 0 in an auction.
pub async fn list_eligibility(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<Option<f64>>, StoreError> {
    // Validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    // Get all rounds for this auction in order
    let rounds = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds 
        WHERE auction_id = $1 
        ORDER BY round_num",
    )
    .bind(auction_id)
    .fetch_all(pool)
    .await?;

    // Get eligibility for each round
    let mut eligibilities = Vec::with_capacity(rounds.len());
    for round in &rounds[1..] {
        let eligibility = sqlx::query_scalar::<_, f64>(
            "SELECT eligibility FROM user_eligibilities 
            WHERE round_id = $1 AND user_id = $2",
        )
        .bind(round.id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        eligibilities.push(eligibility);
    }

    Ok(eligibilities)
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

pub async fn create_or_update_user_value(
    details: &payloads::requests::UserValue,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Verify the space exists and user has access to it
    let (_, _) = get_validated_space(
        &details.space_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    sqlx::query(
        "INSERT INTO user_values (user_id, space_id, value, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4)
        ON CONFLICT (user_id, space_id)
        DO UPDATE SET value = EXCLUDED.value, updated_at = EXCLUDED.updated_at",
    )
    .bind(user_id)
    .bind(details.space_id)
    .bind(details.value)
    .bind(time_source.now().to_sqlx())
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_user_value(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::UserValue, StoreError> {
    // Verify the space exists and user has access to it
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    let value = sqlx::query_as::<_, UserValue>(
        "SELECT * FROM user_values WHERE space_id = $1 AND user_id = $2",
    )
    .bind(space_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::UserValueNotFound,
        e => StoreError::Database(e),
    })?;

    Ok(value.into())
}

pub async fn delete_user_value(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // Verify the space exists and user has access to it
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    sqlx::query("DELETE FROM user_values WHERE space_id = $1 AND user_id = $2")
        .bind(space_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_user_values(
    user_id: &UserId,
    site_id: &SiteId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::UserValue>, StoreError> {
    // Verify user has access to the site
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await?;

    let _ = get_validated_member(user_id, &site.community_id, pool).await?;

    let values = sqlx::query_as::<_, UserValue>(
        "SELECT uv.* FROM user_values uv
        JOIN spaces s ON uv.space_id = s.id
        WHERE uv.user_id = $1 AND s.site_id = $2",
    )
    .bind(user_id)
    .bind(site_id)
    .fetch_all(pool)
    .await?;

    Ok(values.into_iter().map(Into::into).collect())
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

pub async fn create_or_update_proxy_bidding(
    details: &payloads::requests::UseProxyBidding,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Verify user has access to the auction
    let (_, _) = get_validated_auction(
        &details.auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    sqlx::query(
        "INSERT INTO use_proxy_bidding (user_id, auction_id, max_items, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4)
        ON CONFLICT (user_id, auction_id)
        DO UPDATE SET max_items = EXCLUDED.max_items, updated_at = EXCLUDED.updated_at",
    )
    .bind(user_id)
    .bind(details.auction_id)
    .bind(details.max_items)
    .bind(time_source.now().to_sqlx())
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_proxy_bidding(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Option<payloads::responses::UseProxyBidding>, StoreError> {
    // Verify user has access to the auction
    let (_, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    let settings = sqlx::query_as::<_, UseProxyBidding>(
        "SELECT * FROM use_proxy_bidding WHERE auction_id = $1 AND user_id = $2",
    )
    .bind(auction_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(settings.map(|s| s.into()))
}

pub async fn delete_proxy_bidding(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // Verify user has access to the auction
    let (_, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    sqlx::query(
        "DELETE FROM use_proxy_bidding WHERE auction_id = $1 AND user_id = $2",
    )
    .bind(auction_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
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
        let currency_config = currency::currency_config_from_db(
            db.currency_mode,
            db.default_credit_limit,
            db.debts_callable,
            db.allowance_amount,
            db.allowance_period,
            db.allowance_start,
        )
        .ok_or(StoreError::InvalidCurrencyConfiguration)?;

        Ok(Community {
            id: db.id,
            name: db.name,
            new_members_default_active: db.new_members_default_active,
            created_at: db.created_at,
            updated_at: db.updated_at,
            currency_config,
            currency_name: db.currency_name,
            currency_symbol: db.currency_symbol,
            balances_visible_to_members: db.balances_visible_to_members,
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

/// Create a community and add the creating user as the leader.
pub async fn create_community(
    details: &requests::CreateCommunity,
    user_id: UserId, // initial leader of community
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<Community, StoreError> {
    let user = read_user(pool, &user_id).await?;
    if !user.email_verified {
        return Err(StoreError::UnverifiedEmail);
    }
    if details.name.len() > payloads::requests::COMMUNITY_NAME_MAX_LEN {
        return Err(StoreError::FieldTooLong);
    }
    let mut tx = pool.begin().await?;

    // Validate and convert currency config enum to database columns
    // For IOU modes: if debts aren't callable, must have finite credit limit
    match &details.currency_config {
        payloads::CurrencyConfig::DistributedClearing(cfg)
        | payloads::CurrencyConfig::DeferredPayment(cfg) => {
            if !cfg.debts_callable && cfg.default_credit_limit.is_none() {
                return Err(StoreError::InvalidCurrencyConfiguration);
            }
        }
        _ => {}
    }

    let (
        currency_mode,
        default_credit_limit,
        debts_callable,
        allowance_amount,
        allowance_period,
        allowance_start,
    ) = currency::currency_config_to_db(&details.currency_config);

    let db_community = sqlx::query_as::<_, DbCommunity>(
        "INSERT INTO communities (
            name,
            new_members_default_active,
            currency_mode,
            default_credit_limit,
            debts_callable,
            currency_name,
            currency_symbol,
            balances_visible_to_members,
            allowance_amount,
            allowance_period,
            allowance_start,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12) RETURNING *;",
    )
    .bind(&details.name)
    .bind(details.new_members_default_active)
    .bind(currency_mode)
    .bind(default_credit_limit)
    .bind(debts_callable)
    .bind(&details.currency_name)
    .bind(&details.currency_symbol)
    .bind(details.balances_visible_to_members)
    .bind(allowance_amount)
    .bind(allowance_period.as_ref().map(span_to_interval).transpose()?)
    .bind(allowance_start.as_ref().map(|t| t.to_sqlx()))
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut *tx)
    .await?;

    let community: Community = db_community.try_into()?;

    sqlx::query(
        "INSERT INTO community_members (community_id, user_id, role, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4);",
    )
    .bind(community.id)
    .bind(user_id)
    .bind(Role::Leader)
    .bind(time_source.now().to_sqlx())
    .execute(&mut *tx)
    .await?;

    // Create treasury account
    currency::create_account_tx(
        &community.id,
        payloads::AccountOwner::Treasury,
        None,
        time_source,
        &mut tx,
    )
    .await?;

    // Create leader's member_main account
    currency::create_account_tx(
        &community.id,
        payloads::AccountOwner::Member(user_id),
        None,
        time_source,
        &mut tx,
    )
    .await?;

    tx.commit().await?;

    Ok(community)
}

/// Create a new user as would happen during signup.
pub async fn create_user(
    pool: &PgPool,
    username: &str,
    email: &str,
    password_hash: &str,
    time_source: &TimeSource,
) -> Result<User, StoreError> {
    if username.len() > payloads::requests::USERNAME_MAX_LEN {
        return Err(StoreError::FieldTooLong);
    }
    if email.len() > payloads::requests::EMAIL_MAX_LEN {
        return Err(StoreError::FieldTooLong);
    }
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (
                username,
                email,
                password_hash,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $4)
            RETURNING *;",
    )
    .bind(username)
    .bind(email)
    .bind(password_hash)
    .bind(time_source.now().to_sqlx())
    .fetch_one(pool)
    .await?;
    Ok(user)
}

/// Create a new user as would happen during signup.
pub async fn read_user(pool: &PgPool, id: &UserId) -> Result<User, StoreError> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1;")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StoreError::UserNotFound,
            e => StoreError::Database(e),
        })
}

pub async fn update_user_profile(
    user_id: &UserId,
    display_name: &Option<String>,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<User, StoreError> {
    let updated_user = sqlx::query_as::<_, User>(
        r#"
        UPDATE users SET display_name = $2, updated_at = $3
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING *
        "#,
    )
    .bind(user_id.0)
    .bind(display_name.as_ref())
    .bind(time_source.now().to_sqlx())
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::UserNotFound,
        _ => StoreError::Database(e),
    })?;

    Ok(updated_user)
}

/// Delete a user account.
///
/// Attempts a hard delete first. If that fails due to foreign key constraints
/// (user has bids, round_space_results, or user_eligibilities), falls back to
/// anonymizing PII and setting `deleted_at` to preserve referential integrity
/// for auction history.
///
/// On anonymization, also removes: user_values, use_proxy_bidding, tokens, and
/// community_members entries.
///
/// Returns `UserIsLeader` error if the user is a leader of any community.
/// Leaders must transfer leadership before deleting their account.
pub async fn delete_user(
    pool: &PgPool,
    id: &UserId,
    time_source: &TimeSource,
) -> Result<User, StoreError> {
    // Subquery to check if user is a leader. Used in WHERE clauses to
    // atomically prevent deletion of leaders (avoiding race with promotion).
    let is_leader_subquery = "EXISTS (SELECT 1 FROM community_members WHERE user_id = $1 AND role = 'leader')";

    // Try hard delete first
    let delete_result = sqlx::query_as::<_, User>(&format!(
        "DELETE FROM users WHERE id = $1 AND NOT {is_leader_subquery} RETURNING *"
    ))
    .bind(id)
    .fetch_one(pool)
    .await;

    match delete_result {
        Ok(user) => Ok(user),
        Err(sqlx::Error::RowNotFound) => {
            // Either user doesn't exist, or they're a leader. Check which.
            let is_leader = sqlx::query_scalar::<_, bool>(&format!(
                "SELECT {is_leader_subquery}"
            ))
            .bind(id)
            .fetch_one(pool)
            .await?;

            if is_leader {
                Err(StoreError::UserIsLeader)
            } else {
                Err(StoreError::UserNotFound)
            }
        }
        Err(sqlx::Error::Database(db_err))
            if db_err.is_foreign_key_violation() =>
        {
            // User has auction history, anonymize instead
            let now = time_source.now().to_sqlx();
            let mut tx = pool.begin().await?;

            // Delete community_members first, with leader check in WHERE clause
            // to atomically verify they're not a leader before removing their
            // membership (which would orphan the community).
            let rows_deleted = sqlx::query(&format!(
                "DELETE FROM community_members WHERE user_id = $1 AND NOT {is_leader_subquery}"
            ))
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

            // Check if deletion was blocked due to being a leader
            if rows_deleted == 0 {
                let is_leader = sqlx::query_scalar::<_, bool>(&format!(
                    "SELECT {is_leader_subquery}"
                ))
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;

                if is_leader {
                    return Err(StoreError::UserIsLeader);
                }
                // rows_deleted == 0 but not a leader means they had no
                // community memberships, which is fine
            }

            // Remove other non-historical user data
            sqlx::query("DELETE FROM user_values WHERE user_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM use_proxy_bidding WHERE user_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM tokens WHERE user_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;

            // Anonymize PII and mark as unverified to block community actions
            let user = sqlx::query_as::<_, User>(
                r#"
                UPDATE users SET
                    email = 'deleted-' || id || '@deleted.local',
                    username = 'deleted-' || id::text,
                    password_hash = '',
                    display_name = NULL,
                    email_verified = false,
                    deleted_at = $2,
                    updated_at = $2
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(id)
            .bind(now)
            .fetch_one(&mut *tx)
            .await?;

            tx.commit().await?;
            Ok(user)
        }
        Err(e) => Err(e.into()),
    }
}

/// Create a token for email verification or password reset
#[tracing::instrument(skip(pool, time_source))]
pub async fn create_token(
    user_id: &UserId,
    action: TokenAction,
    expires_at: Timestamp,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<TokenId, StoreError> {
    let token_id = sqlx::query_as::<_, TokenId>(
        r#"
        INSERT INTO tokens (user_id, action, expires_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(action)
    .bind(expires_at.to_sqlx())
    .bind(time_source.now().to_sqlx())
    .fetch_one(pool)
    .await
    .context("Failed to create token")?;

    tracing::info!("Created {:?} token for user {}", action, user_id.0);
    Ok(token_id)
}

/// Find and validate a token for use
#[tracing::instrument(skip(pool, time_source))]
pub async fn consume_token(
    token_id: &TokenId,
    expected_action: TokenAction,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<UserId, StoreError> {
    let mut tx = pool.begin().await.context("Failed to begin transaction")?;

    // Get the token and validate it
    let token = sqlx::query_as::<_, Token>(
        r#"
        SELECT *
        FROM tokens 
        WHERE id = $1
        "#,
    )
    .bind(token_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Failed to fetch token")?
    .ok_or(StoreError::TokenNotFound)?;

    // Validate token
    if token.action != expected_action {
        return Err(StoreError::InvalidTokenAction);
    }

    if token.used {
        return Err(StoreError::TokenAlreadyUsed);
    }

    // Check expiration using the provided time source
    let now = time_source.now();
    if now > token.expires_at {
        return Err(StoreError::TokenExpired);
    }

    // Mark token as used
    sqlx::query(
        r#"
        UPDATE tokens
        SET used = true, updated_at = $2
        WHERE id = $1
        "#,
    )
    .bind(token_id)
    .bind(time_source.now().to_sqlx())
    .execute(&mut *tx)
    .await
    .context("Failed to mark token as used")?;

    tx.commit().await.context("Failed to commit transaction")?;

    tracing::info!(
        "Consumed {:?} token for user {}",
        expected_action,
        token.user_id.0
    );
    Ok(token.user_id)
}

/// Mark user's email as verified
///
/// Fails for anonymized users since verification tokens are deleted.
#[tracing::instrument(skip(pool, time_source))]
pub async fn verify_user_email(
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let rows_affected = sqlx::query(
        r#"
        UPDATE users
        SET email_verified = true, updated_at = $2
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .bind(time_source.now().to_sqlx())
    .execute(pool)
    .await
    .context("Failed to verify user email")?
    .rows_affected();

    if rows_affected == 0 {
        return Err(StoreError::UserNotFound);
    }

    tracing::info!("Verified email for user {}", user_id.0);
    Ok(())
}

/// Get user by email for password reset
#[tracing::instrument(skip(pool))]
pub async fn get_user_by_email(
    email: &str,
    pool: &PgPool,
) -> Result<User, StoreError> {
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1 AND deleted_at IS NULL",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .context("Failed to fetch user by email")?
    .ok_or(StoreError::UserNotFound)
}

/// Clean up expired tokens
#[tracing::instrument(skip(pool, time_source))]
pub async fn cleanup_expired_tokens(
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<u64, StoreError> {
    let now = time_source.now();

    let result = sqlx::query(
        r#"
        DELETE FROM tokens 
        WHERE expires_at < $1
        "#,
    )
    .bind(now.to_sqlx())
    .execute(pool)
    .await
    .context("Failed to cleanup expired tokens")?;

    tracing::info!("Cleaned up {} expired tokens", result.rows_affected());
    Ok(result.rows_affected())
}

pub async fn get_validated_member(
    user_id: &UserId,
    community_id: &CommunityId,
    pool: &PgPool,
) -> Result<ValidatedMember, StoreError> {
    let Some(member) = sqlx::query_as::<_, CommunityMember>(
        "SELECT * FROM community_members WHERE
            community_id = $1 AND user_id = $2;",
    )
    .bind(community_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    else {
        return Err(StoreError::MemberNotFound);
    };
    Ok(ValidatedMember(member))
}

/// Batch fetch user identities for a list of user IDs
///
/// Returns a HashMap of user_id -> UserIdentity. This is useful for
/// efficiently fetching display information for multiple users at once.
pub(crate) async fn get_user_identities(
    user_ids: &[UserId],
    // TODO: migrate display_name to community_member table and use this param
    _community_id: &CommunityId,
    pool: &PgPool,
) -> Result<
    std::collections::HashMap<UserId, payloads::responses::UserIdentity>,
    StoreError,
> {
    if user_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let identities: Vec<payloads::responses::UserIdentity> = sqlx::query_as(
        r#"
        SELECT
            u.id as user_id,
            u.username,
            u.display_name
        FROM users u
        WHERE u.id = ANY($1)
        "#,
    )
    .bind(user_ids)
    .fetch_all(pool)
    .await?;

    Ok(identities
        .into_iter()
        .map(|identity| (identity.user_id, identity))
        .collect())
}

/// Helper to enrich a collection of items with user identities
///
/// Given a collection of items, extracts user IDs, batch loads their identities,
/// and maps each item to a result using the provided mapper function.
pub(crate) async fn with_user_identities<T, R, F>(
    items: Vec<T>,
    get_user_id: impl Fn(&T) -> UserId,
    mapper: F,
    community_id: &CommunityId,
    pool: &PgPool,
) -> Result<Vec<R>, StoreError>
where
    F: Fn(T, payloads::responses::UserIdentity) -> Result<R, StoreError>,
{
    if items.is_empty() {
        return Ok(Vec::new());
    }

    // Extract user IDs
    let user_ids: Vec<UserId> = items.iter().map(&get_user_id).collect();

    // Batch fetch identities
    let user_identities =
        get_user_identities(&user_ids, community_id, pool).await?;

    // Map items with their identities
    items
        .into_iter()
        .map(|item| {
            let user_id = get_user_id(&item);
            let identity = user_identities
                .get(&user_id)
                .cloned()
                .ok_or(StoreError::UserNotFound)?;
            mapper(item, identity)
        })
        .collect()
}

pub async fn invite_community_member(
    actor: &ValidatedMember,
    new_member_email: &Option<String>,
    single_use: bool,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<InviteId, StoreError> {
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }
    let invite = sqlx::query_as::<_, CommunityInvite>(
        "INSERT INTO community_invites (community_id, email, single_use, created_at)
        VALUES ($1, $2, $3, $4) RETURNING *;",
    )
    .bind(actor.0.community_id)
    .bind(new_member_email)
    .bind(single_use)
    .bind(time_source.now().to_sqlx())
    .fetch_one(pool)
    .await?;
    Ok(invite.id)
}

pub async fn get_invite_community_name(
    invite_id: &payloads::InviteId,
    pool: &PgPool,
) -> Result<String, StoreError> {
    let community_name = sqlx::query_scalar::<_, String>(
        "SELECT c.name
         FROM community_invites ci
         JOIN communities c ON ci.community_id = c.id
         WHERE ci.id = $1;",
    )
    .bind(invite_id)
    .fetch_optional(pool)
    .await?;

    let Some(community_name) = community_name else {
        return Err(StoreError::CommunityInviteNotFound);
    };

    Ok(community_name)
}

pub async fn accept_invite(
    user_id: &UserId,
    invite_id: &payloads::InviteId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let user = read_user(pool, user_id).await?;
    if !user.email_verified {
        return Err(StoreError::UnverifiedEmail);
    }
    let invite = sqlx::query_as::<_, CommunityInvite>(
        "SELECT * FROM community_invites WHERE id = $1;",
    )
    .bind(invite_id)
    .fetch_optional(pool)
    .await?;
    let Some(invite) = invite else {
        return Err(StoreError::CommunityInviteNotFound);
    };
    if let Some(ref invite_email) = invite.email
        && *invite_email != user.email
    {
        return Err(StoreError::MismatchedInviteEmail);
    }

    let mut tx = pool.begin().await?;

    let result = sqlx::query(
        "INSERT INTO community_members (community_id, user_id, role, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4);",
    )
    .bind(invite.community_id)
    .bind(user_id)
    .bind(Role::Member)
    .bind(time_source.now().to_sqlx())
    .execute(&mut *tx)
    .await;

    if let Err(StoreError::NotUnique(_)) = result.map_err(StoreError::from) {
        return Err(StoreError::AlreadyMember);
    }

    // Create member_main account for the new member
    currency::create_account_tx(
        &invite.community_id,
        payloads::AccountOwner::Member(*user_id),
        None,
        time_source,
        &mut tx,
    )
    .await?;

    if invite.email.is_some() || invite.single_use {
        sqlx::query("DELETE FROM community_invites WHERE id = $1")
            .bind(invite_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    Ok(())
}

pub async fn delete_invite(
    actor: &ValidatedMember,
    invite_id: &payloads::InviteId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }

    // Verify the invite exists and belongs to this community
    let invite_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM community_invites WHERE id = $1 AND community_id = $2)",
    )
    .bind(invite_id)
    .bind(actor.0.community_id)
    .fetch_one(pool)
    .await?;

    if !invite_exists {
        return Err(StoreError::CommunityInviteNotFound);
    }

    // Delete the invite
    sqlx::query("DELETE FROM community_invites WHERE id = $1")
        .bind(invite_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn get_communities(
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::CommunityWithRole>, StoreError> {
    let rows = sqlx::query(
        "SELECT
            b.*,
            a.role as user_role,
            a.is_active as user_is_active
        FROM community_members a
        JOIN communities b ON a.community_id = b.id
        WHERE a.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut communities = Vec::new();
    for row in rows {
        let db_community = DbCommunity::from_row(&row)?;
        let community: Community = db_community.try_into()?;
        let user_role: Role = row.try_get("user_role")?;
        let user_is_active: bool = row.try_get("user_is_active")?;
        communities.push(payloads::responses::CommunityWithRole {
            community,
            user_role,
            user_is_active,
        });
    }

    Ok(communities)
}

pub async fn get_community_by_id(
    community_id: &CommunityId,
    pool: &PgPool,
) -> Result<Community, StoreError> {
    let db_community = sqlx::query_as::<_, DbCommunity>(
        "SELECT * FROM communities WHERE id = $1",
    )
    .bind(community_id)
    .fetch_optional(pool)
    .await?
    .ok_or(StoreError::CommunityNotFound)?;

    db_community.try_into()
}

pub async fn get_received_invites(
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<responses::CommunityInviteReceived>, StoreError> {
    let user = read_user(pool, user_id).await?;
    // Need to make sure this user actually owns this email before showing them
    // the invites they've received
    if !user.email_verified {
        return Err(StoreError::UnverifiedEmail);
    }
    Ok(sqlx::query_as::<_, responses::CommunityInviteReceived>(
        "SELECT
            a.*,
            b.name as community_name
        FROM community_invites a
        JOIN communities b ON a.community_id = b.id
        WHERE a.email = $1",
    )
    .bind(user.email)
    .fetch_all(pool)
    .await?)
}

pub async fn get_issued_invites(
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<Vec<responses::IssuedCommunityInvite>, StoreError> {
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }

    Ok(sqlx::query_as::<_, responses::IssuedCommunityInvite>(
        "SELECT
            id,
            email as new_member_email,
            single_use,
            created_at
        FROM community_invites
        WHERE community_id = $1
        ORDER BY created_at DESC",
    )
    .bind(actor.0.community_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_members(
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<Vec<responses::CommunityMember>, StoreError> {
    #[derive(sqlx::FromRow)]
    struct DbMember {
        user_id: UserId,
        role: Role,
        is_active: bool,
    }

    let db_members: Vec<DbMember> = sqlx::query_as(
        "SELECT user_id, role, is_active
        FROM community_members
        WHERE community_id = $1",
    )
    .bind(actor.0.community_id)
    .fetch_all(pool)
    .await?;

    with_user_identities(
        db_members,
        |m| m.user_id,
        |m, user| {
            Ok(responses::CommunityMember {
                user,
                role: m.role,
                is_active: m.is_active,
            })
        },
        &actor.0.community_id,
        pool,
    )
    .await
}

pub async fn set_membership_schedule(
    actor: &ValidatedMember,
    schedule: &[payloads::MembershipSchedule],
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }

    let mut tx = pool.begin().await?;

    sqlx::query(
        "DELETE FROM community_membership_schedule
        WHERE community_id = $1",
    )
    .bind(actor.0.community_id)
    .execute(&mut *tx)
    .await?;

    for sched_elem in schedule {
        sqlx::query(
            "INSERT INTO community_membership_schedule (
                community_id,
                start_at,
                end_at,
                email,
                created_at,
                updated_at
            ) VALUES ($1, $2, $3, $4, $5, $5);",
        )
        .bind(actor.0.community_id)
        .bind(sched_elem.start_at.to_sqlx())
        .bind(sched_elem.end_at.to_sqlx())
        .bind(&sched_elem.email)
        .bind(time_source.now().to_sqlx())
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

pub async fn get_membership_schedule(
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<Vec<payloads::MembershipSchedule>, StoreError> {
    if !actor.0.role.is_ge_moderator() {
        return Err(StoreError::RequiresModeratorPermissions);
    }

    Ok(sqlx::query_as::<_, payloads::MembershipSchedule>(
        "SELECT * FROM community_membership_schedule
        WHERE community_id = $1",
    )
    .bind(actor.0.community_id)
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, FromRow)]
struct MemberInSchedule {
    community_id: CommunityId,
    user_id: UserId,
    email: String,
}

#[tracing::instrument(skip(pool, time_source), err(level = Level::ERROR))]
/// Update members' is_active status in all communities based on the schedule,
/// if they are present in the schedule.
pub async fn update_is_active_from_schedule(
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Get all (community, user) pairs in the schedule table. Only these
    // community members are to have their is_active status updated.
    let community_members_in_schedule = sqlx::query_as::<_, MemberInSchedule>(
        "SELECT DISTINCT
            a.community_id,
            a.email,
            u.id as user_id
        FROM community_membership_schedule a
        JOIN users u ON a.email = u.email;",
    )
    .fetch_all(pool)
    .await?;

    let now = time_source.now().to_sqlx();
    // Might as well make sure we update everything or nothing to avoid
    // partially completed state.
    let mut tx = pool.begin().await?;
    for community_member in community_members_in_schedule {
        // Set is_active if we can find a matching row in the schedule where the
        // user is meant to be a member.
        sqlx::query(
            "UPDATE community_members m
            SET is_active = EXISTS (
                SELECT 1
                FROM community_membership_schedule a
                WHERE
                    a.email = $1
                    AND a.community_id = $2
                    AND a.start_at <= $3
                    AND a.end_at > $3
            ),
            updated_at = $5
            WHERE
                m.user_id = $4
                AND m.community_id = $2",
        )
        .bind(&community_member.email)
        .bind(community_member.community_id)
        .bind(now)
        .bind(community_member.user_id)
        .bind(time_source.now().to_sqlx())
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

pub async fn create_site(
    details: &payloads::Site,
    actor: &ValidatedMember,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<Site, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        });
    }
    let mut tx = pool.begin().await?;

    let open_hours_id = match &details.open_hours {
        Some(hours) => Some(create_open_hours(hours, &mut tx).await?),
        None => None,
    };
    let auction_params_id = create_auction_params(
        &details.default_auction_params,
        &mut tx,
        time_source,
    )
    .await?;

    let site = sqlx::query_as::<_, Site>(
        "INSERT INTO sites (
            community_id,
            name,
            description,
            default_auction_params_id,
            possession_period,
            auction_lead_time,
            proxy_bidding_lead_time,
            open_hours_id,
            auto_schedule,
            timezone,
            site_image_id,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12) RETURNING *",
    )
    .bind(actor.0.community_id)
    .bind(&details.name)
    .bind(&details.description)
    .bind(auction_params_id)
    .bind(span_to_interval(&details.possession_period)?)
    .bind(span_to_interval(&details.auction_lead_time)?)
    .bind(span_to_interval(&details.proxy_bidding_lead_time)?)
    .bind(open_hours_id)
    .bind(details.auto_schedule)
    .bind(&details.timezone)
    .bind(details.site_image_id)
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(site)
}

async fn create_open_hours(
    open_hours: &payloads::OpenHours,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<OpenHoursId, StoreError> {
    let open_hours_id = sqlx::query_as::<_, OpenHoursId>(
        "INSERT INTO open_hours DEFAULT VALUES RETURNING id",
    )
    .fetch_one(&mut **tx)
    .await?;

    insert_open_hours_weekdays(&open_hours_id, open_hours, tx).await?;
    Ok(open_hours_id)
}

async fn insert_open_hours_weekdays(
    open_hours_id: &OpenHoursId,
    open_hours: &payloads::OpenHours,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), StoreError> {
    for day_of_week in &open_hours.days_of_week {
        sqlx::query(
            "INSERT INTO open_hours_weekday (
                open_hours_id,
                day_of_week,
                open_time,
                close_time
            ) VALUES ($1, $2, $3, $4)",
        )
        .bind(open_hours_id)
        .bind(day_of_week.day_of_week)
        .bind(day_of_week.open_time.to_sqlx())
        .bind(day_of_week.close_time.to_sqlx())
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn create_auction_params(
    params: &payloads::AuctionParams,
    tx: &mut Transaction<'_, Postgres>,
    time_source: &TimeSource,
) -> Result<AuctionParamsId, StoreError> {
    Ok(sqlx::query_as::<_, AuctionParamsId>(
        "INSERT INTO auction_params (
                round_duration,
                bid_increment,
                activity_rule_params,
                created_at,
                updated_at
            ) VALUES ($1, $2, $3, $4, $4) RETURNING id",
    )
    .bind(span_to_interval(&params.round_duration)?)
    .bind(params.bid_increment)
    .bind(Json(params.activity_rule_params.clone()))
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut **tx)
    .await?)
}

pub async fn get_site_community_id(
    site_id: &SiteId,
    pool: &PgPool,
) -> Result<CommunityId, StoreError> {
    sqlx::query_as::<_, CommunityId>(
        "SELECT community_id FROM sites WHERE id = $1",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::SiteNotFound,
        e => StoreError::Database(e),
    })
}

pub async fn get_site(
    site_id: &SiteId,
    pool: &PgPool,
) -> Result<payloads::responses::Site, StoreError> {
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StoreError::SiteNotFound,
            e => StoreError::Database(e),
        })?;
    let open_hours = match &site.open_hours_id {
        Some(open_hours_id) => {
            let days_of_week = sqlx::query_as::<_, payloads::OpenHoursWeekday>(
                "SELECT * FROM open_hours_weekday WHERE open_hours_id = $1",
            )
            .bind(open_hours_id)
            .fetch_all(pool)
            .await?;
            Some(payloads::OpenHours { days_of_week })
        }
        None => None,
    };
    let default_auction_params = sqlx::query_as::<_, AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(site.default_auction_params_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionParamsNotFound,
        e => StoreError::Database(e),
    })?;
    let site_details = payloads::Site {
        community_id: site.community_id,
        name: site.name,
        description: site.description,
        default_auction_params: default_auction_params.into(),
        possession_period: site.possession_period,
        auction_lead_time: site.auction_lead_time,
        proxy_bidding_lead_time: site.proxy_bidding_lead_time,
        open_hours,
        auto_schedule: site.auto_schedule,
        timezone: site.timezone,
        site_image_id: site.site_image_id,
    };
    Ok(payloads::responses::Site {
        site_id: site.id,
        site_details,
        created_at: site.created_at,
        updated_at: site.updated_at,
        deleted_at: site.deleted_at,
    })
}

pub async fn update_site(
    update_site: &payloads::requests::UpdateSite,
    actor: &ValidatedMember,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<responses::Site, StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let details = &update_site.site_details;

    let existing_site =
        sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
            .bind(update_site.site_id)
            .fetch_one(pool)
            .await?;

    let mut tx = pool.begin().await?;

    let new_open_hours_id = update_open_hours(
        &existing_site.open_hours_id,
        &details.open_hours,
        &mut tx,
    )
    .await?;

    let new_auction_params_id = create_auction_params(
        &details.default_auction_params,
        &mut tx,
        time_source,
    )
    .await?;

    sqlx::query(
        "UPDATE sites SET
            name = $1,
            description = $2,
            default_auction_params_id = $3,
            possession_period = $4,
            auction_lead_time = $5,
            proxy_bidding_lead_time = $6,
            open_hours_id = $7,
            auto_schedule = $8,
            timezone = $9,
            site_image_id = $10,
            updated_at = $12
        WHERE id = $11",
    )
    .bind(&details.name)
    .bind(&details.description)
    .bind(new_auction_params_id)
    .bind(span_to_interval(&details.possession_period)?)
    .bind(span_to_interval(&details.auction_lead_time)?)
    .bind(span_to_interval(&details.proxy_bidding_lead_time)?)
    .bind(new_open_hours_id)
    .bind(details.auto_schedule)
    .bind(&details.timezone)
    .bind(details.site_image_id)
    .bind(existing_site.id)
    .bind(time_source.now().to_sqlx())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let site = get_site(&existing_site.id, pool).await?;

    cleanup_unused_auction_params(pool).await;
    Ok(site)
}

async fn cleanup_unused_auction_params(pool: &PgPool) {
    if let Err(e) = sqlx::query(
        "DELETE FROM auction_params p
        WHERE NOT EXISTS (
            SELECT FROM sites
            WHERE default_auction_params_id = p.id
        ) AND NOT EXISTS (
            SELECT FROM auctions
            WHERE auction_params_id = p.id
        );",
    )
    .execute(pool)
    .await
    .context("cleanup unused auction params")
    {
        tracing::error!("{e:#}");
    }
}

/// Update an existing open hours (if it exists), returning the id.
async fn update_open_hours(
    // existing open hours
    open_hours_id: &Option<OpenHoursId>,
    new_open_hours: &Option<payloads::OpenHours>,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Option<OpenHoursId>, StoreError> {
    // delete the existing open hours
    sqlx::query("DELETE FROM open_hours WHERE id = $1;")
        .bind(open_hours_id)
        .execute(&mut **tx)
        .await?;

    // add new open hours
    match new_open_hours {
        Some(new_open_hours) => {
            Ok(Some(create_open_hours(new_open_hours, tx).await?))
        }
        None => Ok(None),
    }
}

pub async fn delete_community(
    community_id: &payloads::CommunityId,
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // Only leader can delete a community
    if !actor.0.role.is_leader() {
        return Err(StoreError::RequiresLeaderPermissions);
    }

    // Delete community - cascades to:
    // - community_members
    // - community_invites
    // - community_membership_schedule
    // - site_images
    // - sites (which cascades to spaces, auctions, etc.)
    let result = sqlx::query("DELETE FROM communities WHERE id = $1")
        .bind(community_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(StoreError::CommunityNotFound);
    }

    // Clean up orphaned auction params
    cleanup_unused_auction_params(pool).await;

    Ok(())
}

pub async fn delete_site(
    site_id: &payloads::SiteId,
    actor: &ValidatedMember,
    pool: &PgPool,
) -> Result<(), StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let existing_site =
        sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
            .bind(site_id)
            .fetch_one(pool)
            .await?;

    let mut tx = pool.begin().await?;

    // remove any remaining open hours
    update_open_hours(&existing_site.open_hours_id, &None, &mut tx).await?;

    sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(site_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    cleanup_unused_auction_params(pool).await;

    Ok(())
}

pub async fn soft_delete_site(
    site_id: &payloads::SiteId,
    actor: &ValidatedMember,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let now = time_source.now().to_sqlx();

    // Use transaction to ensure atomicity
    let mut tx = pool.begin().await?;

    // Cancel any active auctions for this site
    // (auctions where end_at is NULL or in the future)
    sqlx::query(
        "UPDATE auctions
         SET end_at = $2, updated_at = $2
         WHERE site_id = $1
         AND (end_at IS NULL OR end_at > $2)",
    )
    .bind(site_id)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    // Soft delete the site
    let result = sqlx::query(
        "UPDATE sites SET deleted_at = $2, updated_at = $2 WHERE id = $1",
    )
    .bind(site_id)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StoreError::SiteNotFound);
    }

    tx.commit().await?;

    Ok(())
}

pub async fn restore_site(
    site_id: &payloads::SiteId,
    actor: &ValidatedMember,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let now = time_source.now().to_sqlx();

    let result = sqlx::query(
        "UPDATE sites SET deleted_at = NULL, updated_at = $2 WHERE id = $1",
    )
    .bind(site_id)
    .bind(now)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StoreError::SiteNotFound);
    }

    Ok(())
}

pub async fn list_sites(
    community_id: &payloads::CommunityId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::Site>, StoreError> {
    // Validate user is a member of the community
    let _ = get_validated_member(user_id, community_id, pool).await?;

    let sites = sqlx::query_as::<_, Site>(
        "SELECT * FROM sites WHERE community_id = $1 ORDER BY name",
    )
    .bind(community_id)
    .fetch_all(pool)
    .await?;

    // Convert to response format
    let mut site_responses = Vec::new();
    for site in sites {
        let site_response = get_site(&site.id, pool).await?;
        site_responses.push(site_response);
    }

    Ok(site_responses)
}

/// Get a space and validate that the user has the required permission
/// level in the site's community. Returns both the space and the
/// validated member if successful.
async fn get_validated_space(
    space_id: &SpaceId,
    user_id: &UserId,
    required_permission: PermissionLevel,
    pool: &PgPool,
) -> Result<(Space, ValidatedMember), StoreError> {
    let space =
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
            .bind(space_id)
            .fetch_one(pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => StoreError::SpaceNotFound,
                e => StoreError::Database(e),
            })?;

    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(space.site_id)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StoreError::SiteNotFound,
            e => StoreError::Database(e),
        })?;

    let actor = get_validated_member(user_id, &site.community_id, pool).await?;

    if !required_permission.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: required_permission,
        });
    }

    Ok((space, actor))
}

/// Internal transaction-aware space creation function.
/// Caller is responsible for managing the transaction and validating
/// permissions.
async fn create_space_tx(
    details: &payloads::Space,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
) -> Result<Space, StoreError> {
    let space = sqlx::query_as::<_, Space>(
        "INSERT INTO spaces (
            site_id,
            name,
            description,
            eligibility_points,
            is_available,
            site_image_id,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $7) RETURNING *",
    )
    .bind(details.site_id)
    .bind(&details.name)
    .bind(&details.description)
    .bind(details.eligibility_points)
    .bind(details.is_available)
    .bind(details.site_image_id)
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| map_space_name_unique_error(e, &details.name))?;

    Ok(space)
}

pub async fn create_space(
    details: &payloads::Space,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<Space, StoreError> {
    // Get the site and validate user permissions
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(details.site_id)
        .fetch_one(pool)
        .await?;

    let actor = get_validated_member(user_id, &site.community_id, pool).await?;

    if !PermissionLevel::Coleader.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        });
    }

    let mut tx = pool.begin().await?;
    let space = create_space_tx(details, &mut tx, time_source).await?;
    tx.commit().await?;

    Ok(space)
}

pub async fn get_space(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::Space, StoreError> {
    let (space, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    Ok(space.into())
}

/// Check if a space has auction history (bids or round results).
/// Returns true if the space has been used in any auction.
async fn space_has_auction_history(
    space_id: &SpaceId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<bool, StoreError> {
    let has_history = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1 FROM bids WHERE space_id = $1
            UNION
            SELECT 1 FROM round_space_results WHERE space_id = $1
        )",
    )
    .bind(space_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(has_history)
}

/// Check if update contains nontrivial changes (name or eligibility_points).
/// These fields trigger copy-on-write when the space has auction history.
fn has_nontrivial_changes(
    old_space: &Space,
    new_details: &payloads::Space,
) -> bool {
    old_space.name != new_details.name
        || old_space.eligibility_points != new_details.eligibility_points
}

/// Internal transaction-aware space update function.
/// Caller is responsible for managing the transaction.
async fn update_space_tx(
    space_id: &SpaceId,
    details: &payloads::Space,
    user_id: &UserId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::responses::UpdateSpaceResult, StoreError> {
    let (old_space, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Coleader, pool)
            .await?;

    // Check for auction history and nontrivial changes
    let has_history = space_has_auction_history(space_id, tx).await?;
    let nontrivial = has_nontrivial_changes(&old_space, details);

    if has_history && nontrivial {
        // Copy-on-write: create new space and soft-delete old one
        let new_space = create_space_tx(details, tx, time_source).await?;

        // Soft-delete old space
        let now = time_source.now().to_sqlx();
        sqlx::query(
            "UPDATE spaces SET deleted_at = $2, updated_at = $2 WHERE id = $1",
        )
        .bind(old_space.id)
        .bind(now)
        .execute(&mut **tx)
        .await?;

        return Ok(payloads::responses::UpdateSpaceResult {
            space: new_space.into(),
            was_copied: true,
            old_space_id: Some(*space_id),
        });
    }

    // Update in place
    let updated_space = sqlx::query_as::<_, Space>(
        "UPDATE spaces SET
            name = $1,
            description = $2,
            eligibility_points = $3,
            is_available = $4,
            site_image_id = $5,
            updated_at = $7
        WHERE id = $6
        RETURNING *",
    )
    .bind(&details.name)
    .bind(&details.description)
    .bind(details.eligibility_points)
    .bind(details.is_available)
    .bind(details.site_image_id)
    .bind(space_id)
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| map_space_name_unique_error(e, &details.name))?;

    Ok(payloads::responses::UpdateSpaceResult {
        space: updated_space.into(),
        was_copied: false,
        old_space_id: None,
    })
}

pub async fn update_space(
    space_id: &SpaceId,
    details: &payloads::Space,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::responses::UpdateSpaceResult, StoreError> {
    let mut tx = pool.begin().await?;
    let result =
        update_space_tx(space_id, details, user_id, &mut tx, pool, time_source)
            .await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn update_spaces(
    updates: &[payloads::requests::UpdateSpace],
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<Vec<payloads::responses::UpdateSpaceResult>, StoreError> {
    if updates.is_empty() {
        return Ok(Vec::new());
    }

    // Start a transaction
    let mut tx = pool.begin().await?;

    // Process each update using the transaction-based function
    // (validation happens inside update_space_tx)
    let mut results = Vec::new();
    for update in updates {
        let result = update_space_tx(
            &update.space_id,
            &update.space_details,
            user_id,
            &mut tx,
            pool,
            time_source,
        )
        .await?;
        results.push(result);
    }

    // Commit the transaction
    tx.commit().await?;

    Ok(results)
}

pub async fn delete_space(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Coleader, pool)
            .await?;

    // Only delete if no auction history references this space.
    // This preserves auction data integrity while allowing CASCADE for bulk
    // operations like community deletion.
    let result = sqlx::query(
        "DELETE FROM spaces WHERE id = $1
         AND NOT EXISTS (SELECT 1 FROM bids WHERE space_id = $1)
         AND NOT EXISTS (SELECT 1 FROM round_space_results WHERE space_id = $1)",
    )
    .bind(space_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        // Space exists but wasn't deleted due to auction history
        return Err(StoreError::SpaceHasAuctionHistory);
    }

    Ok(())
}

pub async fn soft_delete_space(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Coleader, pool)
            .await?;

    let now = time_source.now().to_sqlx();

    let result = sqlx::query(
        "UPDATE spaces SET deleted_at = $2, updated_at = $2 WHERE id = $1",
    )
    .bind(space_id)
    .bind(now)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StoreError::SpaceNotFound);
    }

    Ok(())
}

pub async fn restore_space(
    space_id: &SpaceId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let (space, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Coleader, pool)
            .await?;

    let now = time_source.now().to_sqlx();

    let result = sqlx::query(
        "UPDATE spaces SET deleted_at = NULL, updated_at = $2 WHERE id = $1",
    )
    .bind(space_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| map_space_name_unique_error(e, &space.name))?;

    if result.rows_affected() == 0 {
        return Err(StoreError::SpaceNotFound);
    }

    Ok(())
}

pub async fn list_spaces(
    site_id: &SiteId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::Space>, StoreError> {
    // Get the site and validate user permissions
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await?;

    let _ = get_validated_member(user_id, &site.community_id, pool).await?;

    let spaces = sqlx::query_as::<_, Space>(
        "SELECT * FROM spaces WHERE site_id = $1 ORDER BY name",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;

    Ok(spaces.into_iter().map(Into::into).collect())
}

/// Get an auction and validate that the user has the required permission
/// level in the site's community. Returns both the auction and the
/// validated member if successful.
async fn get_validated_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    required_permission: PermissionLevel,
    pool: &PgPool,
) -> Result<(Auction, ValidatedMember), StoreError> {
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => StoreError::AuctionNotFound,
                e => StoreError::Database(e),
            })?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let actor = get_validated_member(user_id, &community_id, pool).await?;

    if !required_permission.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: required_permission,
        });
    }

    Ok((auction, actor))
}

pub async fn create_auction(
    details: &payloads::Auction,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::AuctionId, StoreError> {
    // Get the site and validate user permissions
    let community_id = get_site_community_id(&details.site_id, pool).await?;
    let actor = get_validated_member(user_id, &community_id, pool).await?;

    if !PermissionLevel::Coleader.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        });
    }

    // Check if the site has been deleted
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(details.site_id)
        .fetch_one(pool)
        .await?;

    if site.deleted_at.is_some() {
        return Err(StoreError::SiteDeleted);
    }

    let mut tx = pool.begin().await?;

    // Create auction params first
    let auction_params_id =
        create_auction_params(&details.auction_params, &mut tx, time_source)
            .await?;

    let auction_id = sqlx::query_as::<_, Auction>(
        "INSERT INTO auctions (
            site_id,
            possession_start_at,
            possession_end_at,
            start_at,
            auction_params_id,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $6) RETURNING *",
    )
    .bind(details.site_id)
    .bind(details.possession_start_at.to_sqlx())
    .bind(details.possession_end_at.to_sqlx())
    .bind(details.start_at.to_sqlx())
    .bind(auction_params_id)
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut *tx)
    .await?
    .id;

    tx.commit().await?;

    Ok(auction_id)
}

pub async fn read_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::Auction, StoreError> {
    let (auction, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    let auction_params = sqlx::query_as::<_, AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(&auction.auction_params_id)
    .fetch_one(pool)
    .await?;

    Ok(auction.with_params(auction_params))
}

pub async fn delete_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let (_, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Coleader,
        pool,
    )
    .await?;

    sqlx::query("DELETE FROM auctions WHERE id = $1")
        .bind(auction_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_auctions(
    site_id: &SiteId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::Auction>, StoreError> {
    // Get the site and validate user permissions
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await?;

    let _ = get_validated_member(user_id, &site.community_id, pool).await?;

    let auctions = sqlx::query_as::<_, Auction>(
        "SELECT * FROM auctions WHERE site_id = $1 ORDER BY start_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;

    // Convert each auction with its params
    let mut responses = Vec::new();
    for auction in auctions {
        let auction_params = sqlx::query_as::<_, AuctionParams>(
            "SELECT * FROM auction_params WHERE id = $1",
        )
        .bind(&auction.auction_params_id)
        .fetch_one(pool)
        .await?;

        responses.push(auction.with_params(auction_params));
    }

    Ok(responses)
}

pub async fn get_auction_round(
    round_id: &payloads::AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::AuctionRound, StoreError> {
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await?;

    // Validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    Ok(round.into_response())
}

pub async fn list_auction_rounds(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::AuctionRound>, StoreError> {
    // First validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    let rounds = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE auction_id = $1 ORDER BY round_num",
    )
    .bind(auction_id)
    .fetch_all(pool)
    .await?;

    Ok(rounds.into_iter().map(|r| r.into_response()).collect())
}

pub async fn get_round_space_result(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::RoundSpaceResult, StoreError> {
    // Verify user has access to the space
    get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
        .await?;

    // Fetch the round_space_result
    let db_result = sqlx::query_as::<_, RoundSpaceResult>(
        "SELECT * FROM round_space_results WHERE space_id = $1 AND round_id = $2",
    )
    .bind(space_id)
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::RoundSpaceResultNotFound,
        e => e.into(),
    })?;

    // Get the space to find its community
    let space =
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
            .bind(space_id)
            .fetch_one(pool)
            .await?;
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(space.site_id)
        .fetch_one(pool)
        .await?;

    // Fetch user identity
    let user_identities = get_user_identities(
        &[db_result.winning_user_id],
        &site.community_id,
        pool,
    )
    .await?;

    let winner = user_identities
        .get(&db_result.winning_user_id)
        .cloned()
        .ok_or(StoreError::UserNotFound)?;

    Ok(payloads::RoundSpaceResult {
        space_id: db_result.space_id,
        round_id: db_result.round_id,
        winner,
        value: db_result.value,
    })
}

pub async fn list_round_space_results_for_round(
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::RoundSpaceResult>, StoreError> {
    // Verify user has access to the auction round
    let auction_round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => e.into(),
    })?;

    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    // Fetch round space results
    let db_results = sqlx::query_as::<_, RoundSpaceResult>(
        "SELECT * FROM round_space_results WHERE round_id = $1",
    )
    .bind(round_id)
    .fetch_all(pool)
    .await?;

    with_user_identities(
        db_results,
        |r| r.winning_user_id,
        |r, winner| {
            Ok(payloads::RoundSpaceResult {
                space_id: r.space_id,
                round_id: r.round_id,
                winner,
                value: r.value,
            })
        },
        &community_id,
        pool,
    )
    .await
}

pub async fn create_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let mut tx = pool.begin().await?;
    create_bid_tx(space_id, round_id, user_id, &mut tx, time_source, pool)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn create_bid_tx(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
    pool: &PgPool, // for get_validated_space
) -> Result<(), StoreError> {
    // Get the space to validate user permissions and check availability
    let (space, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    // Ensure the space is available for bidding
    if !space.is_available {
        return Err(StoreError::SpaceNotAvailable);
    }

    // Check if the space has been deleted
    if space.deleted_at.is_some() {
        return Err(StoreError::SpaceDeleted);
    }

    // Check if the site has been deleted
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(space.site_id)
        .fetch_one(pool)
        .await?;

    if site.deleted_at.is_some() {
        return Err(StoreError::SiteDeleted);
    }

    // Verify the round exists and is ongoing
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => StoreError::Database(e),
    })?;

    let now = time_source.now();
    if now < round.start_at {
        return Err(StoreError::RoundNotStarted);
    }
    if now >= round.end_at {
        return Err(StoreError::RoundEnded);
    }

    // Check if user is already the standing high bidder from the previous round
    if round.round_num > 0 {
        let previous_round = sqlx::query_as::<_, AuctionRound>(
            "SELECT * FROM auction_rounds
            WHERE auction_id = $1 AND round_num = $2",
        )
        .bind(round.auction_id)
        .bind(round.round_num - 1)
        .fetch_one(&mut **tx)
        .await?;

        let is_winning = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                SELECT 1 FROM round_space_results
                WHERE round_id = $1
                AND space_id = $2
                AND winning_user_id = $3
            )",
        )
        .bind(previous_round.id)
        .bind(space_id)
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?;

        if is_winning {
            return Err(StoreError::AlreadyWinningSpace);
        }
    }

    // If not first round, check eligibility
    if round.round_num > 0 {
        // Get user's eligibility for this round
        let eligibility = sqlx::query_scalar::<_, f64>(
            "SELECT eligibility FROM user_eligibilities
            WHERE round_id = $1 AND user_id = $2",
        )
        .bind(round_id)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(StoreError::NoEligibility)?;

        // Get all spaces this user is currently bidding on or winning in this round
        let active_spaces = sqlx::query_scalar::<_, SpaceId>(
            "SELECT space_id FROM (
                SELECT space_id FROM bids
                WHERE round_id = $1 AND user_id = $2
                UNION
                SELECT space_id FROM round_space_results rsr
                JOIN auction_rounds ar ON rsr.round_id = ar.id
                WHERE ar.auction_id = $3
                AND ar.round_num = $4
                AND winning_user_id = $2
            ) spaces",
        )
        .bind(round_id)
        .bind(user_id)
        .bind(round.auction_id)
        .bind(round.round_num - 1)
        .fetch_all(&mut **tx)
        .await?;

        // Calculate total eligibility points including the new space
        let mut total_points = space.eligibility_points;
        total_points +=
            calculate_total_eligibility_points(&active_spaces, pool).await?;

        // Check if total would exceed eligibility
        if total_points > eligibility {
            return Err(StoreError::ExceedsEligibility {
                available: eligibility,
                required: total_points,
            });
        }
    }

    // Check credit limit before creating bid
    // Get and lock the account for this user in the community
    let account = currency::get_account_for_update_tx(
        &site.community_id,
        payloads::AccountOwner::Member(*user_id),
        tx,
    )
    .await?;

    // Get bid increment from auction params
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(round.auction_id)
            .fetch_one(&mut **tx)
            .await?;

    let auction_params = sqlx::query_as::<_, AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(auction.auction_params_id)
    .fetch_one(&mut **tx)
    .await?;

    // Calculate the amount this bid will lock
    // Get previous round's value for this space (if any)
    let prev_value: Option<Decimal> = if round.round_num > 0 {
        let prev_round_id: Option<payloads::AuctionRoundId> =
            sqlx::query_scalar(
                "SELECT id FROM auction_rounds
                WHERE auction_id = $1 AND round_num = $2",
            )
            .bind(round.auction_id)
            .bind(round.round_num - 1)
            .fetch_optional(&mut **tx)
            .await?;

        if let Some(prev_id) = prev_round_id {
            sqlx::query_scalar(
                "SELECT value FROM round_space_results
                WHERE round_id = $1 AND space_id = $2",
            )
            .bind(prev_id)
            .bind(space_id)
            .fetch_optional(&mut **tx)
            .await?
        } else {
            None
        }
    } else {
        None
    };

    // Bid amount = (prev value + bid increment) OR zero
    let bid_amount = prev_value
        .map(|v| v + auction_params.bid_increment)
        .unwrap_or(Decimal::ZERO);

    // Check if user has sufficient credit for this bid
    currency::check_sufficient_credit_tx(&account.id, bid_amount, tx).await?;

    // Create the bid
    sqlx::query(
        "INSERT INTO bids (space_id, round_id, user_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $4)",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .bind(time_source.now().to_sqlx())
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn get_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Bid, StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    let bid = sqlx::query_as::<_, Bid>(
        "SELECT * FROM bids WHERE space_id = $1 AND round_id = $2 AND user_id = $3",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::BidNotFound,
        e => StoreError::Database(e),
    })?;

    Ok(bid)
}

pub async fn list_bids(
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<Bid>, StoreError> {
    // Verify user has access to the auction round
    let auction_round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => e.into(),
    })?;

    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    let bids = sqlx::query_as::<_, Bid>(
        "SELECT * FROM bids WHERE round_id = $1 AND user_id = $2",
    )
    .bind(round_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(bids)
}

pub async fn delete_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    // Verify the round exists and is ongoing
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => StoreError::Database(e),
    })?;

    let now = time_source.now();
    if now < round.start_at {
        return Err(StoreError::RoundNotStarted);
    }
    if now >= round.end_at {
        return Err(StoreError::RoundEnded);
    }

    // Delete the bid
    sqlx::query(
        "DELETE FROM bids WHERE space_id = $1 AND round_id = $2 AND user_id = $3",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

// Site Image CRUD Operations

pub async fn create_site_image(
    details: &payloads::requests::CreateSiteImage,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::SiteImageId, StoreError> {
    // Validate user is a member of the community
    let actor =
        get_validated_member(user_id, &details.community_id, pool).await?;

    // Check if user has at least coleader permissions
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    let site_image = sqlx::query_as::<_, payloads::responses::SiteImage>(
        "INSERT INTO site_images (community_id, name, image_data, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $4)
         RETURNING *",
    )
    .bind(details.community_id)
    .bind(&details.name)
    .bind(&details.image_data)
    .bind(time_source.now().to_sqlx())
    .fetch_one(pool)
    .await?;

    Ok(site_image.id)
}

pub async fn get_site_image(
    site_image_id: &payloads::SiteImageId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::SiteImage, StoreError> {
    let site_image = sqlx::query_as::<_, payloads::responses::SiteImage>(
        "SELECT * FROM site_images WHERE id = $1",
    )
    .bind(site_image_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::SiteImageNotFound,
        e => StoreError::Database(e),
    })?;

    // Validate user is a member of the community
    let _ =
        get_validated_member(user_id, &site_image.community_id, pool).await?;

    Ok(site_image)
}

pub async fn update_site_image(
    details: &payloads::requests::UpdateSiteImage,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::responses::SiteImage, StoreError> {
    // First, get the existing site image to check permissions
    let existing_site_image =
        sqlx::query_as::<_, payloads::responses::SiteImage>(
            "SELECT * FROM site_images WHERE id = $1",
        )
        .bind(details.id)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StoreError::SiteImageNotFound,
            e => StoreError::Database(e),
        })?;

    // Validate user is a member of the community with coleader permissions
    let actor =
        get_validated_member(user_id, &existing_site_image.community_id, pool)
            .await?;
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    // Update the site image
    let updated_site_image =
        sqlx::query_as::<_, payloads::responses::SiteImage>(
            "UPDATE site_images
         SET name = COALESCE($2, name),
             image_data = COALESCE($3, image_data),
             updated_at = $4
         WHERE id = $1
         RETURNING *",
        )
        .bind(details.id)
        .bind(&details.name)
        .bind(&details.image_data)
        .bind(time_source.now().to_sqlx())
        .fetch_one(pool)
        .await?;

    Ok(updated_site_image)
}

pub async fn delete_site_image(
    site_image_id: &payloads::SiteImageId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    // First, get the existing site image to check permissions
    let existing_site_image =
        sqlx::query_as::<_, payloads::responses::SiteImage>(
            "SELECT * FROM site_images WHERE id = $1",
        )
        .bind(site_image_id)
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StoreError::SiteImageNotFound,
            e => StoreError::Database(e),
        })?;

    // Validate user is a member of the community with coleader permissions
    let actor =
        get_validated_member(user_id, &existing_site_image.community_id, pool)
            .await?;
    if !actor.0.role.is_ge_coleader() {
        return Err(StoreError::RequiresColeaderPermissions);
    }

    // Delete the site image
    sqlx::query("DELETE FROM site_images WHERE id = $1")
        .bind(site_image_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_site_images(
    community_id: &payloads::CommunityId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::SiteImage>, StoreError> {
    // Validate user is a member of the community
    let _ = get_validated_member(user_id, community_id, pool).await?;

    let site_images = sqlx::query_as::<_, payloads::responses::SiteImage>(
        "SELECT * FROM site_images WHERE community_id = $1 ORDER BY name",
    )
    .bind(community_id)
    .fetch_all(pool)
    .await?;

    Ok(site_images)
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
