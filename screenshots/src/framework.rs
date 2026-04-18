//! Browser automation framework for taking screenshots.
//!
//! Based on the archived ui-tests framework, simplified for screenshot capture.

use anyhow::{Context, Result};
use fantoccini::{Client, ClientBuilder};
use rand::Rng;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use test_helpers::TestApp;
use tokio::sync::OnceCell;
use tokio::time::sleep;
use tracing::{debug, info, warn};

// Global state for ensuring trunk build only happens once
static TRUNK_BUILD_ONCE: OnceCell<Result<(), String>> = OnceCell::const_new();

async fn ensure_trunk_build(
    backend_url: &str,
    support_email: &str,
) -> Result<()> {
    let backend_url = backend_url.to_string();
    let support_email = support_email.to_string();

    let result = TRUNK_BUILD_ONCE
        .get_or_init(|| async move {
            info!("Building frontend with trunk build");
            let build_result = Command::new("trunk")
                .arg("build")
                .current_dir("ui")
                .env("BACKEND_URL", &backend_url)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();

            match build_result {
                Ok(status) if status.success() => {
                    info!("Frontend build successful");
                    Ok(())
                }
                Ok(status) => Err(format!(
                    "Frontend build failed with status: {}",
                    status
                )),
                Err(e) => Err(format!("Failed to run trunk build: {}", e)),
            }
        })
        .await;

    match result {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("{}", e)),
    }
}

#[allow(dead_code)]
pub struct ScreenshotEnvironment {
    pub api: TestApp,
    pub browser: Client,
    pub frontend_process: Child,
    pub geckodriver_process: Child,
    pub frontend_url: String,
}

#[allow(dead_code)]
impl ScreenshotEnvironment {
    /// Set up environment with headless browser (for CI/automated screenshots)
    pub async fn setup() -> Result<Self> {
        Self::setup_with_options(false, 800, 800).await
    }

    /// Set up environment with headed browser (for debugging)
    pub async fn setup_headed() -> Result<Self> {
        Self::setup_with_options(true, 800, 800).await
    }

    /// Set up with custom window size
    pub async fn setup_with_size(width: u32, height: u32) -> Result<Self> {
        Self::setup_with_options(false, width, height).await
    }

    async fn setup_with_options(
        headed: bool,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        info!("Setting up screenshot environment");

        // Step 1: Start API server
        info!("Starting API server");
        let api = test_helpers::spawn_app().await;
        let api_url = format!("http://localhost:{}", api.port);
        info!("API server running on {}", api_url);

        // Step 2: Start geckodriver
        info!("Starting geckodriver");
        let (geckodriver_process, gecko_port) =
            start_geckodriver_with_retry(4444).await?;
        info!("Geckodriver running on port {}", gecko_port);

        // Step 3: Start frontend
        info!("Starting frontend");
        let (frontend_process, frontend_port) =
            start_frontend_with_retry(8080, &api_url).await?;
        let frontend_url = format!("http://localhost:{}", frontend_port);

        wait_for_frontend(&frontend_url).await?;
        info!("Frontend ready at {}", frontend_url);

        // Step 4: Connect to browser
        info!("Connecting to browser");
        let browser =
            connect_to_browser(gecko_port, headed, width, height).await?;
        info!("Browser connected");

        Ok(ScreenshotEnvironment {
            api,
            browser,
            frontend_process,
            geckodriver_process,
            frontend_url,
        })
    }

    /// Take a screenshot and save it to the specified path
    pub async fn screenshot(&self, path: &str) -> Result<()> {
        let png_data = self.browser.screenshot().await?;
        std::fs::write(path, &png_data)?;
        info!("Screenshot saved to {}", path);
        Ok(())
    }

    /// Take a full-page screenshot by capturing the body element.
    /// Firefox's element screenshot captures the full element even if it
    /// extends beyond the viewport.
    pub async fn screenshot_full_page(&self, path: &str) -> Result<()> {
        use fantoccini::Locator;

        let body = self.browser.find(Locator::Css("body")).await?;
        let png_data = body.screenshot().await?;
        std::fs::write(path, &png_data)?;
        info!("Full-page screenshot saved to {}", path);

        Ok(())
    }

