use std::{collections::HashMap, hash::RandomState};

use anyhow::Context;
use fred::{
    clients::RedisPool,
    interfaces::{HashesInterface, KeysInterface},
};
use regex::Regex;
use uuid::Uuid;

use crate::error::Error;
use crate::error::Result;
use crate::key;

pub const SESSION_ID_COOKIE: &str = "sid";

/// Helper function for clearing the server's session record. This has to be
/// done if we notice it's corrupted in some way.
fn background_clear_session(redis_pool: &RedisPool, session_id: &str) {
    let session_id = session_id.to_string();
    let redis_pool = redis_pool.clone();
    tokio::spawn(async move {
        if let Err(err) = redis_pool.del::<String, _>(key::session(&session_id)).await {
            log::warn!("Ignored error clearing invalid session entry: {err}");
        }
    });
}

/// Data associated with a session.
pub struct SessionInfo {
    pub raw: HashMap<String, String, RandomState>,
    pub account_id: Uuid,
    pub current_profile: Option<ProfileRenderInfo>,
}

/// Data necessary for rendering a page with a logged in user.
pub struct ProfileRenderInfo {
    pub display_name: String,
    pub username: String,
}

/// Helper function to retrieve a user's session details, with some basic
/// validation. Note that the AuthenticationError handler will clear the user's
/// session cookie.
pub async fn get_session_info(
    redis_pool: &RedisPool,
    alphanumeric: &Regex,
    session_id: &str,
) -> Result<SessionInfo> {
    if session_id.len() != 32 || !alphanumeric.is_match(session_id) {
        return Err(Error::AuthenticationError(
            "Your session was corrupted. Try logging in again.".to_string(),
        ));
    }
    let raw = redis_pool
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
                    background_clear_session(redis_pool, session_id);
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
                current_profile,
            })
        }
        None => {
            if !raw.is_empty() {
                // A session hash with no account_id is invalid. Delete it.
                background_clear_session(redis_pool, session_id);
            }
            return Err(Error::AuthenticationError(
                "Your session expired or was corrupted. Try logging in again.".to_string(),
            ));
        }
    }
}
