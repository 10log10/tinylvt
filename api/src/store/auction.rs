use super::*;
use jiff_sqlx::ToSqlx;
use payloads::{
    AuctionId, AuctionRoundId, Bid, PermissionLevel, SiteId, SpaceId, UserId,
};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::time::TimeSource;

/// Calculate the total eligibility points required for a set of spaces
async fn calculate_total_eligibility_points(
    spaces: &[SpaceId],
    pool: &PgPool,
) -> Result<f64, StoreError> {
    let spaces =
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = ANY($1)")
            .bind(spaces)
            .fetch_all(pool)
            .await?;

    Ok(spaces.iter().map(|space| space.eligibility_points).sum())
}

/// Get a user's eligibility for a specific auction round
pub async fn get_eligibility(
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Option<f64>, StoreError> {
    // Verify the round exists and get auction info
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => StoreError::Database(e),
    })?;

    // Validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    // Get user's eligibility for this round
    let eligibility = sqlx::query_scalar::<_, f64>(
        "SELECT eligibility FROM user_eligibilities 
        WHERE round_id = $1 AND user_id = $2",
    )
    .bind(round_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(eligibility)
}

/// List a user's eligibility for all rounds after round 0 in an auction.
pub async fn list_eligibility(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<Option<f64>>, StoreError> {
    // Validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    // Get all rounds for this auction in order
    let rounds = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds 
        WHERE auction_id = $1 
        ORDER BY round_num",
    )
    .bind(auction_id)
    .fetch_all(pool)
    .await?;

    // Get eligibility for each round
    let mut eligibilities = Vec::with_capacity(rounds.len());
    for round in &rounds[1..] {
        let eligibility = sqlx::query_scalar::<_, f64>(
            "SELECT eligibility FROM user_eligibilities 
            WHERE round_id = $1 AND user_id = $2",
        )
        .bind(round.id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        eligibilities.push(eligibility);
    }

    Ok(eligibilities)
}
/// Get an auction and validate that the user has the required permission
/// level in the site's community. Returns both the auction and the
/// validated member if successful.
pub(super) async fn get_validated_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    required_permission: PermissionLevel,
    pool: &PgPool,
) -> Result<(Auction, ValidatedMember), StoreError> {
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => StoreError::AuctionNotFound,
                e => StoreError::Database(e),
            })?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let actor = get_validated_member(user_id, &community_id, pool).await?;

    if !required_permission.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: required_permission,
        });
    }

    Ok((auction, actor))
}

pub async fn create_auction(
    details: &payloads::Auction,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<payloads::AuctionId, StoreError> {
    // Get the site and validate user permissions
    let community_id = get_site_community_id(&details.site_id, pool).await?;
    let actor = get_validated_member(user_id, &community_id, pool).await?;

    if !PermissionLevel::Coleader.validate(actor.0.role) {
        return Err(StoreError::InsufficientPermissions {
            required: PermissionLevel::Coleader,
        });
    }

    // Check if the site has been deleted
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(details.site_id)
        .fetch_one(pool)
        .await?;

    if site.deleted_at.is_some() {
        return Err(StoreError::SiteDeleted);
    }

    let mut tx = pool.begin().await?;

    // Create auction params first
    let auction_params_id =
        create_auction_params(&details.auction_params, &mut tx, time_source)
            .await?;

    let auction_id = sqlx::query_as::<_, Auction>(
        "INSERT INTO auctions (
            site_id,
            possession_start_at,
            possession_end_at,
            start_at,
            auction_params_id,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $6) RETURNING *",
    )
    .bind(details.site_id)
    .bind(details.possession_start_at.to_sqlx())
    .bind(details.possession_end_at.to_sqlx())
    .bind(details.start_at.to_sqlx())
    .bind(auction_params_id)
    .bind(time_source.now().to_sqlx())
    .fetch_one(&mut *tx)
    .await?
    .id;

    tx.commit().await?;

    Ok(auction_id)
}

pub async fn read_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::Auction, StoreError> {
    let (auction, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Member,
        pool,
    )
    .await?;

    let auction_params = sqlx::query_as::<_, AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(&auction.auction_params_id)
    .fetch_one(pool)
    .await?;

    Ok(auction.with_params(auction_params))
}

