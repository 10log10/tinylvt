use test_helpers::spawn_app;

#[tokio::test]
async fn create_read_update_delete_site() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let response = app.create_test_site(&community_id).await?;

    let site_id = response.site_id;
    app.update_site_details(response).await?;

    // Test soft delete - site should still be accessible
    app.client.soft_delete_site(&site_id).await?;
    let soft_deleted_site = app.client.get_site(&site_id).await?;
    assert!(soft_deleted_site.deleted_at.is_some());

    // Test hard delete - site should no longer be accessible
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
    let expected = test_helpers::site_image_details_a(community_id);
    assert_eq!(site_image.name, expected.name);
    assert_eq!(site_image.community_id, community_id);
    assert_eq!(site_image.image_data, expected.image_data,);

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
    assert_eq!(site_images[0].name, "Blue Square");
    assert_eq!(site_images[1].name, "Red Square");

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

#[tokio::test]
async fn list_sites() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Initially, there should be no sites
    let sites = app.client.list_sites(&community_id).await?;
    assert_eq!(sites.len(), 0);

    // Create a test site
    let site1 = app.create_test_site(&community_id).await?;

    // Create a second site with different details
    let site2_details = payloads::Site {
        community_id,
        name: "Second Test Site".to_string(),
        description: Some("A second test site".to_string()),
        default_auction_params: payloads::AuctionParams {
            round_duration: jiff::Span::new().hours(2), // Different duration
            bid_increment: rust_decimal::Decimal::new(200, 2), // $2.00
            activity_rule_params: payloads::ActivityRuleParams {
                eligibility_progression: vec![(1, 0.8)], // 80% eligibility required
            },
        },
        possession_period: jiff::Span::new().days(14), // 14 days
        auction_lead_time: jiff::Span::new().days(3),  // 3 days
        proxy_bidding_lead_time: jiff::Span::new().hours(12), // 12 hours
        open_hours: None,
        auto_schedule: false,
        timezone: Some("America/New_York".to_string()),
        site_image_id: None,
    };
    let site2_id = app.client.create_site(&site2_details).await?;
    let site2 = app.client.get_site(&site2_id).await?;

    // List all sites
    let sites = app.client.list_sites(&community_id).await?;
    assert_eq!(sites.len(), 2);

    // Sites should be sorted by name
    assert_eq!(sites[0].site_details.name, "Second Test Site");
    assert_eq!(sites[1].site_details.name, "test site");

    // Verify the site details are correct
    assert_eq!(sites[0].site_id, site2.site_id);
    assert_eq!(sites[1].site_id, site1.site_id);

    Ok(())
}

#[tokio::test]
async fn list_sites_permissions() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;

    // Create a test site
    let _site = app.create_test_site(&community_id).await?;

    // Create Bob and make him a member
    app.create_bob_user().await?;
    let invite_id = app.invite_bob().await?;
    app.login_bob().await?;
    app.client.accept_invite(&invite_id).await?;

    // Bob should be able to list sites (any member can)
    let sites = app.client.list_sites(&community_id).await?;
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].site_details.name, "test site");

    Ok(())
}

#[tokio::test]
async fn space_name_must_be_unique() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create first space
    let space1_details = payloads::Space {
        site_id: site.site_id,
        name: "Duplicate Name".to_string(),
        description: Some("First space".to_string()),
        eligibility_points: 1.0,
        is_available: true,
        site_image_id: None,
    };
    app.client.create_space(&space1_details).await?;

    // Try to create second space with same name - should fail
    let space2_details = payloads::Space {
        site_id: site.site_id,
        name: "Duplicate Name".to_string(),
        description: Some("Second space".to_string()),
        eligibility_points: 1.0,
        is_available: true,
        site_image_id: None,
    };
    let result = app.client.create_space(&space2_details).await;

    // Should get a specific error about the duplicate name
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Duplicate Name"),
        "Error message should mention the space name: {}",
        err
    );
    assert!(
        err.contains("already exists"),
        "Error message should say 'already exists': {}",
        err
    );

    Ok(())
}

