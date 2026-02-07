use api::store;
use payloads::{IdempotencyKey, requests};
use reqwest::StatusCode;
use rust_decimal::Decimal;
use uuid::Uuid;

use test_helpers::{assert_status_code, spawn_app};

#[tokio::test]
async fn login_refused() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // test a login with an invalid user
    let body = requests::LoginCredentials {
        username: "random".into(),
        password: "random".into(),
    };
    let result = app.client.login(&body).await;

    match result {
        Err(payloads::ClientError::APIError(code, text)) => {
            assert_eq!(code, StatusCode::UNAUTHORIZED);
            assert_eq!(text, "Authentication failed: Invalid credentials");
        }
        _ => {
            panic!("Expected APIError");
        }
    }

    // login check should fail
    let is_logged_in = app.client.login_check().await?;
    assert!(!is_logged_in);

    Ok(())
}

#[tokio::test]
async fn create_account() -> anyhow::Result<()> {
    let app = spawn_app().await;

    app.create_alice_user().await?;

    // check for valid session
    let is_logged_in = app.client.login_check().await?;
    assert!(is_logged_in);

    Ok(())
}

#[tokio::test]
async fn long_username_email_rejected() -> anyhow::Result<()> {
    let app = spawn_app().await;

    let mut body = requests::CreateAccount {
        username: (0..52).map(|_| "X").collect::<String>(),
        email: "anemail@example.com".into(),
        password: "a-password".into(),
    };
    let result = app.client.create_account(&body).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    body.username = "username".into();
    body.email =
        format!("{}@example.clom", (0..300).map(|_| "X").collect::<String>());
    let result = app.client.create_account(&body).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn update_profile_success() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Create a user and login
    app.create_alice_user().await?;

    // Check initial profile
    let profile = app.client.user_profile().await?;
    println!("Initial profile: {:?}", profile);
    assert_eq!(profile.username, "alice");
    assert_eq!(profile.display_name, None);

    // Update display name
    let update_request = requests::UpdateProfile {
        display_name: Some("Alice Smith".to_string()),
    };
    println!("Sending update request: {:?}", update_request);
    let updated_profile = app.client.update_profile(&update_request).await?;
    println!("Updated profile: {:?}", updated_profile);
    assert_eq!(
        updated_profile.display_name,
        Some("Alice Smith".to_string())
    );
    assert_eq!(updated_profile.username, "alice"); // Username should remain unchanged

    // Verify the change persists
    let profile = app.client.user_profile().await?;
    println!("Final profile: {:?}", profile);
    assert_eq!(profile.display_name, Some("Alice Smith".to_string()));

    Ok(())
}

#[tokio::test]
async fn update_profile_clear_display_name() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Create a user and login
    app.create_alice_user().await?;

    // Set a display name first
    let update_request = requests::UpdateProfile {
        display_name: Some("Alice Smith".to_string()),
    };
    app.client.update_profile(&update_request).await?;

    // Clear the display name
    let update_request = requests::UpdateProfile { display_name: None };
    let updated_profile = app.client.update_profile(&update_request).await?;
    assert_eq!(updated_profile.display_name, None);

    Ok(())
}

#[tokio::test]
async fn update_profile_display_name_too_long() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Create a user and login
    app.create_alice_user().await?;

    // Try to set a display name that's too long
    let long_display_name = (0..256).map(|_| "X").collect::<String>();
    let update_request = requests::UpdateProfile {
        display_name: Some(long_display_name),
    };

    let result = app.client.update_profile(&update_request).await;
    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn update_profile_requires_authentication() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Try to update profile without being logged in
    let update_request = requests::UpdateProfile {
        display_name: Some("Alice Smith".to_string()),
    };

    let result = app.client.update_profile(&update_request).await;
    assert_status_code(result, StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn delete_user_no_auction_history() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Create and login as alice
    app.create_alice_user().await?;

    // Get the user ID before deletion
    let user = sqlx::query_as::<_, store::User>(
        "SELECT * FROM users WHERE username = 'alice'",
    )
    .fetch_one(&app.db_pool)
    .await?;

    // Delete via API (should be a hard delete since no auction history)
    app.client.delete_user().await?;

    // Verify user is logged out
    let is_logged_in = app.client.login_check().await?;
    assert!(!is_logged_in);

    // Verify user is completely gone from the database
    let user_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)",
    )
    .bind(user.id)
    .fetch_one(&app.db_pool)
    .await?;
    assert!(!user_exists, "User should be fully deleted");

    // Verify login fails
    let login_result = app
        .client
        .login(&test_helpers::alice_login_credentials())
        .await;
    assert!(login_result.is_err());

    Ok(())
}

