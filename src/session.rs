use std::{collections::HashMap, hash::RandomState};

use anyhow::Context;
use fred::{
    clients::RedisPool,
    interfaces::{HashesInterface, KeysInterface},
};
use regex::Regex;

use crate::error::Error;
use crate::error::Result;
use crate::key;

pub const SESSION_ID_COOKIE: &str = "sid";

pub async fn get_session_info(
    redis_pool: &RedisPool,
    alphanumeric: &Regex,
    session_id: &str,
) -> Result<HashMap<String, String, RandomState>> {
    if session_id.len() != 32 || !alphanumeric.is_match(session_id) {
        return Err(Error::AuthenticationError(
            "Your session was corrupted. Try logging in again.".to_string(),
        ));
    }
    let session_info = redis_pool
        .hgetall::<HashMap<String, String, _>, _>(key::session(session_id))
        .await
        .context("Failed to retrieve session info")?;
    if let None = session_info.get("user_id") {
        if !session_info.is_empty() {
            // A session hash with no user_id is invalid. Delete it.
            let session_id = session_id.to_string();
            let redis_pool = redis_pool.clone();
            tokio::spawn(async move {
                if let Err(err) = redis_pool.del::<String, _>(key::session(&session_id)).await {
                    log::warn!("Ignored error clearing invalid session entry: {err}");
                }
            });
        }
        return Err(Error::AuthenticationError(
            "Your session expired or was corrupted. Try logging in again.".to_string(),
        ));
    }
    Ok(session_info)
}
