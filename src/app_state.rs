use config::{builder::DefaultState, Config, ConfigBuilder, ConfigError};
use fred::clients::RedisPool;
use oauth2::basic::BasicClient;
use serde::Deserialize;
use std::sync::Arc;

pub type FnBox = Box<dyn FnOnce() + Send + 'static>;

/// Actix state object for dependency injection.
#[derive(Clone)]
pub struct AppState {
    /// Send a task to be executed by a background thread, not returning any
    /// result.
    pub background_sender: Arc<crossbeam_channel::Sender<FnBox>>,
    pub config: AppConfig,
    pub db_pool: sqlx::postgres::PgPool,
    pub redis_pool: RedisPool,
    pub oauth_client: BasicClient,
    pub regex: CompiledRegex,
    pub uuid_seed: [u8; 6],
}

#[derive(Clone)]
pub struct CompiledRegex {
    pub alphanumeric: regex::Regex,
    pub oauth_state_ok: regex::Regex,
}

#[derive(Clone, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub discord_app_id: String,
    pub discord_client_secret: String,
    pub port: u16,
    pub redis_url: String,
    pub site_name: String,
}

/// Create a config builder with default values set.
pub fn config_with_defaults() -> Result<ConfigBuilder<DefaultState>, ConfigError> {
    Ok(Config::builder()
        .set_default("site_name", "Quest")?
        .set_default("port", 8080)?)
}
