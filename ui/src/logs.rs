//! Logging to the javascript console and to the backend.

use tracing_subscriber::{EnvFilter, prelude::*};
use tracing_web::MakeWebConsoleWriter;

/// Initialize logging
pub fn init_logging() {
    let env_filter = EnvFilter::new("error,ui=debug");

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_line_number(true)
        .with_ansi(false) // Only partially supported across browsers
        .without_time() // std::time is not available in browsers
        .with_writer(MakeWebConsoleWriter::new().with_pretty_level())
        .with_level(false);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    tracing::info!("Initialized logs");
}
