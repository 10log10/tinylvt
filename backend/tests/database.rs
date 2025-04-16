use std::str::FromStr;

use jiff::{
    SignedDuration, Timestamp,
    civil::{Time, Weekday},
};
use jiff_sqlx::ToSqlx;
use rust_decimal::dec;
use sqlx::{Error, PgPool, migrate::Migrator};
use sqlx_postgres::types::PgInterval;
use uuid::Uuid;

use backend::store::{
    self,
    model::{
        AuctionParams, Community, CommunityId, OpenHours, OpenHoursId,
        OpenHoursWeekday, Site, Space, User,
    },
};

const DATABASE_URL: &str = "postgresql://user:password@localhost:5432";
const DEFAULT_DB: &str = "tinylvt";

const HOUR_MICROSECONDS: i64 = 60 * 60 * 1_000_000;

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
    let community = store::community::create(conn, "Test Community").await?;
    let _users = populate_test_users(conn, &community.id).await?;
    let open_hours_id = populate_test_open_hours(conn).await?;

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
            default_auction_params_id,
            possession_period,
            auction_lead_time,
            open_hours_id
        ) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *;",
    )
    .bind(&community.id)
    .bind("Test Site")
    .bind(&auction_params.id)
    .bind(PgInterval {
        microseconds: HOUR_MICROSECONDS,
        ..Default::default()
    })
    .bind(PgInterval {
        microseconds: HOUR_MICROSECONDS,
        ..Default::default()
    })
    .bind(open_hours_id)
    .fetch_one(conn)
    .await?;

    let mut spaces = vec![];
    for i in 0..2 {
        let space = sqlx::query_as::<_, Space>(
            "INSERT INTO spaces (
                site_id,
                name,
                eligibility_points
            ) VALUES ($1, $2, $3) RETURNING *;",
        )
        .bind(&site.id)
        .bind(format!("Space {i}"))
        .bind(100.0)
        .fetch_one(conn)
        .await?;
        spaces.push(space);
    }

    Ok(())
}

async fn populate_test_users(
    conn: &PgPool,
    community_id: &CommunityId,
) -> Result<Vec<User>, Error> {
    let roles = ["leader", "coleader", "member"];

    for (i, role) in roles.iter().enumerate() {
        let mut user = store::user::create(
            conn,
            &format!("{role}_user"),
            &format!("{role}@example.com"),
            &format!("hashed_pw_{i}"),
        )
        .await?;
        user.display_name = Some(format!("{role} user"));
        user.email_verified = true;
        user.balance = dec!(1000.000000);
        store::user::update(conn, &user).await?;

        sqlx::query(
            "INSERT INTO community_members (
                community_id,
                user_id,
                role
          ) VALUES ($1, $2, $3);",
        )
        .bind(community_id)
        .bind(&user.id)
        .bind(role)
        .execute(conn)
        .await?;
    }

    sqlx::query_as::<_, User>("SELECT * FROM users;")
        .fetch_all(conn)
        .await
}

async fn populate_test_open_hours(conn: &PgPool) -> Result<OpenHoursId, Error> {
    let open_hours = sqlx::query_as::<_, OpenHours>(
        "INSERT INTO open_hours (timezone) VALUES ($1) RETURNING *;",
    )
    .bind("America/Los_Angeles")
    .fetch_one(conn)
    .await?;

    for weekday in Weekday::Sunday.cycle_forward().take(7) {
        sqlx::query_as::<_, OpenHoursWeekday>(
            "INSERT INTO open_hours_weekday (
                open_hours_id,
                day_of_week,
                open_time,
                close_time
            ) VALUES ($1, $2, $3, $4) RETURNING *;",
        )
        .bind(&open_hours.id)
        .bind(weekday.to_monday_one_offset() as i16)
        .bind(Time::from_str("09:00").unwrap().to_sqlx())
        .bind(Time::from_str("17:00").unwrap().to_sqlx())
        .fetch_one(conn)
        .await?;
    }

    Ok(open_hours.id)
}
