//! Points a dev community at the Stripe test Connect account, so that funding
//! flows can be exercised locally without going through onboarding.
//!
//! Reads `DATABASE_URL` and `TEST_CONNECT_ACCOUNT_ID` from `.env`.
//!
//! Usage: cargo run -p dev-server --bin set_stripe_account [community name]

use anyhow::{Context, Result};
use sqlx::{PgPool, Row};
use uuid::Uuid;

const DEFAULT_COMMUNITY: &str = "Test Community";

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let community_name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_COMMUNITY.to_string());

    let database_url = env_var("DATABASE_URL")?;
    let account_id = env_var("TEST_CONNECT_ACCOUNT_ID")?;

    let pool = PgPool::connect(&database_url)
        .await
        .context("failed to connect to the database")?;

    // A zero-row update means the name was wrong, not that the work is done, so
    // surface it as an error rather than reporting success.
    let updated = sqlx::query(
        "update communities
         set stripe_account_id = $1, stripe_charges_enabled = true
         where name = $2
         returning id",
    )
    .bind(&account_id)
    .bind(&community_name)
    .fetch_optional(&pool)
    .await
    .context("failed to update the community")?
    .map(|row| row.get::<Uuid, _>("id"))
    .with_context(|| format!("no community named {community_name:?}"))?;

    println!("Set {community_name:?} ({updated}) to {account_id}");

    Ok(())
}

fn env_var(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("{key} not set in .env"))
}
