use actix_web::rt::task::JoinHandle;
use tracing::Subscriber;
use tracing::subscriber::set_global_default;
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, Registry, fmt, layer::SubscriberExt};

/// Log an error if it exists using the alternate selector, which emits the
/// error chain.
pub fn log_error(e: impl Into<anyhow::Error>) {
    let e: anyhow::Error = e.into();
    tracing::error!("{e:#}");
}

pub fn get_subscriber(env_filter: String) -> impl Subscriber + Sync + Send {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(env_filter));
    let stderr = fmt::Layer::new()
        .with_writer(std::io::stderr)
        .pretty()
        .with_span_events(fmt::format::FmtSpan::CLOSE);
    Registry::default().with(env_filter).with(stderr)
}

/// Register a subscriber as global default to process span data.
///
/// It should only be called once!
pub fn init_subscriber(subscriber: impl Subscriber + Sync + Send) {
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}

pub fn spawn_blocking_with_tracing<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let current_span = tracing::Span::current();
    actix_web::rt::task::spawn_blocking(move || current_span.in_scope(f))
}
