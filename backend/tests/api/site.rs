use crate::helpers::spawn_app;

#[tokio::test]
async fn create_read_update_delete_site() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let response = app.create_test_site(&community_id).await?;

    let site_id = response.site_id;
    app.update_site_details(response).await?;
    app.client.delete_site(&site_id).await?;
    assert!(
        app.client
            .get_site(&site_id)
            .await
            .unwrap_err()
            .to_string()
            .contains("Row not found")
    );

    Ok(())
}

#[tokio::test]
async fn create_read_update_delete_space() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create and verify a test space
    let space = app.create_test_space(&site.site_id).await?;

    // Update the space
    app.update_space_details(space.clone()).await?;

    // List spaces
    let spaces = app.client.list_spaces(&site.site_id).await?;
    assert_eq!(spaces.len(), 1);
    assert_eq!(spaces[0].space_details.name, "test space a updated");

    // Delete the space
    app.client.delete_space(&space.space_id).await?;
    assert!(
        app.client
            .get_space(&space.space_id)
            .await
            .unwrap_err()
            .to_string()
            .contains("Row not found")
    );

    Ok(())
}
