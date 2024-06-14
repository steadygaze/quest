use std::{collections::HashMap, hash::RandomState};

use actix_web::HttpRequest;
use anyhow::Context;
use config::{builder::DefaultState, Config, ConfigBuilder, ConfigError};
use fred::{
    clients::RedisPool,
    interfaces::{HashesInterface, KeysInterface},
};
use oauth2::basic::BasicClient;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::key;

pub const SESSION_ID_COOKIE: &str = "sid";

/// Data associated with a session.
pub struct SessionInfo {
    pub raw: HashMap<String, String, RandomState>,
    pub account_id: Uuid,
    pub session_id: String,
    pub current_profile: Option<ProfileRenderInfo>,
}

/// Data necessary for rendering a page with a logged in user.
pub struct ProfileRenderInfo {
    pub display_name: String,
    pub username: String,
}

/// Actix state object that all route handlers will have access to.
#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db_pool: sqlx::postgres::PgPool,
    pub redis_pool: RedisPool,
    pub oauth_client: BasicClient,
    pub regex: CompiledRegexes,
    pub uuid_seed: [u8; 6],
}

impl AppState {
    /// Helper to get a user's session details.
    pub async fn get_session(&self, request: HttpRequest) -> Option<Result<SessionInfo>> {
        if let Some(session_cookie) = request.cookie(SESSION_ID_COOKIE) {
            Some(self.get_session_for(session_cookie.value()).await)
        } else {
            None
        }
    }

    /// Helper to get a user's session details that also requires that they be
    /// logged in.
    pub async fn require_session(&self, request: HttpRequest) -> Result<SessionInfo> {
        match self.get_session(request).await {
            Some(session_info) => session_info,
            None => Err(Error::AuthorizationError(
                "You must be logged in to access this page.".to_string(),
            )),
        }
    }

    /// Helper function for clearing the server's session record. This has to be
    /// done if we notice it's corrupted in some way.
    fn background_clear_session(&self, session_id: &str) {
        let session_id = session_id.to_string();
        let redis_pool = self.redis_pool.clone();
        tokio::spawn(async move {
            if let Err(err) = redis_pool.del::<String, _>(key::session(&session_id)).await {
                log::warn!("Ignored error clearing invalid session entry: {err}");
            }
        });
    }

    fn valid_session_id(&self, session_id: &str) -> bool {
        session_id.len() == 32 && self.regex.alphanumeric.is_match(session_id)
    }

    async fn get_session_for(&self, session_id: &str) -> Result<SessionInfo> {
        if !self.valid_session_id(session_id) {
            return Err(Error::AuthenticationError(
                "Your session was corrupted. Try logging in again.".to_string(),
            ));
        }

        let raw = self
            .redis_pool
            .hgetall::<HashMap<String, String, _>, _>(key::session(session_id))
            .await
            .context("Failed to retrieve session info")?;
        // TODO - Create shortened hash key constant system.
        match raw.get("account_id") {
            Some(account_id) => {
                let account_id = match Uuid::try_parse(account_id) {
                    Ok(account_id) => account_id,
                    Err(err) => {
                        log::error!("Unparseable account_id: {}", account_id);
                        self.background_clear_session(session_id);
                        return Err(Error::AuthenticationError(
                            "Your session was corrupted. Try logging in again.".to_string(),
                        ));
                    }
                };

                let current_profile = raw.get("username").and_then(|username| {
                    raw.get("display_name").and_then(|display_name| {
                        Some(ProfileRenderInfo {
                            username: username.clone(),
                            display_name: display_name.clone(),
                        })
                    })
                });

                Ok(SessionInfo {
                    raw,
                    account_id,
                    session_id: session_id.to_string(),
                    current_profile,
                })
            }
            None => {
                if !raw.is_empty() {
                    // A session hash with no account_id is invalid. Delete it.
                    self.background_clear_session(session_id);
                }
                return Err(Error::AuthenticationError(
                    "Your session expired or was corrupted. Try logging in again.".to_string(),
                ));
            }
        }
    }
}

/// Struct for storing compiled regexes.
#[derive(Clone)]
pub struct CompiledRegexes {
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
