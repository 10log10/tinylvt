#[cfg(not(feature = "test-utils"))]
use anyhow::Context;
use anyhow::Result;
#[cfg(not(feature = "test-utils"))]
use resend_rs::{Resend, types::CreateEmailBaseOptions};
#[cfg(not(feature = "test-utils"))]
use secrecy::ExposeSecret;
use secrecy::SecretBox;

pub struct EmailService {
    #[cfg(not(feature = "test-utils"))]
    client: Resend,
    from_address: String,
}

#[derive(Debug)]
pub struct EmailTemplate {
    pub subject: String,
    pub html_body: String,
    pub text_body: String,
}

impl EmailService {
    #[cfg(not(feature = "test-utils"))]
    pub fn new(api_key: SecretBox<String>, from_address: String) -> Self {
        let client = Resend::new(api_key.expose_secret());
        Self {
            client,
            from_address,
        }
    }

    #[cfg(feature = "test-utils")]
    pub fn new(_api_key: SecretBox<String>, from_address: String) -> Self {
        Self { from_address }
    }

    #[tracing::instrument(skip(self), fields(to = %to_email))]
    #[cfg(not(feature = "test-utils"))]
    pub async fn send_email(
        &self,
        to_email: &str,
        template: EmailTemplate,
    ) -> Result<()> {
        let email = CreateEmailBaseOptions::new(
            &self.from_address,
            [to_email],
            &template.subject,
        )
        .with_html(&template.html_body)
        .with_text(&template.text_body);

        self.client
            .emails
            .send(email)
            .await
            .context("Failed to send email via Resend")?;

        tracing::info!("Email sent successfully");
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(to = %to_email))]
    #[cfg(feature = "test-utils")]
    pub async fn send_email(
        &self,
        to_email: &str,
        template: EmailTemplate,
    ) -> Result<()> {
        tracing::info!(
            "Test mode: Mock email sent to: {} from: {} with subject: {}",
            to_email,
            self.from_address,
            template.subject
        );
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn send_verification_email(
        &self,
        to_email: &str,
        username: &str,
        verification_token: &str,
        base_url: &str,
    ) -> Result<()> {
        let verification_link =
            format!("{}/verify-email?token={}", base_url, verification_token);

        let template = EmailTemplate {
            subject: "Verify your email address".to_string(),
            html_body: format!(
                r#"
                <h2>Welcome to TinyLVT, {}!</h2>
                <p>Thank you for signing up. Please click the link below to verify your email address:</p>
                <p><a href="{}" style="background-color: #007bff; color: white; padding: 10px 20px; text-decoration: none; border-radius: 5px;">Verify Email</a></p>
                <p>Or copy and paste this link in your browser:</p>
                <p>{}</p>
                <p>This link will expire in 24 hours.</p>
                <p>If you didn't create an account, you can safely ignore this email.</p>
                "#,
                username, verification_link, verification_link
            ),
            text_body: format!(
                r#"
Welcome to TinyLVT, {}!

Thank you for signing up. Please visit the following link to verify your email address:

{}

This link will expire in 24 hours.

If you didn't create an account, you can safely ignore this email.
                "#,
                username, verification_link
            ),
        };

        self.send_email(to_email, template).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        username: &str,
        reset_token: &str,
        base_url: &str,
    ) -> Result<()> {
        let reset_link =
            format!("{}/reset-password?token={}", base_url, reset_token);

        let template = EmailTemplate {
            subject: "Reset your password".to_string(),
            html_body: format!(
                r#"
                <h2>Password Reset Request</h2>
                <p>Hi {},</p>
                <p>We received a request to reset your password for your TinyLVT account.</p>
                <p><a href="{}" style="background-color: #dc3545; color: white; padding: 10px 20px; text-decoration: none; border-radius: 5px;">Reset Password</a></p>
                <p>Or copy and paste this link in your browser:</p>
                <p>{}</p>
                <p>This link will expire in 1 hour.</p>
                <p>If you didn't request this password reset, you can safely ignore this email. Your password will not be changed.</p>
                "#,
                username, reset_link, reset_link
            ),
            text_body: format!(
                r#"
Password Reset Request

Hi {},

We received a request to reset your password for your TinyLVT account.

Please visit the following link to reset your password:

{}

This link will expire in 1 hour.

If you didn't request this password reset, you can safely ignore this email. Your password will not be changed.
                "#,
                username, reset_link
            ),
        };

        self.send_email(to_email, template).await
    }
}
