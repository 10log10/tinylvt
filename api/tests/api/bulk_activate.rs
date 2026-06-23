use payloads::requests;
use reqwest::StatusCode;
use test_helpers::{assert_status_code, spawn_app};

/// A moderator+ can activate members in bulk by email or username, and the
/// result reports matched/unmatched identifiers. Activation is additive:
/// members not named in the list keep their status.
#[tokio::test]
async fn bulk_activate_by_email_and_username() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_three_person_community().await?;

    // Members default to active. Deactivate bob and charlie so the bulk
    // activation has an observable effect, leaving alice (leader) active.
    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let charlie = members
        .iter()
        .find(|m| m.user.username == "charlie")
        .unwrap();
    let bob_id = bob.user.user_id;
    let charlie_id = charlie.user.user_id;

    app.login_alice().await?;
    for member_user_id in [bob_id, charlie_id] {
        app.client
            .update_member_active_status(&requests::UpdateMemberActiveStatus {
                community_id,
                member_user_id,
                is_active: false,
            })
            .await?;
    }

    // Activate bob by email and charlie by username, plus a bogus entry that
    // matches no member. Both lookups use mixed case to exercise the
    // case-insensitive match. Alice is not in the list and stays untouched.
    let result = app
        .client
        .bulk_activate_members(&requests::BulkActivateMembers {
            community_id,
            identifiers: vec![
                "BOB@example.com".into(),
                "Charlie".into(),
                "nobody".into(),
            ],
        })
        .await?;

    assert_eq!(result.activated_count, 2);
    assert_eq!(result.unmatched, vec!["nobody".to_string()]);

    // Bob and charlie are now active again; alice was never changed.
    let members = app.client.get_members(&community_id).await?;
    for member in &members {
        match member.user.username.as_str() {
            "alice" => assert!(member.is_active),
            "bob" => assert!(member.is_active),
            "charlie" => assert!(member.is_active),
            _ => (),
        }
    }

    Ok(())
}

/// Email matching is case-insensitive, so an identifier whose case differs
/// from the stored email still activates the member.
#[tokio::test]
async fn bulk_activate_email_case_insensitive() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let bob_id = bob.user.user_id;

    app.login_alice().await?;
    app.client
        .update_member_active_status(&requests::UpdateMemberActiveStatus {
            community_id,
            member_user_id: bob_id,
            is_active: false,
        })
        .await?;

    let result = app
        .client
        .bulk_activate_members(&requests::BulkActivateMembers {
            community_id,
            identifiers: vec!["BOB@Example.COM".into()],
        })
        .await?;

    assert_eq!(result.activated_count, 1);
    assert!(result.unmatched.is_empty());

    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    assert!(bob.is_active);

    Ok(())
}

/// A member named more than once counts once in `activated_count`, but every
/// unmatched input is echoed back verbatim and in input order, so a user who
/// typed two spellings of a missing name sees both.
#[tokio::test]
async fn bulk_activate_counts_distinct_but_echoes_all_unmatched()
-> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let bob_id = bob.user.user_id;

    app.login_alice().await?;
    app.client
        .update_member_active_status(&requests::UpdateMemberActiveStatus {
            community_id,
            member_user_id: bob_id,
            is_active: false,
        })
        .await?;

    let result = app
        .client
        .bulk_activate_members(&requests::BulkActivateMembers {
            community_id,
            identifiers: vec![
                "bob".into(),
                "BOB".into(),
                "ghost".into(),
                "Ghost".into(),
            ],
        })
        .await?;

    // Bob counted once despite two spellings; both unmatched spellings of
    // "ghost" are echoed back in input order.
    assert_eq!(result.activated_count, 1);
    assert_eq!(
        result.unmatched,
        vec!["ghost".to_string(), "Ghost".to_string()]
    );

    Ok(())
}

/// Naming one member by both their email and username counts that member once
/// in `activated_count` (it is `count(distinct member)`, not a per-input
/// tally).
#[tokio::test]
async fn bulk_activate_same_member_two_ways_counts_once() -> anyhow::Result<()>
{
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    let members = app.client.get_members(&community_id).await?;
    let bob = members.iter().find(|m| m.user.username == "bob").unwrap();
    let bob_id = bob.user.user_id;

    app.login_alice().await?;
    app.client
        .update_member_active_status(&requests::UpdateMemberActiveStatus {
            community_id,
            member_user_id: bob_id,
            is_active: false,
        })
        .await?;

    // Bob identified by both his username and his email.
    let result = app
        .client
        .bulk_activate_members(&requests::BulkActivateMembers {
            community_id,
            identifiers: vec!["bob".into(), "bob@example.com".into()],
        })
        .await?;

    assert_eq!(result.activated_count, 1);
    assert!(result.unmatched.is_empty());

    Ok(())
}

/// A plain member (below moderator) cannot bulk-activate.
#[tokio::test]
async fn bulk_activate_requires_moderator() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    // Bob is a plain member.
    app.login_bob().await?;
    let result = app
        .client
        .bulk_activate_members(&requests::BulkActivateMembers {
            community_id,
            identifiers: vec!["alice@example.com".into()],
        })
        .await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

/// A request exceeding `MAX_BULK_ACTIVATE_IDENTIFIERS` is rejected outright,
/// bounding the work a single statement performs.
#[tokio::test]
async fn bulk_activate_rejects_oversized_list() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    app.login_alice().await?;
    let identifiers = (0..requests::MAX_BULK_ACTIVATE_IDENTIFIERS + 1)
        .map(|i| format!("ghost{i}"))
        .collect();
    let result = app
        .client
        .bulk_activate_members(&requests::BulkActivateMembers {
            community_id,
            identifiers,
        })
        .await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}

/// A single identifier longer than `MAX_BULK_ACTIVATE_IDENTIFIER_LEN` is
/// rejected, guarding against a pathological single paste. Such an identifier
/// could never match a member anyway, since the email column is `VARCHAR(255)`.
#[tokio::test]
async fn bulk_activate_rejects_overlong_identifier() -> anyhow::Result<()> {
    let app = spawn_app().await;
    let community_id = app.create_two_person_community().await?;

    app.login_alice().await?;
    let too_long = "x".repeat(requests::MAX_BULK_ACTIVATE_IDENTIFIER_LEN + 1);
    let result = app
        .client
        .bulk_activate_members(&requests::BulkActivateMembers {
            community_id,
            identifiers: vec![too_long],
        })
        .await;

    assert_status_code(result, StatusCode::BAD_REQUEST);

    Ok(())
}
