use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct LoginCredentials {
    pub username: String,
    pub password: String,
}
