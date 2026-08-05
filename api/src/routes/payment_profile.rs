use actix_identity::Identity;
use actix_web::{HttpResponse, post, web};
use payloads::CheckoutSessionResponse;
use sqlx::PgPool;

use crate::AppConfig;
use crate::store;
use crate::stripe_service::StripeService;

use super::{RouteError, get_user_id};

#[post("/create_card_setup_session")]
pub async fn create_card_setup_session(
    user: Identity,
    pool: web::Data<PgPool>,
    stripe_service: web::Data<StripeService>,
    app_config: web::Data<AppConfig>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let url = store::payment_profile::create_card_setup_session(
        &user_id,
        &stripe_service,
        &app_config,
        &time_source,
        &pool,
    )
    .await?;
    Ok(HttpResponse::Ok().json(CheckoutSessionResponse { checkout_url: url }))
}

#[post("/get_payment_profile")]
pub async fn get_payment_profile(
    user: Identity,
    pool: web::Data<PgPool>,
    stripe_service: web::Data<StripeService>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let profile = store::payment_profile::get_payment_profile(
        &user_id,
        &stripe_service,
        &time_source,
        &pool,
    )
    .await?;
    Ok(HttpResponse::Ok().json(profile))
}

#[post("/remove_payment_method")]
pub async fn remove_payment_method(
    user: Identity,
    pool: web::Data<PgPool>,
    stripe_service: web::Data<StripeService>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::payment_profile::remove_payment_method(
        &user_id,
        &stripe_service,
        &time_source,
        &pool,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/update_card_charge_grant")]
pub async fn update_card_charge_grant(
    user: Identity,
    request: web::Json<payloads::requests::UpdateCardChargeGrant>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let member =
        super::get_validated_member(&user_id, &request.community_id, &pool)
            .await?;
    store::payment_profile::update_card_charge_grant(
        &member,
        request.grant,
        &time_source,
        &pool,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/get_card_charge_grant")]
pub async fn get_card_charge_grant(
    user: Identity,
    community_id: web::Json<payloads::CommunityId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    let member =
        super::get_validated_member(&user_id, &community_id, &pool).await?;
    Ok(HttpResponse::Ok()
        .json(store::payment_profile::card_charge_granted(&member)))
}

#[post("/update_hold_strategy")]
pub async fn update_hold_strategy(
    user: Identity,
    request: web::Json<payloads::requests::UpdateHoldStrategy>,
    pool: web::Data<PgPool>,
    time_source: web::Data<crate::time::TimeSource>,
) -> Result<HttpResponse, RouteError> {
    let user_id = get_user_id(&user)?;
    store::payment_profile::update_hold_strategy(
        &user_id,
        request.budget_holds,
        &time_source,
        &pool,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}
