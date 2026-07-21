use tracing_subscriber::EnvFilter;

/// Shared tracing-subscriber init for every binary. Reads `RUST_LOG`, defaults to `info`.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
