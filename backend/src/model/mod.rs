use jiff_sqlx::{Span, Timestamp};
use rust_decimal::Decimal;
use sqlx::FromRow;
use uuid::Uuid;

/// Id type wrapper helps ensure we don't mix up ids for different tables.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct CommunityId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct Community {
    pub id: CommunityId,
    pub name: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct UserId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub email_verified: bool,
    pub balance: Decimal,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct RoleId(pub String);

#[derive(Debug, Clone, FromRow)]
pub struct UserRole {
    pub id: RoleId,
    pub display_name: String,
    pub rank: i32,
    pub scope: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct CommunityMember {
    pub community_id: CommunityId,
    pub user_id: UserId,
    pub role: RoleId,
    pub joined_at: Timestamp,
    pub active_at: Timestamp,
    pub inactive_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct CommunityMembershipScheduleId(pub String);

#[derive(Debug, Clone, FromRow)]
pub struct CommunityMembershipSchedule {
    pub id: CommunityMembershipScheduleId,
    pub community_id: CommunityId,
    pub start_at: Timestamp,
    pub end_at: Timestamp,
    pub email: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct AuctionParamsId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct AuctionParams {
    pub id: AuctionParamsId,
    /// Span is only for decode, need to use sqlx_postgres::types::PgInterval
    /// at encoding time.
    pub round_duration: Span,
    pub bid_increment: Decimal,
    pub activity_rule_params: serde_json::Value,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct SiteId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct Site {
    pub id: SiteId,
    pub community_id: CommunityId,
    pub name: String,
    pub description: Option<String>,
    pub default_auction_params_id: AuctionParamsId,
    pub is_available: bool,
    pub site_image_id: Option<SiteImageId>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct SpaceId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct Space {
    pub id: SpaceId,
    pub site_id: SiteId,
    pub name: String,
    pub description: Option<String>,
    pub eligibility_points: f64,
    pub is_available: bool,
    pub site_image_id: Option<SiteImageId>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct SiteImageId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct SiteImage {
    pub id: SiteImageId,
    pub site_id: SiteId,
    pub name: String,
    pub image_data: Vec<u8>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct AuctionId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct Auction {
    pub id: AuctionId,
    pub site_id: SiteId,
    pub start_at: Timestamp,
    pub end_at: Option<Timestamp>,
    pub auction_params_id: AuctionParamsId,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct AuctionRoundId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct AuctionRound {
    pub id: AuctionRoundId,
    pub auction_id: AuctionId,
    pub round_num: i32,
    pub start_at: Timestamp,
    pub end_at: Timestamp,
    pub eligibility_threshold: f64, // fractional eligibility; 0-1
}

#[derive(Debug, Clone, FromRow)]
pub struct SpaceRound {
    pub space_id: SpaceId,
    pub round_id: AuctionRoundId,
    pub winning_user_id: Option<UserId>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Bid {
    pub space_id: SpaceId,
    pub round_id: AuctionRoundId,
    pub user_id: UserId,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserEligibility {
    pub user_id: UserId,
    pub round_id: AuctionRoundId,
    pub eligibility: f64,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserValues {
    pub user_id: UserId,
    pub space_id: SpaceId,
    pub value: Decimal,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, FromRow)]
pub struct UseProxyBidding {
    pub user_id: UserId,
    pub auction_id: AuctionId,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct AuditLogId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct AuditLog {
    pub id: AuditLogId,
    pub actor_id: Option<UserId>,
    pub action: String,
    pub target_table: Option<String>,
    pub target_id: Option<Uuid>,
    pub details: Option<serde_json::Value>,
    pub created_at: Timestamp,
}
