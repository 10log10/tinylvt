use anyhow::Result;
use fantoccini::Locator;
use std::time::Duration;
use test_helpers::alice_login_credentials;
use tokio::time::sleep;
use tracing::{debug, info};

use crate::framework::{TestEnvironment, login_user};

/// UI integration test for US-001: Create new account with email verification.
///
/// This test covers the user story:
///   As a user, I want to create and manage my account so I can participate in the system.
///
/// Steps:
/// - Navigate to the registration page
/// - Fill out the registration form (email, username, password, confirm password)
/// - Submit the form
/// - Verify that the 'Check your email' heading is shown, indicating the verification prompt
#[tokio::test]
async fn test_account_registration_flow() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Navigate to registration page
    info!("📝 Navigating to registration page");
    env.browser
        .goto(&format!("{}/register", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;

    // Step 2: Fill registration form
    info!("🔑 Filling registration form");
    let username = format!("testuser_{}", rand::random::<u32>());
    let email = format!("{}@example.com", username);
    let password = "TestPassword123!";

    let email_field = env.browser.find(Locator::Id("email")).await?;
    email_field.click().await?;
    email_field.clear().await?;
    email_field.send_keys(&email).await?;

    let username_field = env.browser.find(Locator::Id("username")).await?;
    username_field.click().await?;
    username_field.clear().await?;
    username_field.send_keys(&username).await?;

    let password_field = env.browser.find(Locator::Id("password")).await?;
    password_field.click().await?;
    password_field.clear().await?;
    password_field.send_keys(password).await?;

    let confirm_field =
        env.browser.find(Locator::Id("confirm-password")).await?;
    confirm_field.click().await?;
    confirm_field.clear().await?;
    confirm_field.send_keys(password).await?;
    // Blur confirm password field to trigger onchange
    env.browser
        .execute(
            "document.getElementById('confirm-password')?.blur();",
            vec![],
        )
        .await?;
    sleep(Duration::from_millis(100)).await;

    // Blur password field to trigger onchange
    env.browser
        .execute("document.getElementById('password')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;

    // Step 3: Submit the form
    info!("🚀 Submitting registration form");
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 4: Verify registration success and email verification prompt
    info!("🔍 Verifying registration success and email verification prompt");
    let current_url = env.browser.current_url().await?;
    debug!("Current URL after registration: {}", current_url);

    // Look for the heading 'Check your email' (VerifyEmailPrompt)
    let heading = env
        .browser
        .find(Locator::XPath(
            "//h3[contains(text(), 'Please verify your email')]",
        ))
        .await?;
    let heading_text = heading.text().await.unwrap_or_default();
    info!("Found heading after registration: {}", heading_text);

    info!("✅ Registration test completed successfully");
    Ok(())
}

/// UI integration test for US-002: Login with valid credentials.
///
/// This test covers the user story:
///   As a user, I want to log in with valid credentials and have my session persist.
///
/// Steps:
/// - Ensure Alice user exists and is verified
/// - Navigate to the login page
/// - Fill out the login form (username, password)
/// - Submit the form
/// - Verify successful login and redirect
/// - Reload the page and verify session persistence
#[tokio::test]
async fn test_login_with_valid_credentials() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Ensure Alice user exists and is verified
    info!("👤 Ensuring Alice user exists and is verified");
    env.api.create_alice_user().await?;

    // Step 2: Log in as Alice
    login_user(&env.browser, &env.frontend_url, &alice_login_credentials())
        .await?;

    // Step 3: Verify successful login and redirect
    info!("🔍 Verifying successful login and redirect");
    let current_url = env.browser.current_url().await?;
    debug!("Current URL after login: {}", current_url);
    assert!(
        !current_url.as_str().contains("/login"),
        "Should not remain on login page after successful login"
    );

    // Optionally, check for a user-specific element (e.g., username in nav)
    // let user_nav = env.browser.find(Locator::Css("#user-nav")).await?;
    // let nav_text = user_nav.text().await.unwrap_or_default();
    // assert!(nav_text.contains(&credentials.username));

    // Step 4: Reload the page and verify session persistence
    info!("🔄 Reloading page to verify session persistence");
    env.browser.refresh().await?;
    sleep(Duration::from_secs(1)).await;
    let url_after_reload = env.browser.current_url().await?;
    debug!("URL after reload: {}", url_after_reload);
    assert!(
        !url_after_reload.as_str().contains("/login"),
        "Session should persist after reload"
    );

    info!("✅ Login test completed successfully");
    Ok(())
}

/// UI integration test for US-003: Login failure with invalid credentials.
///
/// This test covers the user story:
///   As a user, I want to see an error when I try to log in with invalid credentials, and not be redirected.
///
/// Steps:
/// - Ensure Alice user exists and is verified
/// - Navigate to the login page
/// - Fill out the login form with a valid username and wrong password
/// - Submit the form
/// - Verify error message is displayed and no redirect occurs
/// - Fill out the login form with a non-existent username
/// - Submit the form
/// - Verify error message is displayed and no redirect occurs
#[tokio::test]
async fn test_login_failure_with_invalid_credentials() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Ensure Alice user exists and is verified
    info!("👤 Ensuring Alice user exists and is verified");
    env.api.create_alice_user().await?;
    let credentials = test_helpers::alice_credentials();

    // Step 2: Navigate to login page
    info!("🔑 Navigating to login page");
    env.browser
        .goto(&format!("{}/login", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;

    // Step 3: Attempt login with valid username and wrong password
    info!("❌ Attempting login with valid username and wrong password");
    let username_field = env.browser.find(Locator::Id("username")).await?;
    username_field.click().await?;
    username_field.clear().await?;
    username_field.send_keys(&credentials.username).await?;

    let password_field = env.browser.find(Locator::Id("password")).await?;
    password_field.click().await?;
    password_field.clear().await?;
    password_field.send_keys("wrongpassword").await?;

    // Blur password field to trigger onchange
    env.browser
        .execute("document.getElementById('password')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;

    // Submit the form
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 4: Verify error message and no redirect
    info!("🔍 Verifying error message and no redirect");
    let current_url = env.browser.current_url().await?;
    debug!("Current URL after failed login: {}", current_url);
    assert!(
        current_url.as_str().contains("/login"),
        "Should remain on login page after failed login"
    );

    // Look for error message (by class or role)
    let error_element = env
        .browser
        .find(Locator::Css(".bg-red-50, .bg-red-900, [role='alert']"))
        .await;
    assert!(
        error_element.is_ok(),
        "Error message should be displayed for invalid login"
    );

    // Step 5: Attempt login with non-existent username
    info!("❌ Attempting login with non-existent username");
    let username_field = env.browser.find(Locator::Id("username")).await?;
    username_field.click().await?;
    username_field.clear().await?;
    username_field.send_keys("notarealuser").await?;

    let password_field = env.browser.find(Locator::Id("password")).await?;
    password_field.click().await?;
    password_field.clear().await?;
    password_field.send_keys("somepassword").await?;

    // Blur password field to trigger onchange
    env.browser
        .execute("document.getElementById('password')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;

    // Submit the form
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 6: Verify error message and no redirect
    info!("🔍 Verifying error message and no redirect for non-existent user");
    let current_url = env.browser.current_url().await?;
    debug!("Current URL after failed login: {}", current_url);
    assert!(
        current_url.as_str().contains("/login"),
        "Should remain on login page after failed login"
    );

    let error_element = env
        .browser
        .find(Locator::Css(".bg-red-50, .bg-red-900, [role='alert']"))
        .await;
    assert!(
        error_element.is_ok(),
        "Error message should be displayed for invalid login"
    );

    info!("✅ Login failure test completed successfully");
    Ok(())
}

/// UI integration test for US-004: Password reset flow.
///
/// This test covers the user story:
///   As a user, I want to reset my password if I forget it, so I can regain access to my account.
///
/// Steps:
/// - Create and verify a user (Alice)
/// - Navigate to the login page and click 'Forgot your password?'
/// - Enter the user's email and submit the forgot password form
/// - Wait for the UI to show the reset email sent message
/// - Retrieve the reset token from the database
/// - Navigate to the reset password page with the token
/// - Enter a new password and submit the reset form
/// - Wait for the UI to show the password reset success message
/// - Log in with the new password and verify success
/// - Attempt login with the old password and verify failure
#[tokio::test]
async fn test_password_reset_flow() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Create and verify Alice user
    info!("👤 Creating and verifying Alice user");
    env.api.create_alice_user().await?;
    let credentials = test_helpers::alice_credentials();

    // Step 2: Navigate to login page and click 'Forgot your password?'
    info!("🔑 Navigating to login page");
    env.browser
        .goto(&format!("{}/login", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;
    let forgot_link = env
        .browser
        .find(Locator::XPath(
            "//a[contains(text(), 'Forgot your password?')]",
        ))
        .await?;
    forgot_link.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 3: Enter email and submit forgot password form
    info!("✉️ Submitting forgot password form");
    let email_field = env.browser.find(Locator::Id("email")).await?;
    email_field.click().await?;
    email_field.clear().await?;
    email_field.send_keys(&credentials.email).await?;
    // Blur email field to trigger onchange
    env.browser
        .execute("document.getElementById('email')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Debug: print the page HTML after submitting the form
    let page_html = env.browser.source().await.unwrap_or_default();
    debug!("Page HTML after forgot password submit: {}", page_html);

    // Step 4: Wait for reset email sent message
    info!("🔍 Waiting for reset email sent message");
    let sent_message = match env
        .browser
        .find(Locator::Css(".bg-green-50, .bg-green-900, [role='alert']"))
        .await
    {
        Ok(el) => el,
        Err(_) => {
            env.browser
                .find(Locator::XPath(
                    "//*[contains(text(), 'If an account with that email exists, a password reset link has been sent.')]"
                ))
                .await?
        }
    };
    let sent_text = sent_message.text().await.unwrap_or_default();
    assert!(sent_text.contains("If an account with that email exists, a password reset link has been sent."), "Reset email sent message not found, got: {}", sent_text);
    info!("Found reset email sent message: {}", sent_text);

    // Step 5: Retrieve reset token from database
    info!("🔑 Retrieving reset token from database");
    let reset_token = env
        .api
        .get_password_reset_token_from_db(&credentials.email)
        .await?;
    info!("Got reset token: {}", reset_token);

    // Step 6: Navigate to reset password page with token
    info!("🔗 Navigating to reset password page");
    env.browser
        .goto(&format!(
            "{}/reset_password?token={}",
            env.frontend_url, reset_token
        ))
        .await?;
    sleep(Duration::from_secs(1)).await;

    // Step 7: Enter new password and submit reset form
    info!("🔒 Entering new password and submitting reset form");
    let new_password = "NewSuperSecret123!";
    let password_field = env.browser.find(Locator::Id("password")).await?;
    password_field.click().await?;
    password_field.clear().await?;
    password_field.send_keys(new_password).await?;
    let confirm_field =
        env.browser.find(Locator::Id("confirm-password")).await?;
    confirm_field.click().await?;
    confirm_field.clear().await?;
    confirm_field.send_keys(new_password).await?;
    // Blur confirm password field to trigger onchange
    env.browser
        .execute(
            "document.getElementById('confirm-password')?.blur();",
            vec![],
        )
        .await?;
    sleep(Duration::from_millis(100)).await;
    // Blur password field to trigger onchange
    env.browser
        .execute("document.getElementById('password')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 8: Wait for password reset success message
    info!("✅ Waiting for password reset success message");
    let success_message = env
        .browser
        .find(Locator::XPath(
            "//div[contains(text(), 'Password has been reset successfully.')]",
        ))
        .await?;
    let success_text = success_message.text().await.unwrap_or_default();
    info!("Found password reset success message: {}", success_text);

    // Step 9: Log in with new password and verify success
    info!("🔑 Logging in with new password");
    env.browser
        .goto(&format!("{}/login", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;
    let username_field = env.browser.find(Locator::Id("username")).await?;
    username_field.click().await?;
    username_field.clear().await?;
    username_field.send_keys(&credentials.username).await?;
    let password_field = env.browser.find(Locator::Id("password")).await?;
    password_field.click().await?;
    password_field.clear().await?;
    password_field.send_keys(new_password).await?;
    env.browser
        .execute("document.getElementById('password')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;
    let current_url = env.browser.current_url().await?;
    debug!("Current URL after login with new password: {}", current_url);
    assert!(
        !current_url.as_str().contains("/login"),
        "Should not remain on login page after successful login with new password"
    );

    // Step 10: Attempt login with old password and verify failure
    info!("❌ Attempting login with old password");
    env.browser
        .goto(&format!("{}/login", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;
    let username_field = env.browser.find(Locator::Id("username")).await?;
    username_field.click().await?;
    username_field.clear().await?;
    username_field.send_keys(&credentials.username).await?;
    let password_field = env.browser.find(Locator::Id("password")).await?;
    password_field.click().await?;
    password_field.clear().await?;
    password_field.send_keys(&credentials.password).await?;
    env.browser
        .execute("document.getElementById('password')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;
    let current_url = env.browser.current_url().await?;
    debug!("Current URL after login with old password: {}", current_url);
    assert!(
        current_url.as_str().contains("/login"),
        "Should remain on login page after failed login with old password"
    );
    let error_element = env
        .browser
        .find(Locator::Css(".bg-red-50, .bg-red-900, [role='alert']"))
        .await;
    assert!(
        error_element.is_ok(),
        "Error message should be displayed for invalid login with old password"
    );

    info!("✅ Password reset flow test completed successfully");
    Ok(())
}

/// UI integration test for US-005: Email verification flow.
///
/// This test covers the user story:
///   As a user, I want to verify my email address so I can activate my account.
///
/// Steps:
/// - Register a new user (unverified)
/// - Confirm the 'Check your email' prompt is shown
/// - Retrieve the verification token from the database
/// - Visit the verification link in the browser
/// - Confirm the UI shows a success message (e.g., 'Email verified successfully!')
/// - Attempt to log in with the verified user and confirm login works
#[tokio::test]
async fn test_email_verification_flow() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Register a new user (unverified)
    info!("📝 Registering new user");
    let username = format!("testverify_{}", rand::random::<u32>());
    let email = format!("{}@example.com", username);
    let password = "TestPassword123!";

    env.browser
        .goto(&format!("{}/register", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;

    let email_field = env.browser.find(Locator::Id("email")).await?;
    email_field.click().await?;
    email_field.clear().await?;
    email_field.send_keys(&email).await?;

    let username_field = env.browser.find(Locator::Id("username")).await?;
    username_field.click().await?;
    username_field.clear().await?;
    username_field.send_keys(&username).await?;

    let password_field = env.browser.find(Locator::Id("password")).await?;
    password_field.click().await?;
    password_field.clear().await?;
    password_field.send_keys(password).await?;

    let confirm_field =
        env.browser.find(Locator::Id("confirm-password")).await?;
    confirm_field.click().await?;
    confirm_field.clear().await?;
    confirm_field.send_keys(password).await?;
    // Blur confirm password field to trigger onchange
    env.browser
        .execute(
            "document.getElementById('confirm-password')?.blur();",
            vec![],
        )
        .await?;
    sleep(Duration::from_millis(100)).await;
    // Blur password field to trigger onchange
    env.browser
        .execute("document.getElementById('password')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;

    // Submit the registration form
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 2: Confirm the 'Check your email' prompt is shown
    info!("🔍 Verifying email verification prompt");
    let page_html = env.browser.source().await.unwrap_or_default();
    debug!("Page HTML after registration: {}", page_html);
    let prompt = env
        .browser
        .find(Locator::XPath("//p[contains(., 'Please check your inbox and click the link to verify your email.')]"))
        .await?;
    let prompt_text = prompt.text().await.unwrap_or_default();
    assert!(
        prompt_text.contains(
            "Please check your inbox and click the link to verify your email."
        ),
        "Verification prompt not found, got: {}",
        prompt_text
    );
    info!("Found verification prompt: {}", prompt_text);

    // Step 3: Retrieve the verification token from the database
    info!("🔑 Retrieving verification token from database");
    let verification_token =
        env.api.get_verification_token_from_db(&email).await?;
    info!("Got verification token: {}", verification_token);

    // Step 4: Visit the verification link in the browser
    info!("🔗 Navigating to verification link");
    env.browser
        .goto(&format!(
            "{}/verify_email?token={}",
            env.frontend_url, verification_token
        ))
        .await?;
    sleep(Duration::from_secs(1)).await;

    // Step 5: Confirm the UI shows a success message
    info!("✅ Verifying success message after email verification");
    let success_message = env
        .browser
        .find(Locator::XPath(
            "//*[contains(text(), 'Email verified successfully!')]",
        ))
        .await?;
    let success_text = success_message.text().await.unwrap_or_default();
    assert!(
        success_text.contains("Email verified successfully!"),
        "Success message not found, got: {}",
        success_text
    );
    info!("Found email verified success message: {}", success_text);

    // Step 6: Attempt to log in with the verified user and confirm login works
    info!("🔑 Logging in with verified user");
    env.browser
        .goto(&format!("{}/login", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;
    let username_field = env.browser.find(Locator::Id("username")).await?;
    username_field.click().await?;
    username_field.clear().await?;
    username_field.send_keys(&username).await?;
    let password_field = env.browser.find(Locator::Id("password")).await?;
    password_field.click().await?;
    password_field.clear().await?;
    password_field.send_keys(password).await?;
    env.browser
        .execute("document.getElementById('password')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;
    let submit_button = env
        .browser
        .find(Locator::Css("button[type='submit']"))
        .await?;
    submit_button.click().await?;
    sleep(Duration::from_secs(1)).await;
    let current_url = env.browser.current_url().await?;
    debug!("Current URL after login: {}", current_url);
    assert!(
        !current_url.as_str().contains("/login"),
        "Should not remain on login page after successful login"
    );

    info!("✅ Email verification flow test completed successfully");
    Ok(())
}

/// UI integration test for US-006: View and edit profile.
///
/// This test covers the user story:
///   As a user, I want to view and edit my profile so I can manage my account information.
///
/// Steps:
/// - Ensure Alice user exists and is verified
/// - Log in as Alice
/// - Navigate to profile page
/// - View current profile information
/// - Edit display name
/// - Save changes
/// - Verify changes persist after page reload
/// - Verify changes are reflected in the profile
#[tokio::test]
async fn test_view_and_edit_profile() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Ensure Alice user exists and is verified
    info!("👤 Ensuring Alice user exists and is verified");
    env.api.create_alice_user().await?;
    let credentials = test_helpers::alice_credentials();

    // Step 2: Log in as Alice
    login_user(&env.browser, &env.frontend_url, &alice_login_credentials())
        .await?;

    // Step 3: Navigate to profile page
    info!("👤 Navigating to profile page");
    env.browser
        .goto(&format!("{}/profile", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;

    // Step 4: View current profile information
    info!("📋 Viewing current profile information");

    // Check for profile page title
    let page_title = env
        .browser
        .find(Locator::XPath("//h1[contains(text(), 'Profile Settings')]"))
        .await?;
    let title_text = page_title.text().await?;
    assert!(
        title_text.contains("Profile Settings"),
        "Profile page title not found"
    );
    info!("Found profile page title: {}", title_text);

    // Check username is displayed (read-only)
    let username_display = env
        .browser
        .find(Locator::XPath(
            "//label[contains(text(), 'Username')]/following-sibling::div",
        ))
        .await?;
    let username_text = username_display.text().await?;
    assert!(
        username_text.contains(&credentials.username),
        "Username not displayed correctly"
    );
    info!("Username displayed: {}", username_text);

    // Check email is displayed (read-only)
    let email_display = env
        .browser
        .find(Locator::XPath(
            "//label[contains(text(), 'Email Address')]/following-sibling::div",
        ))
        .await?;
    let email_text = email_display.text().await?;
    assert!(
        email_text.contains(&credentials.email),
        "Email not displayed correctly"
    );
    info!("Email displayed: {}", email_text);

    // Step 5: Edit display name
    info!("✏️ Editing display name");
    let new_display_name = format!("Alice Test User {}", rand::random::<u32>());

    let display_name_field =
        env.browser.find(Locator::Id("display-name")).await?;
    display_name_field.click().await?;
    display_name_field.clear().await?;
    display_name_field.send_keys(&new_display_name).await?;

    // Blur the field to trigger any validation
    env.browser
        .execute("document.getElementById('display-name')?.blur();", vec![])
        .await?;
    sleep(Duration::from_millis(100)).await;

    // Step 6: Save changes
    info!("💾 Saving profile changes");
    let save_button = env
        .browser
        .find(Locator::XPath("//button[contains(text(), 'Save Changes')]"))
        .await?;
    save_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Wait for and verify success message
    let success_message = env
        .browser
        .find(Locator::XPath(
            "//*[contains(text(), 'Profile updated successfully')]",
        ))
        .await?;
    let success_text = success_message.text().await?;
    assert!(
        success_text.contains("Profile updated successfully"),
        "Success message not found"
    );
    info!("Profile update success message: {}", success_text);

    // Step 7: Verify changes persist after page reload
    info!("🔄 Reloading page to verify changes persist");
    env.browser.refresh().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 8: Verify changes are reflected in the profile
    info!("✅ Verifying saved changes are reflected");

    // Use JavaScript to get the actual input value instead of the HTML attribute
    // because modern frameworks like Yew may not update the HTML attribute
    let field_value: String = env
        .browser
        .execute(
            "return document.getElementById('display-name')?.value || '';",
            vec![],
        )
        .await?
        .as_str()
        .unwrap_or("")
        .to_string();

    assert_eq!(
        field_value, new_display_name,
        "Display name changes did not persist. Expected: '{}', Found: '{}'",
        new_display_name, field_value
    );
    info!(
        "Display name successfully updated and persisted: {}",
        field_value
    );

    // Additional verification: field value is retrieved via JavaScript above

    info!("✅ Profile view and edit test completed successfully");
    Ok(())
}

/// UI integration test for US-007: Logout functionality.
///
/// This test covers the user story:
///   As a user, I want to logout and have my session terminated properly.
///
/// Steps:
/// - Ensure Alice user exists and is verified
/// - Log in as Alice
/// - Verify successful login
/// - Click logout from user menu
/// - Verify session is terminated and redirect to homepage
/// - Verify protected pages require re-login
#[tokio::test]
async fn test_logout_functionality() -> Result<()> {
    let env = TestEnvironment::setup().await?;

    // Step 1: Ensure Alice user exists and is verified
    info!("👤 Ensuring Alice user exists and is verified");
    env.api.create_alice_user().await?;

    // Step 2: Log in as Alice
    login_user(&env.browser, &env.frontend_url, &alice_login_credentials())
        .await?;

    // Step 3: Click logout from user menu
    info!("🔓 Clicking logout from user menu");

    // Try to find the logout button - it might be in desktop or mobile menu
    let logout_button = env
        .browser
        .find(Locator::XPath("//button[contains(text(), 'Logout')]"))
        .await;

    if logout_button.is_err() {
        // If desktop logout button is not visible, try to find and click mobile menu toggle
        info!("🔍 Desktop logout not visible, checking mobile menu");
        let mobile_menu_button = env
            .browser
            .find(Locator::Css("button[aria-label='Toggle navigation menu']"))
            .await;

        if let Ok(mobile_button) = mobile_menu_button {
            mobile_button.click().await?;
            sleep(Duration::from_millis(300)).await; // Wait for mobile menu to open
        }
    }

    // Now try to find the logout button again
    let logout_button = env
        .browser
        .find(Locator::XPath("//button[contains(text(), 'Logout')]"))
        .await?;

    logout_button.click().await?;
    sleep(Duration::from_secs(1)).await;

    // Step 4: Verify session is terminated and redirect to homepage
    info!("🔍 Verifying session termination and redirect to homepage");
    let current_url = env.browser.current_url().await?;
    debug!("Current URL after logout: {}", current_url);

    // Should be redirected to homepage
    assert!(
        current_url.as_str().ends_with("/")
            || current_url.as_str().contains("/#/"),
        "Should be redirected to homepage after logout"
    );

    // Should not see logout button anymore (user is logged out)
    let logout_button_after = env
        .browser
        .find(Locator::XPath("//button[contains(text(), 'Logout')]"))
        .await;
    assert!(
        logout_button_after.is_err(),
        "Logout button should not be visible after logout"
    );

    // Should see login button instead
    let login_link = env
        .browser
        .find(Locator::XPath("//a[contains(text(), 'Login')]"))
        .await;
    assert!(
        login_link.is_ok(),
        "Login link should be visible after logout"
    );

    // Step 5: Verify protected pages require re-login
    info!("🔒 Verifying protected pages require re-login");

    // Try to access a protected page (communities)
    env.browser
        .goto(&format!("{}/communities", env.frontend_url))
        .await?;
    sleep(Duration::from_secs(1)).await;

    let protected_url = env.browser.current_url().await?;
    debug!("URL after accessing protected page: {}", protected_url);

    // Should be redirected to login or show login requirement
    // The exact behavior depends on implementation - either redirect to login or show access denied
    let is_redirected_to_login = protected_url.as_str().contains("/login");
    let has_login_form =
        env.browser.find(Locator::Id("username")).await.is_ok();
    let has_access_denied = env
        .browser
        .find(Locator::XPath("//*[contains(text(), 'login') or contains(text(), 'authenticated')]"))
        .await
        .is_ok();

    assert!(
        is_redirected_to_login || has_login_form || has_access_denied,
        "Protected page should require authentication after logout"
    );

    info!("✅ Logout functionality test completed successfully");
    Ok(())
}
