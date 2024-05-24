use fred::clients::RedisPool;
use oauth2::basic::BasicClient;
use serde::Deserialize;

/// Actix state object for dependency injection.
#[derive(Clone)]
pub struct AppState {
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
