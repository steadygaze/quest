use oauth2::basic::BasicClient;

/// Actix state object for dependency injection.
#[derive(Clone)]
pub struct AppState {
    pub oauth_client: BasicClient,
    pub pool: sqlx::postgres::PgPool,
    pub uuid_seed: [u8; 6],
}
