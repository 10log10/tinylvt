//! Some basic database tests.
//!
//! Though api::store defines its own level of API interface, most tests are
//! at the http route level.
use std::str::FromStr;

use jiff::{
    SignedDuration, Timestamp,
    civil::{Time, Weekday},
};
use jiff_sqlx::ToSqlx;
use rust_decimal::dec;
use sqlx::{Error, PgPool};
use sqlx_postgres::types::PgInterval;

use api::store::{
    self, AuctionParams, AuctionParamsId, OpenHours, OpenHoursId,
    OpenHoursWeekday, Site, Space, StoreError, User,
};
use payloads::{CommunityId, SiteId, responses::Community};

use test_helpers::spawn_app;

const HOUR_MICROSECONDS: i64 = 60 * 60 * 1_000_000;

/// Check is a timestamp is from the last ten seconds.
fn timestamp_is_recent(ts: Timestamp) -> bool {
    ts.duration_since(Timestamp::now()) < SignedDuration::from_secs(10)
}

#[tokio::test]
async fn test_community() -> Result<(), Error> {
    let app = spawn_app().await;
    let conn = app.db_pool;
    let name = "community".to_string();
    let community = sqlx::query_as::<_, Community>(
        "INSERT INTO communities (name, new_members_default_active)
        VALUES ($1, false) RETURNING *;",
    )
    .bind(name)
    .fetch_one(&conn)
    .await?;

    assert!(timestamp_is_recent(community.created_at));
    assert!(timestamp_is_recent(community.updated_at));

    let community_retrieved = sqlx::query_as::<_, Community>(
        "SELECT * FROM communities WHERE id = $1;",
    )
    .bind(community.id)
    .fetch_one(&conn)
    .await?;

    assert_eq!(community, community_retrieved);

    Ok(())
}

#[tokio::test]
async fn test_populate() -> Result<(), StoreError> {
    let app = spawn_app().await;
    let conn = &app.db_pool;

    let community = sqlx::query_as::<_, Community>(
        "INSERT INTO communities (name) VALUES ($1) RETURNING *;",
    )
    .bind("Test Community")
    .fetch_one(conn)
    .await?;
    println!("1");
    let _users = populate_users(conn, &community.id).await?;
    println!("2");
    // check that we get a unique constaint error if attempting to populate the
    // same usernames
    let result = populate_users(conn, &community.id).await;
    assert!(matches!(result, Err(StoreError::NotUnique(_))));
    let open_hours = populate_open_hours(conn).await?;
    let auction_params = populate_auction_params(conn).await?;
    let site =
        populate_site(conn, &community.id, &auction_params.id, &open_hours.id)
            .await?;
    let _spaces = populate_spaces(conn, &site.id).await?;

    Ok(())
}

async fn populate_users(
    conn: &PgPool,
    community_id: &CommunityId,
) -> Result<Vec<User>, StoreError> {
    use payloads::Role;
    let roles = [Role::Leader, Role::Coleader, Role::Member];

    for (i, role) in roles.iter().enumerate() {
        let mut user = store::create_user(
            conn,
            &format!("{role}_user"),
            &format!("{role}@example.com"),
            &format!("hashed_pw_{i}"),
        )
        .await?;
        user.display_name = Some(format!("{role} user"));
        user.email_verified = true;
        user.balance = dec!(1000.000000);
        store::update_user(conn, &user).await?;

        sqlx::query(
            "INSERT INTO community_members (
                community_id,
                user_id,
                role
          ) VALUES ($1, $2, $3);",
        )
        .bind(community_id)
        .bind(user.id)
        .bind(role)
        .execute(conn)
        .await?;
    }

    Ok(sqlx::query_as::<_, User>("SELECT * FROM users;")
        .fetch_all(conn)
        .await?)
}

async fn populate_open_hours(conn: &PgPool) -> Result<OpenHours, Error> {
    let open_hours = sqlx::query_as::<_, OpenHours>(
        "INSERT INTO open_hours DEFAULT VALUES RETURNING *;",
    )
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
        .bind(open_hours.id)
        .bind(weekday.to_monday_one_offset() as i16)
        .bind(Time::from_str("09:00").unwrap().to_sqlx())
        .bind(Time::from_str("17:00").unwrap().to_sqlx())
        .fetch_one(conn)
        .await?;
    }

    Ok(open_hours)
}

async fn populate_auction_params(
    conn: &PgPool,
) -> Result<AuctionParams, Error> {
    sqlx::query_as::<_, AuctionParams>(
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
    .bind(
        serde_json::Value::from_str(r#"{"eligibility_progression":[]}"#)
            .unwrap(),
    )
    .fetch_one(conn)
    .await
}

async fn populate_site(
    conn: &PgPool,
    community_id: &CommunityId,
    auction_params_id: &AuctionParamsId,
    open_hours_id: &OpenHoursId,
) -> Result<Site, Error> {
    sqlx::query_as::<_, Site>(
        "INSERT INTO sites (
            community_id,
            name,
            default_auction_params_id,
            possession_period,
            auction_lead_time,
            proxy_bidding_lead_time,
            open_hours_id,
            timezone
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *;",
    )
    .bind(community_id)
    .bind("Test Site")
    .bind(auction_params_id)
    .bind(PgInterval {
        microseconds: HOUR_MICROSECONDS,
        ..Default::default()
    })
    .bind(PgInterval {
        microseconds: HOUR_MICROSECONDS,
        ..Default::default()
    })
    .bind(PgInterval {
        microseconds: HOUR_MICROSECONDS,
        ..Default::default()
    })
    .bind(open_hours_id)
    .bind("America/Los_Angeles")
    .fetch_one(conn)
    .await
}

async fn populate_spaces(
    conn: &PgPool,
    site_id: &SiteId,
) -> Result<Vec<Space>, Error> {
    let mut spaces = vec![];
    for i in 0..2 {
        let space = sqlx::query_as::<_, Space>(
            "INSERT INTO spaces (
                site_id,
                name,
                eligibility_points
            ) VALUES ($1, $2, $3) RETURNING *;",
        )
        .bind(site_id)
        .bind(format!("Space {i}"))
        .bind(100.0)
        .fetch_one(conn)
        .await?;
        spaces.push(space);
    }
    Ok(spaces)
}
