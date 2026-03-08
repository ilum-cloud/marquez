use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SentryConfig {
    pub dsn: Option<String>,
    #[serde(default = "default_environment")]
    pub environment: String,
    #[serde(default = "default_traces_sample_rate")]
    pub traces_sample_rate: f32,
    #[serde(default)]
    pub debug: bool,
}

fn default_environment() -> String {
    "local".to_string()
}

fn default_traces_sample_rate() -> f32 {
    0.01
}

/// Initialize Sentry. Returns guard that must be held for app lifetime.
/// Returns None if DSN is not configured (Sentry disabled).
pub fn init_sentry(config: &SentryConfig) -> Option<sentry::ClientInitGuard> {
    let dsn = config.dsn.as_deref().filter(|s| !s.is_empty())?;
    Some(sentry::init(sentry::ClientOptions {
        dsn: dsn.parse().ok(),
        environment: Some(config.environment.clone().into()),
        traces_sample_rate: config.traces_sample_rate,
        debug: config.debug,
        ..Default::default()
    }))
}

/// Returns a tower Layer that creates Sentry transactions per HTTP request.
pub fn sentry_http_layer() -> sentry_tower::SentryHttpLayer {
    sentry_tower::SentryHttpLayer::new()
}
