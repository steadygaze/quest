use crate::routes::prelude::*;

pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope.service(edit_quest).service(edit_quest_submit)
}

#[derive(Template)]
#[template(path = "qm/edit.html")]
struct EditQuestTemplate<'a> {
    config: &'a AppConfig,
    logged_in: bool,
    current_profile: &'a Option<ProfileRenderInfo>,
    title: &'a String,
    slug: &'a String,
}

#[get("/edit/{slug}")]
async fn edit_quest(
    app_state: web::Data<AppState>,
    info: web::Path<(String,)>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let (slug,) = info.into_inner();
    let SessionInfo {
        account_id,
        current_profile,
        ..
    } = app_state.require_session(request).await?;

    let (title,): (String,) = sqlx::query_as(
        r#"
        select title
        from quest
        where questmaster = $1
        and slug = $2
        "#,
    )
    .bind(account_id)
    .bind(&slug)
    .fetch_one(&app_state.db_pool)
    .await
    .context("Failed to fetch quest")?;

    Ok(EditQuestTemplate {
        config: &app_state.config,
        logged_in: true,
        current_profile: &current_profile,
        title: &title,
        slug: &slug,
    }
    .to_response())
}

#[derive(Deserialize)]
struct NewQuestPostForm {
    title: String,
    body: String,
}

#[post("/edit/{slug}")]
async fn edit_quest_submit(
    app_state: web::Data<AppState>,
    info: web::Path<(String,)>,
    form: web::Form<NewQuestPostForm>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let (slug,) = info.into_inner();
    let SessionInfo {
        account_id,
        current_profile,
        ..
    } = app_state.require_session(request).await?;
    // TODO - Must be the QM of this quest.

    let mut transaction = app_state
        .db_pool
        .begin()
        .await
        .context("Failed to create transaction")?;

    let (quest_id,): (Uuid,) = sqlx::query_as(
        r#"
        select id
        from quest
        where questmaster = $1 and slug = $2
        "#,
    )
    .bind(account_id)
    .bind(&slug)
    .fetch_one(&mut *transaction)
    .await
    .context("Failed to fetch quest id")?;

    sqlx::query(
        r#"
        insert into quest_post (id, quest, title, body)
        values ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::now_v6(&app_state.uuid_seed))
    .bind(quest_id)
    .bind(&form.title)
    .bind(&form.body)
    .execute(&mut *transaction)
    .await
    .context("Failed to post update")?;

    transaction.commit().await.context("Failed to commit")?;

    Ok("update posted successfully")
}
