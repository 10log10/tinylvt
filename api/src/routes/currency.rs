use actix_identity::Identity;
use actix_web::{HttpResponse, post, web};
use payloads::requests;
use sqlx::PgPool;

use crate::store;

use super::{APIError, get_user_id, get_validated_member};

// Phase 3: Credit Limit Management

#[tracing::instrument(skip(user, pool), ret)]
#[post("/update_credit_limit")]
pub async fn update_credit_limit(
    user: Identity,
    details: web::Json<requests::UpdateCreditLimit>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    let account = store::currency::update_credit_limit(
        &validated_member,
        &details.member_user_id,
        details.credit_limit,
        &pool,
    )
    .await?;

    Ok(HttpResponse::Ok().json(account))
}

// Phase 4: Balance & Transaction Queries

#[tracing::instrument(skip(user, pool), ret)]
#[post("/get_member_currency_info")]
pub async fn get_member_currency_info(
    user: Identity,
    details: web::Json<requests::GetMemberCurrencyInfo>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    let info = store::currency::get_member_currency_info_with_permissions(
        &validated_member,
        details.member_user_id.as_ref(),
        &pool,
    )
    .await?;

    Ok(HttpResponse::Ok().json(info))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/get_member_transactions")]
pub async fn get_member_transactions(
    user: Identity,
    details: web::Json<requests::GetMemberTransactions>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    let transactions =
        store::currency::get_member_transactions_with_permissions(
            &validated_member,
            details.member_user_id.as_ref(),
            details.limit,
            details.offset,
            &pool,
        )
        .await?;

    Ok(HttpResponse::Ok().json(transactions))
}

// Phase 5: Member-to-Member Transfers

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/create_transfer")]
pub async fn create_transfer(
    user: Identity,
    details: web::Json<requests::CreateTransfer>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    store::currency::create_transfer(
        &validated_member,
        &details.to_user_id,
        details.amount,
        details.note.clone(),
        details.idempotency_key,
        &time_source,
        &pool,
    )
    .await?;

    Ok(HttpResponse::Ok().finish())
}

// Phase 6: Treasury Operations

#[tracing::instrument(skip(user, pool), ret)]
#[post("/get_treasury_account")]
pub async fn get_treasury_account(
    user: Identity,
    details: web::Json<requests::GetTreasuryAccount>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    let account =
        store::currency::get_treasury_account(&validated_member, &pool).await?;

    Ok(HttpResponse::Ok().json(account))
}

#[tracing::instrument(skip(user, pool), ret)]
#[post("/get_treasury_transactions")]
pub async fn get_treasury_transactions(
    user: Identity,
    details: web::Json<requests::GetTreasuryTransactions>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    let transactions = store::currency::get_treasury_transactions(
        &validated_member,
        details.limit,
        details.offset,
        &pool,
    )
    .await?;

    Ok(HttpResponse::Ok().json(transactions))
}

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/treasury_credit_operation")]
pub async fn treasury_credit_operation(
    user: Identity,
    details: web::Json<requests::TreasuryCreditOperation>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    let result = store::currency::treasury_credit_operation(
        &validated_member,
        details.recipient.clone(),
        details.amount_per_recipient,
        details.note.clone(),
        details.idempotency_key,
        &time_source,
        &pool,
    )
    .await?;

    Ok(HttpResponse::Ok().json(result))
}

// Currency Configuration Management

/// Update currency configuration for a community (coleader+ only)
#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/update_currency_config")]
pub async fn update_currency_config(
    user: Identity,
    details: web::Json<requests::UpdateCurrencyConfig>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    store::currency::update_currency_config(
        &validated_member,
        &details.currency,
        &pool,
        &time_source,
    )
    .await?;

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(skip(user, pool, time_source), ret)]
#[post("/reset_all_balances")]
pub async fn reset_all_balances(
    user: Identity,
    details: web::Json<requests::ResetAllBalances>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, APIError> {
    let user_id = get_user_id(&user)?;
    let validated_member =
        get_validated_member(&user_id, &details.community_id, &pool).await?;

    let result = store::currency::reset_all_balances(
        &validated_member,
        details.note.clone(),
        &pool,
        &time_source,
    )
    .await?;

    Ok(HttpResponse::Ok().json(result))
}
