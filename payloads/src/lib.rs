pub mod requests {
    use super::{CommunityId, UserId};
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
        pub new_member_user_id: UserId,
    }
}

pub mod responses {
    use super::CommunityId;
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
}

use derive_more::Display;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Id type wrappers help ensure we don't mix up ids for different tables.
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct CommunityId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct UserId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct TokenId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
#[cfg_attr(feature = "use-sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct RoleId(pub String);