pub async fn delete_auction(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<(), StoreError> {
    let (_, _) = get_validated_auction(
        auction_id,
        user_id,
        PermissionLevel::Coleader,
        pool,
    )
    .await?;

    sqlx::query("DELETE FROM auctions WHERE id = $1")
        .bind(auction_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_auctions(
    site_id: &SiteId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::Auction>, StoreError> {
    // Get the site and validate user permissions
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(site_id)
        .fetch_one(pool)
        .await?;

    let _ = get_validated_member(user_id, &site.community_id, pool).await?;

    let auctions = sqlx::query_as::<_, Auction>(
        "SELECT * FROM auctions WHERE site_id = $1 ORDER BY start_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await?;

    // Convert each auction with its params
    let mut responses = Vec::new();
    for auction in auctions {
        let auction_params = sqlx::query_as::<_, AuctionParams>(
            "SELECT * FROM auction_params WHERE id = $1",
        )
        .bind(&auction.auction_params_id)
        .fetch_one(pool)
        .await?;

        responses.push(auction.with_params(auction_params));
    }

    Ok(responses)
}

pub async fn get_auction_round(
    round_id: &payloads::AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::responses::AuctionRound, StoreError> {
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await?;

    // Validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    Ok(round.into_response())
}

pub async fn list_auction_rounds(
    auction_id: &AuctionId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::responses::AuctionRound>, StoreError> {
    // First validate user has access to this auction's community
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    let rounds = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE auction_id = $1 ORDER BY round_num",
    )
    .bind(auction_id)
    .fetch_all(pool)
    .await?;

    Ok(rounds.into_iter().map(|r| r.into_response()).collect())
}

pub async fn get_round_space_result(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<payloads::RoundSpaceResult, StoreError> {
    // Verify user has access to the space
    get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
        .await?;

    // Fetch the round_space_result
    let db_result = sqlx::query_as::<_, RoundSpaceResult>(
        "SELECT * FROM round_space_results WHERE space_id = $1 AND round_id = $2",
    )
    .bind(space_id)
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::RoundSpaceResultNotFound,
        e => e.into(),
    })?;

    // Get the space to find its community
    let space =
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
            .bind(space_id)
            .fetch_one(pool)
            .await?;
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(space.site_id)
        .fetch_one(pool)
        .await?;

    // Fetch user identity
    let user_identities = get_user_identities(
        &[db_result.winning_user_id],
        &site.community_id,
        pool,
    )
    .await?;

    let winner = user_identities
        .get(&db_result.winning_user_id)
        .cloned()
        .ok_or(StoreError::UserNotFound)?;

    Ok(payloads::RoundSpaceResult {
        space_id: db_result.space_id,
        round_id: db_result.round_id,
        winner,
        value: db_result.value,
    })
}

pub async fn list_round_space_results_for_round(
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<payloads::RoundSpaceResult>, StoreError> {
    // Verify user has access to the auction round
    let auction_round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => e.into(),
    })?;

    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    // Fetch round space results
    let db_results = sqlx::query_as::<_, RoundSpaceResult>(
        "SELECT * FROM round_space_results WHERE round_id = $1",
    )
    .bind(round_id)
    .fetch_all(pool)
    .await?;

    with_user_identities(
        db_results,
        |r| r.winning_user_id,
        |r, winner| {
            Ok(payloads::RoundSpaceResult {
                space_id: r.space_id,
                round_id: r.round_id,
                winner,
                value: r.value,
            })
        },
        &community_id,
        pool,
    )
    .await
}

pub async fn create_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    let mut tx = pool.begin().await?;
    create_bid_tx(space_id, round_id, user_id, &mut tx, time_source, pool)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn create_bid_tx(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    time_source: &TimeSource,
    pool: &PgPool, // for get_validated_space
) -> Result<(), StoreError> {
    // Get the space to validate user permissions and check availability
    let (space, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    // Ensure the space is available for bidding
    if !space.is_available {
        return Err(StoreError::SpaceNotAvailable);
    }

    // Check if the space has been deleted
    if space.deleted_at.is_some() {
        return Err(StoreError::SpaceDeleted);
    }

    // Check if the site has been deleted
    let site = sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
        .bind(space.site_id)
        .fetch_one(pool)
        .await?;

    if site.deleted_at.is_some() {
        return Err(StoreError::SiteDeleted);
    }

    // Verify the round exists and is ongoing
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => StoreError::Database(e),
    })?;

    let now = time_source.now();
    if now < round.start_at {
        return Err(StoreError::RoundNotStarted);
    }
    if now >= round.end_at {
        return Err(StoreError::RoundEnded);
    }

    // Check if user is already the standing high bidder from the previous round
    if round.round_num > 0 {
        let previous_round = sqlx::query_as::<_, AuctionRound>(
            "SELECT * FROM auction_rounds
            WHERE auction_id = $1 AND round_num = $2",
        )
        .bind(round.auction_id)
        .bind(round.round_num - 1)
        .fetch_one(&mut **tx)
        .await?;

        let is_winning = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                SELECT 1 FROM round_space_results
                WHERE round_id = $1
                AND space_id = $2
                AND winning_user_id = $3
            )",
        )
        .bind(previous_round.id)
        .bind(space_id)
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?;

        if is_winning {
            return Err(StoreError::AlreadyWinningSpace);
        }
    }

    // If not first round, check eligibility
    if round.round_num > 0 {
        // Get user's eligibility for this round
        let eligibility = sqlx::query_scalar::<_, f64>(
            "SELECT eligibility FROM user_eligibilities
            WHERE round_id = $1 AND user_id = $2",
        )
        .bind(round_id)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(StoreError::NoEligibility)?;

        // Get all spaces this user is currently bidding on or winning in this round
        let active_spaces = sqlx::query_scalar::<_, SpaceId>(
            "SELECT space_id FROM (
                SELECT space_id FROM bids
                WHERE round_id = $1 AND user_id = $2
                UNION
                SELECT space_id FROM round_space_results rsr
                JOIN auction_rounds ar ON rsr.round_id = ar.id
                WHERE ar.auction_id = $3
                AND ar.round_num = $4
                AND winning_user_id = $2
            ) spaces",
        )
        .bind(round_id)
        .bind(user_id)
        .bind(round.auction_id)
        .bind(round.round_num - 1)
        .fetch_all(&mut **tx)
        .await?;

        // Calculate total eligibility points including the new space
        let mut total_points = space.eligibility_points;
        total_points +=
            calculate_total_eligibility_points(&active_spaces, pool).await?;

        // Check if total would exceed eligibility
        if total_points > eligibility {
            return Err(StoreError::ExceedsEligibility {
                available: eligibility,
                required: total_points,
            });
        }
    }

    // Check credit limit before creating bid
    // Get and lock the account for this user in the community
    let account = currency::get_account_for_update_tx(
        &site.community_id,
        payloads::AccountOwner::Member(*user_id),
        tx,
    )
    .await?;

    // Get bid increment from auction params
    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(round.auction_id)
            .fetch_one(&mut **tx)
            .await?;

    let auction_params = sqlx::query_as::<_, AuctionParams>(
        "SELECT * FROM auction_params WHERE id = $1",
    )
    .bind(auction.auction_params_id)
    .fetch_one(&mut **tx)
    .await?;

    // Calculate the amount this bid will lock
    // Get previous round's value for this space (if any)
    let prev_value: Option<Decimal> = if round.round_num > 0 {
        let prev_round_id: Option<payloads::AuctionRoundId> =
            sqlx::query_scalar(
                "SELECT id FROM auction_rounds
                WHERE auction_id = $1 AND round_num = $2",
            )
            .bind(round.auction_id)
            .bind(round.round_num - 1)
            .fetch_optional(&mut **tx)
            .await?;

        if let Some(prev_id) = prev_round_id {
            sqlx::query_scalar(
                "SELECT value FROM round_space_results
                WHERE round_id = $1 AND space_id = $2",
            )
            .bind(prev_id)
            .bind(space_id)
            .fetch_optional(&mut **tx)
            .await?
        } else {
            None
        }
    } else {
        None
    };

    // Bid amount = (prev value + bid increment) OR zero
    let bid_amount = prev_value
        .map(|v| v + auction_params.bid_increment)
        .unwrap_or(Decimal::ZERO);

    // Check if user has sufficient credit for this bid
    currency::check_sufficient_credit_tx(&account.id, bid_amount, tx).await?;

    // Create the bid
    sqlx::query(
        "INSERT INTO bids (space_id, round_id, user_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $4)",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .bind(time_source.now().to_sqlx())
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn get_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Bid, StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    let bid = sqlx::query_as::<_, Bid>(
        "SELECT * FROM bids WHERE space_id = $1 AND round_id = $2 AND user_id = $3",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::BidNotFound,
        e => StoreError::Database(e),
    })?;

    Ok(bid)
}