#[tokio::test]
async fn delete_user_with_auction_history() -> anyhow::Result<()> {
    use api::scheduler;
    use jiff::Span;

    let app = spawn_app().await;

    // Create a community with Alice (leader) and Bob (member)
    let community_id = app.create_two_person_community().await?;
    let site = app.create_test_site(&community_id).await?;
    let space = app.create_test_space(&site.site_id).await?;

    // Create an auction that starts now
    let auction = app.create_test_auction(&site.site_id).await?;

    // Run scheduler to create the first round
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

    // Get the current round
    let rounds = app.client.list_auction_rounds(&auction.auction_id).await?;
    assert!(!rounds.is_empty());
    let round = &rounds[0];

    // Bob places a bid (not Alice, since Alice is leader and can't be deleted)
    app.login_bob().await?;
    app.client
        .create_bid(&space.space_id, &round.round_id)
        .await?;

    // Complete the round so Bob wins
    app.time_source.advance(Span::new().minutes(2));
    scheduler::schedule_tick(&app.db_pool, &app.time_source).await?;

    // Verify Bob won
    let results = app
        .client
        .list_round_space_results_for_round(&round.round_id)
        .await?;
    assert!(!results.is_empty());
    assert_eq!(results[0].winner.username, "bob");

    // Get Bob's user ID before deletion
    let bob_id = sqlx::query_scalar::<_, payloads::UserId>(
        "SELECT id FROM users WHERE username = 'bob'",
    )
    .fetch_one(&app.db_pool)
    .await?;

    // Delete Bob via API (should anonymize since he has auction history)
    app.client.delete_user().await?;

    // Verify user still exists but is anonymized
    let anonymized_user =
        sqlx::query_as::<_, store::User>("SELECT * FROM users WHERE id = $1")
            .bind(bob_id)
            .fetch_one(&app.db_pool)
            .await?;

    assert!(
        anonymized_user.deleted_at.is_some(),
        "deleted_at should be set"
    );
    assert!(
        anonymized_user.username.starts_with("deleted-"),
        "Username should be anonymized"
    );
    assert!(
        anonymized_user.email.ends_with("@deleted.local"),
        "Email should be anonymized"
    );
    assert!(
        anonymized_user.password_hash.is_empty(),
        "Password hash should be cleared"
    );
    assert!(
        anonymized_user.display_name.is_none(),
        "Display name should be cleared"
    );
    assert!(
        !anonymized_user.email_verified,
        "email_verified should be false"
    );

    // Verify login fails for anonymized user
    let login_result = app
        .client
        .login(&test_helpers::bob_login_credentials())
        .await;
    assert!(login_result.is_err());

    // Verify auction history still shows the anonymized username
    // Note: we login directly since delete_user already logged out Bob
    app.client
        .login(&test_helpers::alice_login_credentials())
        .await?;
    let results = app
        .client
        .list_round_space_results_for_round(&round.round_id)
        .await?;
    assert!(!results.is_empty());
    assert!(
        results[0].winner.username.starts_with("deleted-"),
        "Auction history should show anonymized username"
    );

    Ok(())
}

#[tokio::test]
async fn delete_user_leader_blocked() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Create Alice who becomes leader of a community
    app.create_alice_user().await?;
    app.create_test_community().await?;

    // Attempt to delete Alice via API (should fail since she's a leader)
    let result = app.client.delete_user().await;
    assert!(result.is_err(), "Expected error when deleting leader");

    // Verify Alice still exists and is not anonymized
    let alice_after = sqlx::query_as::<_, store::User>(
        "SELECT * FROM users WHERE username = 'alice'",
    )
    .fetch_one(&app.db_pool)
    .await?;
    assert!(alice_after.deleted_at.is_none());

    // Verify Alice is still logged in
    let is_logged_in = app.client.login_check().await?;
    assert!(is_logged_in);

    Ok(())
}

/// User with transaction history (but no auction history) should be anonymized.
/// The FK constraint chain is: users -> accounts (CASCADE) -> entry_lines (RESTRICT)
/// So if the user's account has any entry_lines, the cascade is blocked.
#[tokio::test]
async fn delete_user_with_transaction_history() -> anyhow::Result<()> {
    let app = spawn_app().await;

    // Create a community with Alice (leader) and Bob (member)
    let community_id = app.create_two_person_community().await?;

    // Get Bob's user_id
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let bob_id = bob.user.user_id;

    // Alice transfers funds to Bob (creates entry_lines for Bob's account)
    app.login_alice().await?;
    app.client
        .create_transfer(&requests::CreateTransfer {
            community_id,
            to_user_id: bob_id,
            amount: Decimal::new(5000, 2), // 50.00
            note: Some("Test transfer".into()),
            idempotency_key: IdempotencyKey(Uuid::new_v4()),
        })
        .await?;

    // Bob leaves the community (account becomes orphaned)
    app.login_bob().await?;
    app.client
        .leave_community(&requests::LeaveCommunity { community_id })
        .await?;

    // Bob deletes his account (should anonymize due to transaction history)
    app.client.delete_user().await?;

    // Verify user still exists but is anonymized
    let anonymized_user =
        sqlx::query_as::<_, store::User>("SELECT * FROM users WHERE id = $1")
            .bind(bob_id)
            .fetch_one(&app.db_pool)
            .await?;

    assert!(
        anonymized_user.deleted_at.is_some(),
        "deleted_at should be set"
    );
    assert!(
        anonymized_user.username.starts_with("deleted-"),
        "Username should be anonymized"
    );
    assert!(
        anonymized_user.email.ends_with("@deleted.local"),
        "Email should be anonymized"
    );

    // Verify account still exists (orphaned) with preserved balance
    let balance = sqlx::query_scalar::<_, Decimal>(
        "SELECT balance_cached FROM accounts WHERE owner_id = $1",
    )
    .bind(bob_id)
    .fetch_one(&app.db_pool)
    .await?;

    assert_eq!(
        balance,
        Decimal::new(5000, 2),
        "Account balance should be preserved"
    );

    Ok(())
}
