//! Run with debugging output:
//!
//! ```shell
//! RUST_LOG=ui_tests=debug,api=info cargo test -- --nocapture
//! ```

use anyhow::{Context, Result};
use fantoccini::{Client, ClientBuilder};
use rand::Rng;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use test_helpers::TestApp;
use tokio::time::sleep;
use tracing::{debug, info, warn};

pub struct TestEnvironment {
    pub api: TestApp,
    pub browser: Client,
    pub frontend_process: Child,
    pub geckodriver_process: Child,
    pub frontend_url: String,
}

impl TestEnvironment {
    #[cfg(test)]
    #[allow(dead_code)]
    pub async fn setup() -> Result<Self> {
        Self::setup_with_options(false).await
    }

    pub async fn setup_headed() -> Result<Self> {
        Self::setup_with_options(true).await
    }

    async fn setup_with_options(headed: bool) -> Result<Self> {
        info!("🔧 Setting up test environment");

        // Step 1: Start API server (using test-helpers)
        info!("🚀 Starting API server");
        let api = test_helpers::spawn_app().await;
        let api_url = format!("http://localhost:{}", api.port);
        info!("✅ API server running on {}", api_url);

        // Step 2: Start geckodriver with retry logic
        info!("🦎 Starting geckodriver");
        let (geckodriver_process, gecko_port) =
            start_geckodriver_with_retry(4444).await?;
        info!("✅ Geckodriver running on port {}", gecko_port);

        // Step 3: Start frontend with retry logic
        info!("🎨 Starting frontend");
        let (frontend_process, frontend_port) =
            start_frontend_with_retry(8080, &api_url).await?;
        let frontend_url = format!("http://localhost:{}", frontend_port);

        // Wait for frontend to be ready
        wait_for_frontend(&frontend_url).await?;
        info!("✅ Frontend ready at {}", frontend_url);

        // Step 4: Connect to browser
        info!("🌐 Connecting to browser");
        let browser = connect_to_browser(gecko_port, headed).await?;
        info!("✅ Browser connected");

        Ok(TestEnvironment {
            api,
            browser,
            frontend_process,
            geckodriver_process,
            frontend_url,
        })
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        info!("🧹 Cleaning up test environment");

        // Kill frontend process
        if let Err(e) = self.frontend_process.kill() {
            warn!("Failed to kill frontend process: {}", e);
        }

        // Kill geckodriver process
        if let Err(e) = self.geckodriver_process.kill() {
            warn!("Failed to kill geckodriver process: {}", e);
        }

        info!("✅ Cleanup complete");
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
                // Give geckodriver a moment to either start successfully or exit with error
                sleep(Duration::from_millis(500)).await;

                // Check if the process is still running (success) or exited (failure)
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // Process exited - likely port conflict or other startup error
                        debug!(
                            "Geckodriver exited with status {}, trying different port",
                            status
                        );
                    }
                    Ok(None) => {
                        // Process still running - success!
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
    // First, ensure the frontend builds successfully
    debug!("Building frontend with trunk build");
    let build_result = Command::new("trunk")
        .arg("build")
        .current_dir("../ui")
        .env("BACKEND_URL", backend_url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match build_result {
        Ok(status) if status.success() => {
            debug!("Frontend build successful");
        }
        Ok(status) => {
            return Err(anyhow::anyhow!(
                "Frontend build failed with status: {}",
                status
            ));
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to run trunk build: {}", e));
        }
    }

    // Now try to start the server on different ports
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
            .current_dir("../ui")
            .env("BACKEND_URL", backend_url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                // Give trunk serve a moment to start (should be quick since build is done)
                sleep(Duration::from_millis(500)).await;

                // Check if the process is still running (success) or exited (failure)
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // Process exited - likely port conflict
                        debug!(
                            "Frontend exited with status {}, trying different port",
                            status
                        );
                    }
                    Ok(None) => {
                        // Process still running - success!
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

async fn connect_to_browser(gecko_port: u16, headed: bool) -> Result<Client> {
    let gecko_url = format!("http://localhost:{}", gecko_port);

    // Configure browser options based on headed parameter
    let mut caps = serde_json::Map::new();
    let firefox_opts = if headed {
        info!("🖥️ Starting browser in headed mode");
        serde_json::json!({
            "log": {"level": "error"}
        })
    } else {
        info!("👻 Starting browser in headless mode");
        serde_json::json!({
            "args": ["--headless"],
            "log": {"level": "error"}
        })
    };
    caps.insert("moz:firefoxOptions".to_string(), firefox_opts);

    let client = ClientBuilder::native()
        .capabilities(caps)
        .connect(&gecko_url)
        .await
        .context("Failed to connect to geckodriver")?;

    Ok(client)
}
