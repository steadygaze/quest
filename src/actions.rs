// use uuid::Uuid;
use crate::models::*;

pub async fn get_user(
    pool: &sqlx::postgres::PgPool,
    username: &str,
) -> Result<Account, sqlx::Error> {
    sqlx::query_as(
        "select id, username, display_name, email, bio from account where username = $1 limit 1",
    )
    .bind(username)
    .fetch_one(pool)
    .await
}

pub async fn is_username_available<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres>>(
    db: E,
    username: &str,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("select 1 from account where username = $1 limit 1")
            .bind(username)
            .fetch_optional(db)
            .await?
            .is_some(),
    )
}

pub async fn create_user(
    pool: &sqlx::postgres::PgPool,
    username: &str,
    display_name: Option<&str>,
    email: &str,
) -> Result<bool, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    // Lock before checking username availability to prevent attempting to
    // insert two of the same username at once.  Not strictly required because
    // username is unique, but we will have better errors if we can detect duped
    // username.
    sqlx::query("lock table account in row exclusive mode")
        .execute(&mut *transaction)
        .await?;
    let existing_account = is_username_available(pool, username).await?;
    if existing_account {
        println!("existing account for {}", username);
        transaction.rollback().await?;
        return Ok(false);
    }
    sqlx::query("insert into account (username, display_name, email) values ($1, $2, $3)")
        .bind(username)
        .bind(display_name)
        .bind(email)
        .execute(pool)
        .await?;
    transaction.commit().await?;
    Ok(true)
}

pub async fn update_bio(
    pool: &sqlx::postgres::PgPool,
    username: &str,
    bio: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("update account set bio = $1 where username = $2")
        .bind(bio)
        .bind(username)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_recent_quests_preview(
    pool: &sqlx::postgres::PgPool,
    username: &str,
) -> Result<Vec<QuestInfo>, sqlx::Error> {
    Ok(sqlx::query_as("select title, slug, description, last_updated from quest where questmaster = $1 and visibility = 'public'::quest_visibility order by last_updated desc")
      .bind(username)
      .fetch_all(pool)
      .await?)
}
