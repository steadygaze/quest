use std::{collections::HashMap, hash::RandomState};

use actix_web::HttpRequest;
use config::{builder::DefaultState, Config, ConfigBuilder, ConfigError};
use fred::clients::RedisPool;
use oauth2::basic::BasicClient;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    error::{Error, Result},
    session,
};

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

impl AppState {
    pub async fn get_session(
        &self,
        request: HttpRequest,
    ) -> Result<(HashMap<String, String, RandomState>, Uuid)> {
        let session_cookie = match request.cookie(session::SESSION_ID_COOKIE) {
            Some(session_cookie) => session_cookie,
            None => {
                return Err(Error::AuthorizationError(
                    "You must be logged in to access this page.".to_string(),
                ))
            }
        };
        session::get_session_info(
            &self.redis_pool,
            &self.regex.alphanumeric,
            session_cookie.value(),
        )
        .await
    }
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
pub fn config_with_defaults() -> std::result::Result<ConfigBuilder<DefaultState>, ConfigError> {
    Ok(Config::builder()
        .set_default("site_name", "Quest")?
        .set_default("port", 8080)?)
}