    /// Set dark mode preference via JavaScript
    pub async fn set_dark_mode(&self, enabled: bool) -> Result<()> {
        let script = if enabled {
            "document.documentElement.classList.add('dark');"
        } else {
            "document.documentElement.classList.remove('dark');"
        };
        self.browser.execute(script, vec![]).await?;
        // Wait for transition-colors animations to complete (typically 150-300ms)
        sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    /// Scroll to the bottom of the page
    pub async fn scroll_to_bottom(&self) -> Result<()> {
        self.browser
            .execute("window.scrollTo(0, document.body.scrollHeight);", vec![])
            .await?;
        sleep(Duration::from_millis(200)).await;
        Ok(())
    }

    /// Scroll to the top of the page
    pub async fn scroll_to_top(&self) -> Result<()> {
        self.browser
            .execute("window.scrollTo(0, 0);", vec![])
            .await?;
        sleep(Duration::from_millis(200)).await;
        Ok(())
    }

    /// Scroll to a specific Y position
    pub async fn scroll_to(&self, y: u32) -> Result<()> {
        self.browser
            .execute(&format!("window.scrollTo(0, {});", y), vec![])
            .await?;
        sleep(Duration::from_millis(200)).await;
        Ok(())
    }

    /// Navigate to a URL (relative to frontend)
    pub async fn goto(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.frontend_url, path);
        self.browser.goto(&url).await?;
        // Wait for page to load
        sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    /// Wait for an element to be present
    pub async fn wait_for(&self, selector: &str) -> Result<()> {
        use fantoccini::Locator;
        for _ in 0..30 {
            if self.browser.find(Locator::Css(selector)).await.is_ok() {
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }
        Err(anyhow::anyhow!("Timeout waiting for element: {}", selector))
    }

    /// Take full-page screenshots in both light and dark mode
    pub async fn screenshot_both_modes(
        &self,
        base_path: &std::path::Path,
        name: &str,
    ) -> Result<()> {
        // Light mode
        self.set_dark_mode(false).await?;
        let light_path = base_path.join(format!("{}-light.png", name));
        self.screenshot_full_page(&light_path.to_string_lossy())
            .await?;

        // Dark mode
        self.set_dark_mode(true).await?;
        let dark_path = base_path.join(format!("{}-dark.png", name));
        self.screenshot_full_page(&dark_path.to_string_lossy())
            .await?;

        Ok(())
    }
}

impl Drop for ScreenshotEnvironment {
    fn drop(&mut self) {
        info!("Cleaning up screenshot environment");

        if let Err(e) = self.frontend_process.kill() {
            warn!("Failed to kill frontend process: {}", e);
        }

        if let Err(e) = self.geckodriver_process.kill() {
            warn!("Failed to kill geckodriver process: {}", e);
        }

        info!("Cleanup complete");
    }
}

async fn start_geckodriver_with_retry(base_port: u16) -> Result<(Child, u16)> {
    for attempt in 1..=5 {
        let port = base_port + rand::thread_rng().gen_range(0..=100);
        debug!(
            "Attempting to start geckodriver on port {} (attempt {})",
            port, attempt
        );

        match Command::new("geckodriver")
            .arg("--port")
            .arg(port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                sleep(Duration::from_millis(500)).await;

                match child.try_wait() {
                    Ok(Some(status)) => {
                        debug!(
                            "Geckodriver exited with status {}, trying different port",
                            status
                        );
                    }
                    Ok(None) => {
                        return Ok((child, port));
                    }
                    Err(e) => {
                        debug!("Error checking geckodriver status: {}", e);
                        let _ = child.kill();
                    }
                }
            }
            Err(e) => {
                debug!("Failed to start geckodriver: {}", e);
            }
        }

        if attempt < 5 {
            sleep(Duration::from_millis(100)).await;
        }
    }

    Err(anyhow::anyhow!(
        "Failed to start geckodriver after 5 attempts"
    ))
}

async fn start_frontend_with_retry(
    base_port: u16,
    backend_url: &str,
) -> Result<(Child, u16)> {
    let support_email = "support@example.com";
    ensure_trunk_build(backend_url, support_email).await?;

    for attempt in 1..=5 {
        let port = base_port + rand::thread_rng().gen_range(0..=100);
        debug!(
            "Attempting to start frontend on port {} (attempt {})",
            port, attempt
        );

        match Command::new("trunk")
            .arg("serve")
            .arg("--port")
            .arg(port.to_string())
            .current_dir("ui")
            .env("BACKEND_URL", backend_url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                sleep(Duration::from_millis(500)).await;

                match child.try_wait() {
                    Ok(Some(status)) => {
                        debug!(
                            "Frontend exited with status {}, trying different port",
                            status
                        );
                    }
                    Ok(None) => {
                        return Ok((child, port));
                    }
                    Err(e) => {
                        debug!("Error checking frontend status: {}", e);
                        let _ = child.kill();
                    }
                }
            }
            Err(e) => {
                debug!("Failed to start frontend: {}", e);
            }
        }

        if attempt < 5 {
            sleep(Duration::from_millis(100)).await;
        }
    }

    Err(anyhow::anyhow!("Failed to start frontend after 5 attempts"))
}

async fn wait_for_frontend(url: &str) -> Result<()> {
    for i in 1..=30 {
        match reqwest::get(url).await {
            Ok(response) if response.status().is_success() => {
                debug!("Frontend ready after {} attempts", i);
                return Ok(());
            }
            _ => {
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
    Err(anyhow::anyhow!(
        "Frontend failed to start after 30 attempts"
    ))
}

async fn connect_to_browser(
    gecko_port: u16,
    headed: bool,
    width: u32,
    height: u32,
) -> Result<Client> {
    let gecko_url = format!("http://localhost:{}", gecko_port);

    let mut caps = serde_json::Map::new();
    // Use 2x device pixel ratio for high-resolution screenshots
    let firefox_opts = if headed {
        info!("Starting browser in headed mode");
        serde_json::json!({
            "log": {"level": "error"},
            "prefs": {
                "layout.css.devPixelsPerPx": "2.0"
            }
        })
    } else {
        info!("Starting browser in headless mode");
        serde_json::json!({
            "args": [
                "--headless",
                &format!("--width={}", width),
                &format!("--height={}", height)
            ],
            "log": {"level": "error"},
            "prefs": {
                "layout.css.devPixelsPerPx": "2.0"
            }
        })
    };
    caps.insert("moz:firefoxOptions".to_string(), firefox_opts);

    let client = ClientBuilder::native()
        .capabilities(caps)
        .connect(&gecko_url)
        .await
        .context("Failed to connect to geckodriver")?;

    // Set window size for headed mode
    if headed {
        client.set_window_size(width, height).await?;
    }

    Ok(client)
}

/// Login as a user via the UI
pub async fn login_user(
    env: &ScreenshotEnvironment,
    credentials: &payloads::requests::LoginCredentials,
) -> Result<()> {
    use fantoccini::Locator;

    info!("Logging in as {}", credentials.username);

    env.goto("/login").await?;

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
    sleep(Duration::from_millis(500)).await;

    let current_url = env.browser.current_url().await?;
    if current_url.as_str().contains("/login") {
        return Err(anyhow::anyhow!("Login failed - still on login page"));
    }

    info!("Successfully logged in as {}", credentials.username);
    Ok(())
}
