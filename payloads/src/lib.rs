pub mod requests {
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
}

pub mod responses {}
