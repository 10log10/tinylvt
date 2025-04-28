mod community;
mod database;
mod helpers;
mod login;

use helpers::spawn_app;
use reqwest::StatusCode;

#[tokio::test]
async fn health_check() -> anyhow::Result<()> {
    let app = spawn_app().await;

    let response = app
        .api_client
        .get(format!("{}/api/health_check", app.address))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}