#[tokio::test]
async fn space_restore_detects_name_conflict() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create first space and soft-delete it
    let space1_details = payloads::Space {
        site_id: site.site_id,
        name: "Conflicting Name".to_string(),
        description: Some("First space".to_string()),
        eligibility_points: 1.0,
        is_available: true,
        site_image_id: None,
    };
    let space1 = app.client.create_space(&space1_details).await?;
    app.client.soft_delete_space(&space1).await?;

    // Create second space with same name (allowed since first is deleted)
    let space2_details = payloads::Space {
        site_id: site.site_id,
        name: "Conflicting Name".to_string(),
        description: Some("Second space".to_string()),
        eligibility_points: 1.0,
        is_available: true,
        site_image_id: None,
    };
    app.client.create_space(&space2_details).await?;

    // Try to restore first space - should fail due to name conflict
    let result = app.client.restore_space(&space1).await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Conflicting Name"),
        "Error message should mention the space name: {}",
        err
    );
    assert!(
        err.contains("already exists"),
        "Error message should say 'already exists': {}",
        err
    );

    Ok(())
}

#[tokio::test]
async fn space_copy_on_write_with_auction_history() -> anyhow::Result<()> {
    let app = spawn_app().await;
    app.create_alice_user().await?;
    let community_id = app.create_test_community().await?;
    let site = app.create_test_site(&community_id).await?;

    // Create a space
    let space_details = payloads::Space {
        site_id: site.site_id,
        name: "Original Name".to_string(),
        description: Some("Original description".to_string()),
        eligibility_points: 1.0,
        is_available: true,
        site_image_id: None,
    };
    let space = app.client.create_space(&space_details).await?;

    // Update trivial fields (description) - should update in place
    let trivial_update = payloads::requests::UpdateSpace {
        space_id: space,
        space_details: payloads::Space {
            site_id: site.site_id,
            name: "Original Name".to_string(),
            description: Some("Updated description".to_string()),
            eligibility_points: 1.0,
            is_available: true,
            site_image_id: None,
        },
    };
    let trivial_result = app.client.update_space(&trivial_update).await?;
    assert!(!trivial_result.was_copied);
    assert!(trivial_result.old_space_id.is_none());
    assert_eq!(trivial_result.space.space_id, space);

    // Create an auction to give the space auction history
    let auction = app.create_test_auction(&site.site_id).await?;

    // Start the auction and create a bid to establish auction history
    api::scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;
    let rounds = app.client.list_auction_rounds(&auction.auction_id).await?;
    assert!(!rounds.is_empty());

    app.create_bob_user().await?;
    let invite_id = app.invite_bob().await?;
    app.login_bob().await?;
    app.client.accept_invite(&invite_id).await?;

    app.client.create_bid(&space, &rounds[0].round_id).await?;

    // Now update nontrivial field (name) - should trigger copy-on-write
    app.login_alice().await?;
    let nontrivial_update = payloads::requests::UpdateSpace {
        space_id: space,
        space_details: payloads::Space {
            site_id: site.site_id,
            name: "Updated Name".to_string(),
            description: Some("Updated description".to_string()),
            eligibility_points: 1.0,
            is_available: true,
            site_image_id: None,
        },
    };
    let nontrivial_result = app.client.update_space(&nontrivial_update).await?;

    // Should have created a new space (copy-on-write)
    assert!(nontrivial_result.was_copied);
    assert_eq!(nontrivial_result.old_space_id, Some(space));
    assert_ne!(nontrivial_result.space.space_id, space);
    assert_eq!(nontrivial_result.space.space_details.name, "Updated Name");

    // Old space should be soft-deleted
    let old_space = app.client.get_space(&space).await?;
    assert!(old_space.deleted_at.is_some());

    // New space should exist and not be deleted
    let new_space = app
        .client
        .get_space(&nontrivial_result.space.space_id)
        .await?;
    assert!(new_space.deleted_at.is_none());

    // List spaces returns both spaces (list doesn't filter deleted)
    let spaces = app.client.list_spaces(&site.site_id).await?;
    assert_eq!(spaces.len(), 2);

    // Find the new space (not deleted)
    let new_in_list = spaces
        .iter()
        .find(|s| s.deleted_at.is_none())
        .expect("Should have one non-deleted space");
    assert_eq!(new_in_list.space_id, nontrivial_result.space.space_id);
    assert_eq!(new_in_list.space_details.name, "Updated Name");

    // Find the old space (soft-deleted)
    let old_in_list = spaces
        .iter()
        .find(|s| s.deleted_at.is_some())
        .expect("Should have one deleted space");
    assert_eq!(old_in_list.space_id, space);

    Ok(())
}
