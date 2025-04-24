pub mod model {

    use derive_more::Display;
    use jiff_sqlx::{Time, Timestamp};
    use rust_decimal::Decimal;
    use sqlx::FromRow;
    use sqlx_postgres::types::PgInterval;
    use uuid::Uuid;

    /// Id type wrapper helps ensure we don't mix up ids for different tables.
    ///
    /// Display is derived to make it easier to log events with the id.
    #[derive(Debug, Clone, PartialEq, Eq, Display, sqlx::Type)]
    #[sqlx(transparent)]
    pub struct CommunityId(pub Uuid);

    #[derive(Debug, Clone, PartialEq, Eq, FromRow)]
    pub struct Community {
        pub id: CommunityId,
        pub name: String,
        pub created_at: Timestamp,
        pub updated_at: Timestamp,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Display, sqlx::Type)]
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
    pub struct TokenId(pub Uuid);

    #[derive(Debug, Clone, FromRow)]
    pub struct Token {
        pub id: TokenId,
        pub action: String,
        pub used: bool,
        pub expires_at: Timestamp,
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
        pub round_duration: PgInterval,
        pub bid_increment: Decimal,
        pub activity_rule_params: serde_json::Value,
        pub created_at: Timestamp,
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
        pub open_time: Time,
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
}

pub mod community {
    use sqlx::{Error, PgPool};

    use super::model::Community;

    pub async fn create(conn: &PgPool, name: &str) -> Result<Community, Error> {
        sqlx::query_as::<_, Community>(
            "INSERT INTO communities (name) VALUES ($1) RETURNING *;",
        )
        .bind(name)
        .fetch_one(conn)
        .await
    }
}

pub mod user {
    use sqlx::{Error, PgPool};

    use super::model::{User, UserId};

    /// Create a new user as would happen during signup.
    pub async fn create(
        conn: &PgPool,
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
        .fetch_one(conn)
        .await
    }

    /// Create a new user as would happen during signup.
    pub async fn read(conn: &PgPool, id: &UserId) -> Result<User, Error> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1;")
            .bind(id)
            .fetch_one(conn)
            .await
    }

    /// Update fields that are not in the signup process.
    pub async fn update(conn: &PgPool, user: &User) -> Result<User, Error> {
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
        .fetch_one(conn)
        .await
    }

    pub async fn delete(conn: &PgPool, id: &UserId) -> Result<User, Error> {
        sqlx::query_as::<_, User>("DELETE FROM users WHERE id = $1;")
            .bind(id)
            .fetch_one(conn)
            .await
    }
}
