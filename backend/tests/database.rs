use jiff::{SignedDuration, Timestamp};
use rust_decimal::dec;
use sqlx::{Error, PgPool, migrate::Migrator};
use sqlx_postgres::types::PgInterval;
use uuid::Uuid;

use backend::model::{AuctionParams, Community, Site, User};

const DATABASE_URL: &str = "postgresql://user:password@localhost:5432";
const DEFAULT_DB: &str = "tinylvt";

pub static MIGRATOR: Migrator = sqlx::migrate!();

/// Drop guard for releasing a database that is used for a single test.
///
/// Contains a connection to the default database and the name of the
/// test-specific database to drop.
#[derive(Clone)]
struct DropDatabaseGuard(PgPool, String);

impl Drop for DropDatabaseGuard {
    fn drop(&mut self) {
        let conn = self.0.clone();
        let name = self.1.clone();
        tokio::task::spawn(async move {
            let _ = sqlx::query(&format!(r#"DROP DATABASE "{}";"#, name))
                .execute(&conn)
                .await;
        });
    }
}

/// Create a new database specific for the test and migrate it.
async fn setup_database() -> Result<(PgPool, DropDatabaseGuard), Error> {
    let default_conn =
        PgPool::connect(&format!("{DATABASE_URL}/{DEFAULT_DB}")).await?;
    let new_db = Uuid::new_v4().to_string();
    sqlx::query(&format!(r#"CREATE DATABASE "{}";"#, new_db))
        .execute(&default_conn)
        .await?;
    // If anything fails, we clean up the database with the guard
    let guard = DropDatabaseGuard(default_conn, new_db.clone());
    let conn = PgPool::connect(&format!("{DATABASE_URL}/{new_db}")).await?;
    MIGRATOR.run(&conn).await?;
    Ok((conn, guard))
}

/// Check is a timestamp is from the last ten seconds.
fn timestamp_is_recent(ts: Timestamp) -> bool {
    ts.duration_since(Timestamp::now()) < SignedDuration::from_secs(10)
}

#[tokio::test]
async fn test_community() -> Result<(), Error> {
    let (conn, _guard) = setup_database().await?;
    let name = "community".to_string();
    let community = sqlx::query_as::<_, Community>(
        "INSERT INTO communities (name) VALUES ($1) RETURNING *;",
    )
    .bind(name)
    .fetch_one(&conn)
    .await?;

    assert!(timestamp_is_recent(community.created_at.to_jiff()));
    assert!(timestamp_is_recent(community.updated_at.to_jiff()));

    let community_retrieved = sqlx::query_as::<_, Community>(
        "SELECT * FROM communities WHERE id = $1;",
    )
    .bind(&community.id)
    .fetch_one(&conn)
    .await?;

    assert_eq!(community, community_retrieved);

    Ok(())
}

#[tokio::test]
async fn test_populate() -> Result<(), Error> {
    let (conn, _guard) = setup_database().await?;
    populate_test_data(&conn).await?;
    Ok(())
}

async fn populate_test_data(conn: &PgPool) -> Result<(), Error> {
    let community_name = "Test Community";
    let community = sqlx::query_as::<_, Community>(
        "INSERT INTO communities (name) VALUES ($1) RETURNING *;",
    )
    .bind(community_name)
    .fetch_one(conn)
    .await?;

    let roles = ["leader", "coleader", "member"];
    for (i, role) in roles.iter().enumerate() {
        let user = sqlx::query_as::<_, User>(
            "INSERT INTO users (
                username,
                email,
                password_hash,
                display_name,
                email_verified,
                balance
            ) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *;",
        )
        .bind(format!("{role}_user"))
        .bind(format!("{role}@example.com"))
        .bind(format!("hashed_pw_{i}"))
        .bind(format!("{role} user"))
        .bind(true)
        .bind(dec!(1000.000000))
        .fetch_one(conn)
        .await?;

        sqlx::query(
            "INSERT INTO community_members (
                community_id,
                user_id,
                role
            ) VALUES ($1, $2, $3);",
        )
        .bind(&community.id)
        .bind(&user.id)
        .bind(role)
        .execute(conn)
        .await?;
    }

    let users = sqlx::query_as::<_, User>("SELECT * FROM users;")
        .bind(community_name)
        .fetch_all(conn)
        .await?;

    let auction_params = sqlx::query_as::<_, AuctionParams>(
        "INSERT INTO auction_params (
            round_duration,
            bid_increment,
            activity_rule_params
        ) VALUES ($1, $2, $3) RETURNING *;",
    )
    .bind(PgInterval {
        microseconds: 100_000, // 100 ms, for fast testing
        ..Default::default()
    })
    .bind(dec!(1.000000))
    .bind(serde_json::Value::String("placeholder".into()))
    .fetch_one(conn)
    .await?;

    let site = sqlx::query_as::<_, Site>(
        "INSERT INTO sites (
            community_id,
            name,
            default_auction_params_id
        ) VALUES ($1, $2, $3) RETURNING *;",
    )
    .bind(&community.id)
    .bind("Test Site")
    .bind(&auction_params.id)
    .fetch_one(conn)
    .await?;

    for i in 0..2 {
        sqlx::query(
            "INSERT INTO spaces (
                site_id,
                name,
                eligibility_points
            ) VALUES ($1, $2, $3);",
        )
        .bind(&site.id)
        .bind(format!("Space {i}"))
        .bind(100.0)
        .execute(conn)
        .await?;
    }

    Ok(())
}
