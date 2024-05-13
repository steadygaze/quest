// use diesel::prelude::*;
use chrono;
use oauth2::basic::BasicClient;
use serde::{Deserialize, Serialize};
use uuid;

// #[derive(Queryable, Selectable, Identifiable, Debug)]
// #[diesel(table_name = crate::schema::account)]
// #[diesel(check_for_backend(diesel::pg::Pg))]
#[derive(Debug, Deserialize, Serialize, sqlx::FromRow)]
pub struct Account {
    pub id: uuid::Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub email: String,
    pub bio: Option<String>,
}

// #[derive(Queryable, Selectable, Identifiable, Associations, Debug)]
// #[diesel(table_name = crate::schema::quest)]
// #[diesel(check_for_backend(diesel::pg::Pg))]
// #[diesel(belongs_to(Account, foreign_key = questmaster))]
#[derive(Debug)]
pub struct Quest {
    pub id: uuid::Uuid,
    pub questmaster: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::offset::Utc>,
    pub unlisted: bool, // TODO: visibility
                        // state quest_state default 'preparing'
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct QuestInfo {
    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub last_updated: chrono::DateTime<chrono::offset::Utc>,
}

#[derive(Clone)]
pub struct AppState {
    pub oauth_client: BasicClient,
    pub pool: sqlx::postgres::PgPool,
    pub uuid_seed: [u8; 6],
}
