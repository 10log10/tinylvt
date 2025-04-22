use actix_web::{
    HttpRequest, HttpResponse, Responder, http::header::LOCATION, post, web,
};
use sqlx::PgPool;

use crate::password::{NewUserDetails, create_user};

// TODO: return error for better instrumentation?
#[tracing::instrument(
    skip(new_user_details, pool),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
#[post("/create_account")]
pub async fn create_account(
    request: HttpRequest,
    new_user_details: web::Form<NewUserDetails>,
    pool: web::Data<PgPool>,
) -> impl Responder {
    match create_user(new_user_details.0, &pool).await {
        Ok(()) => HttpResponse::SeeOther()
            .insert_header((LOCATION, "/login"))
            .finish(),
        Err(_) => {
            HttpResponse::InternalServerError().body("Something went wrong")
        }
    }
}
