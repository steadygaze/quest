use fred::clients::RedisPool;
use oauth2::basic::BasicClient;

/// Actix state object for dependency injection.
#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::postgres::PgPool,
    pub redis_pool: RedisPool,
    pub oauth_client: BasicClient,
    pub uuid_seed: [u8; 6],
}
