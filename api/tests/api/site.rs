use test_helpers::spawn_app;

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
            .contains("Site not found")
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
            .contains("Space not found")
    );

    Ok(())
}

#[tokio::test]
async fn create_read_update_delete_site_image() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Create and verify a test site image
    let site_image = app.create_test_site_image(&community_id).await?;
    assert_eq!(site_image.name, "test image");
    assert_eq!(site_image.community_id, community_id);
    assert_eq!(
        site_image.image_data,
        vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );

    // Update the site image
    app.update_site_image_details(site_image.clone()).await?;

    // List site images for the community
    let site_images = app.client.list_site_images(&community_id).await?;
    assert_eq!(site_images.len(), 1);
    assert_eq!(site_images[0].name, "test image updated");

    // Delete the site image
    app.client.delete_site_image(&site_image.id).await?;
    assert!(
        app.client
            .get_site_image(&site_image.id)
            .await
            .unwrap_err()
            .to_string()
            .contains("Site image not found")
    );

    Ok(())
}

#[tokio::test]
async fn list_site_images_multiple() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Create multiple site images
    let _image1 = app.create_test_site_image(&community_id).await?;

    let body2 = test_helpers::site_image_details_b(community_id);
    let image2_id = app.client.create_site_image(&body2).await?;
    let _image2 = app.client.get_site_image(&image2_id).await?;

    // List all site images
    let site_images = app.client.list_site_images(&community_id).await?;
    assert_eq!(site_images.len(), 2);

    // Images should be sorted by name
    assert_eq!(site_images[0].name, "test image");
    assert_eq!(site_images[1].name, "test image b");

    Ok(())
}

#[tokio::test]
async fn site_image_permissions_require_coleader() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Create Bob and make him only a member (not coleader)
    app.create_bob_user().await?;
    let invite_id = app.invite_bob().await?;
    app.login_bob().await?;
    app.client.accept_invite(&invite_id).await?;

    // Bob should not be able to create site images (only coleaders+ can)
    let body = test_helpers::site_image_details_a(community_id);
    let result = app.client.create_site_image(&body).await;
    test_helpers::assert_status_code(result, reqwest::StatusCode::BAD_REQUEST);

    // But Bob should be able to view site images (any member can)
    app.login_alice().await?;
    let site_image = app.create_test_site_image(&community_id).await?;

    app.login_bob().await?;
    let retrieved = app.client.get_site_image(&site_image.id).await?;
    assert_eq!(retrieved.id, site_image.id);

    Ok(())
}

#[tokio::test]
async fn site_image_unique_names_per_community() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Create first site image
    let _image1 = app.create_test_site_image(&community_id).await?;

    // Try to create another image with the same name - should fail
    let duplicate_body = test_helpers::site_image_details_a(community_id);
    let result = app.client.create_site_image(&duplicate_body).await;
    test_helpers::assert_status_code(result, reqwest::StatusCode::BAD_REQUEST);

    Ok(())
}
