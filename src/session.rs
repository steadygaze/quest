use std::{collections::HashMap, hash::RandomState};

use fred::{clients::RedisPool, interfaces::HashesInterface, prelude::RedisError};

use crate::key;

pub async fn get_session_info(
    redis_pool: RedisPool,
    session_id: &str,
) -> Result<HashMap<String, String, RandomState>, RedisError> {
    redis_pool
        .hgetall::<HashMap<String, String, _>, _>(key::session(session_id))
        .await
}
