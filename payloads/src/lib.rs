pub mod requests {
    use crate::CommunityId;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub struct LoginCredentials {
        pub username: String,
        pub password: String,
    }

    pub const EMAIL_MAX_LEN: usize = 255;
    pub const USERNAME_MAX_LEN: usize = 50;

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
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct InviteCommunityMember {
        pub community_id: CommunityId,
        pub new_member_email: Option<String>,
    }
}

pub mod responses {
    use crate::{CommunityId, InviteId};
    use jiff::Timestamp;
    use jiff_sqlx::Timestamp as SqlxTs;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
    pub struct Community {
        pub id: CommunityId,
        pub name: String,
        #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
        pub created_at: Timestamp,
        #[cfg_attr(feature = "use-sqlx", sqlx(try_from = "SqlxTs"))]
        pub updated_at: Timestamp,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "use-sqlx", derive(sqlx::FromRow))]
    pub struct CommunityInvite {
        pub id: InviteId,
        pub community_name: String,
        #[sqlx(try_from = "SqlxTs")]
        pub created_at: Timestamp,
    }
}

use derive_more::Display;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Id type wrappers help ensure we don't mix up ids for different tables.
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct UserId(pub Uuid);

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct CommunityId(pub Uuid);

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct InviteId(pub Uuid);

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct TokenId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct RoleId(pub String);

impl RoleId {
    pub fn member() -> Self {
        Self("member".into())
    }
    pub fn moderator() -> Self {
        Self("moderator".into())
    }
    pub fn coleader() -> Self {
        Self("coleader".into())
    }
    pub fn leader() -> Self {
        Self("leader".into())
    }

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
