use derive_more::Display;
use jiff::{Timestamp, civil::Time};
use jiff_sqlx::Timestamp as SqlxTs;
use rust_decimal::Decimal;
use sqlx::{Error, FromRow, PgPool};
use sqlx_postgres::types::PgInterval;
use uuid::Uuid;

use payloads::{CommunityId, UserId, responses::Community};

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
}

#[derive(Debug, Clone, PartialEq, Eq, Display, sqlx::Type)]
#[sqlx(transparent)]
pub struct TokenId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct Token {
    pub id: TokenId,
    pub action: String,
    pub used: bool,
    #[sqlx(try_from = "SqlxTs")]
    pub expires_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, sqlx::Type)]
#[sqlx(transparent)]
pub struct RoleId(pub String);

impl RoleId {
    pub fn is_mmeber(&self) -> bool {
        self.0 == "member"
    }
    pub fn is_moderator(&self) -> bool {
        self.0 == "moderator"
    }
    pub fn is_coleader(&self) -> bool {
        self.0 == "coleader"
    }
    pub fn is_leader(&self) -> bool {
        self.0 == "leader"
    }

    /// If the role is moderator or higher rank
    pub fn is_ge_moderator(&self) -> bool {
        self.is_moderator() || self.is_ge_coleader()
    }
    pub fn is_ge_coleader(&self) -> bool {
        self.is_coleader() || self.is_leader()
    }
}

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
    #[sqlx(try_from = "SqlxTs")]
    pub joined_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub active_at: Timestamp,
    #[sqlx(try_from = "OptionalTimestamp")]
    pub inactive_at: Option<Timestamp>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(sqlx::Type)]
#[sqlx(transparent)]
struct OptionalTimestamp(Option<SqlxTs>);

impl From<OptionalTimestamp> for Option<Timestamp> {
    fn from(x: OptionalTimestamp) -> Option<Timestamp> {
        x.0.map(|x| x.to_jiff())
    }
}

/// A type that can only exist if the interior CommunityMember has been
/// validated to exist.
pub struct ValidatedMember(CommunityMember);

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct CommunityMembershipScheduleId(pub String);

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

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct AuctionParamsId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct AuctionParams {
    pub id: AuctionParamsId,
    pub round_duration: PgInterval,
    pub bid_increment: Decimal,
    pub activity_rule_params: serde_json::Value,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct OpenHoursId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct OpenHours {
    pub id: OpenHoursId,
    pub timezone: String,
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
    pub possession_period: PgInterval,
    pub auction_lead_time: PgInterval,
    pub open_hours_id: Option<OpenHoursId>,
    pub is_available: bool,
    pub site_image_id: Option<SiteImageId>,
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
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
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
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
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub struct AuctionId(pub Uuid);

#[derive(Debug, Clone, FromRow)]
pub struct Auction {
    pub id: AuctionId,
    pub site_id: SiteId,
    #[sqlx(try_from = "SqlxTs")]
    pub start_at: Timestamp,
    #[sqlx(try_from = "OptionalTimestamp")]
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
    #[sqlx(try_from = "SqlxTs")]
    pub start_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
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
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
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
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
    #[sqlx(try_from = "SqlxTs")]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, FromRow)]
pub struct UseProxyBidding {
    pub user_id: UserId,
    pub auction_id: AuctionId,
    #[sqlx(try_from = "SqlxTs")]
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
    #[sqlx(try_from = "SqlxTs")]
    pub created_at: Timestamp,
}

/// Create a community and add the creating user as the leader.
pub async fn create_community(
    name: &str,
    leader_id: UserId, // initial leader of community
    pool: &PgPool,
) -> Result<Community, Error> {
    let mut tx = pool.begin().await?;

    let community = sqlx::query_as::<_, Community>(
        "INSERT INTO communities (name) VALUES ($1) RETURNING *;",
    )
    .bind(name)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO community_members (community_id, user_id, role)
        VALUES ($1, $2, $3);",
    )
    .bind(&community.id)
    .bind(leader_id)
    .bind("leader")
    .execute(&mut *tx)
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
) -> Result<User, Error> {
    sqlx::query_as::<_, User>(
        "INSERT INTO users (
                username,
                email,
                password_hash
            )
            VALUES ($1, $2, $3)
            RETURNING *;",
    )
    .bind(username)
    .bind(email)
    .bind(password_hash)
    .fetch_one(pool)
    .await
}

/// Create a new user as would happen during signup.
pub async fn read_user(pool: &PgPool, id: &UserId) -> Result<User, Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1;")
        .bind(id)
        .fetch_one(pool)
        .await
}

/// Update fields that are not in the signup process.
pub async fn update_user(pool: &PgPool, user: &User) -> Result<User, Error> {
    sqlx::query_as::<_, User>(
        "UPDATE users
            SET display_name = $1,
                email_verified = $2,
                balance = $3
            WHERE id = $4
            RETURNING *;",
    )
    .bind(&user.display_name)
    .bind(user.email_verified)
    .bind(user.balance)
    .bind(&user.id)
    .fetch_one(pool)
    .await
}

pub async fn delete_user(conn: &PgPool, id: &UserId) -> Result<User, Error> {
    sqlx::query_as::<_, User>("DELETE FROM users WHERE id = $1;")
        .bind(id)
        .fetch_one(conn)
        .await
}

pub async fn get_validated_member(
    conn: &PgPool,
    user_id: &UserId,
    community_id: &CommunityId,
) -> Result<ValidatedMember, Error> {
    Ok(ValidatedMember(
        sqlx::query_as::<_, CommunityMember>(
            "SELECT * FROM community_members WHERE
            community_id = $1 AND user_id = $2;",
        )
        .bind(community_id)
        .bind(user_id)
        .fetch_one(conn)
        .await?,
    ))
}

pub async fn invite_community_member(
    conn: &PgPool,
    actor: &ValidatedMember,
    user_to_add: &UserId,
) -> anyhow::Result<()> {
    if !actor.0.role.is_ge_moderator() {
        return Err(anyhow::anyhow!(
            "Must be a moderator to add community members."
        ));
    }
    sqlx::query(
        "INSERT INTO community_members (community_id, user_id, role)
        VALUES ($1, $2, $3);",
    )
    .bind(&actor.0.community_id)
    .bind(user_to_add)
    .bind("member")
    .execute(conn)
    .await?;
    Ok(())
}