pub async fn list_bids(
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
) -> Result<Vec<Bid>, StoreError> {
    // Verify user has access to the auction round
    let auction_round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => e.into(),
    })?;

    let auction =
        sqlx::query_as::<_, Auction>("SELECT * FROM auctions WHERE id = $1")
            .bind(auction_round.auction_id)
            .fetch_one(pool)
            .await?;

    let community_id = get_site_community_id(&auction.site_id, pool).await?;
    let _ = get_validated_member(user_id, &community_id, pool).await?;

    let bids = sqlx::query_as::<_, Bid>(
        "SELECT * FROM bids WHERE round_id = $1 AND user_id = $2",
    )
    .bind(round_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(bids)
}

pub async fn delete_bid(
    space_id: &SpaceId,
    round_id: &AuctionRoundId,
    user_id: &UserId,
    pool: &PgPool,
    time_source: &TimeSource,
) -> Result<(), StoreError> {
    // Get the space to validate user permissions
    let (_, _) =
        get_validated_space(space_id, user_id, PermissionLevel::Member, pool)
            .await?;

    // Verify the round exists and is ongoing
    let round = sqlx::query_as::<_, AuctionRound>(
        "SELECT * FROM auction_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StoreError::AuctionRoundNotFound,
        e => StoreError::Database(e),
    })?;

    let now = time_source.now();
    if now < round.start_at {
        return Err(StoreError::RoundNotStarted);
    }
    if now >= round.end_at {
        return Err(StoreError::RoundEnded);
    }

    // Delete the bid
    sqlx::query(
        "DELETE FROM bids WHERE space_id = $1 AND round_id = $2 AND user_id = $3",
    )
    .bind(space_id)
    .bind(round_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}
