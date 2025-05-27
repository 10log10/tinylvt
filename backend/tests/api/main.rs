mod auction;
mod community;
mod database;
mod helpers;
mod login;
mod site;

use helpers::spawn_app;

#[tokio::test]
async fn health_check() -> anyhow::Result<()> {
    let app = spawn_app().await;

    app.client.health_check().await?;

    Ok(())
}
