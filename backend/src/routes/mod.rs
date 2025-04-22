pub mod create_account;
pub mod login;

use actix_web::{HttpResponse, Responder, dev::HttpServiceFactory, get};

/// Returns all api routes.
pub fn api_routes() -> impl HttpServiceFactory {
    (
        health_check,
        login::login_routes(),
        create_account::create_account,
    )
}

#[get("/health_check")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("healthy")
}
