use crate::routes::prelude::*;

pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope.service(view_quest)
}

/// Output object for quest list query.
#[derive(sqlx::FromRow, Debug, PartialEq, Eq)]
struct ListPost {
    title: String,
    body_html: String,
}

#[derive(Template)]
#[template(path = "quest/view.html")]
struct ViewQuestTemplate<'a> {
    config: &'a AppConfig,
    logged_in: bool,
    current_profile: &'a Option<ProfileRenderInfo>,
    title: &'a String,
    posts: &'a Vec<ListPost>,
}

#[get("/{slug}")]
async fn view_quest(
    app_state: web::Data<AppState>,
    info: web::Path<(String, String)>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let (username, slug) = info.into_inner();
    let session_info = app_state.get_session(request).await.transpose()?;

    // TODO - Handle user doesn't exist.
    // TODO - Handle quest doesn't exist.
    // TODO - Handle no permission to view.

    let posts: Vec<ListPost> = sqlx::query_as(
        r#"
        select quest_post.title, quest_post.body_html
        from quest
          join profile on questmaster = account_id
          join quest_post on quest.id = quest_post.quest
        where profile.username = $1
          and quest.slug = $2
        "#,
    )
    .bind(username)
    .bind(slug)
    .fetch_all(&app_state.db_pool)
    .await
    .context("Failed to fetch quests")?;

    Ok(ViewQuestTemplate {
        config: &app_state.config,
        logged_in: session_info.is_some(),
        current_profile: &session_info.and_then(|session_info| session_info.current_profile),
        title: &"Placeholder".to_string(),
        posts: &posts,
    }
    .to_response())
}
