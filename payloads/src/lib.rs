pub mod requests {
    use serde::{Deserialize, Serialize};

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

    #[derive(Debug, Serialize, Deserialize)]
    pub struct CreateCommunity {
        pub name: String,
    }
}

pub mod responses {}
