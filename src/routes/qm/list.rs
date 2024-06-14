use std::vec::Vec;

use crate::routes::prelude::*;

pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope.service(list_quests)
}

/// Output object for quest list query.
#[derive(sqlx::FromRow, Debug, PartialEq, Eq)]
struct ListQuest {
    title: String,
    slug: String,
}

#[derive(Template)]
#[template(path = "qm/list.html")]
struct ListQuestTemplate<'a> {
    config: &'a AppConfig,
    current_profile: &'a Option<ProfileRenderInfo>,
    quests: &'a Vec<ListQuest>,
}

#[get("/")]
async fn list_quests(
    app_state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let SessionInfo {
        account_id,
        current_profile,
        ..
    } = app_state.require_session(request).await?;
    // TODO - Must be QM to view this page.

    let quests: Vec<ListQuest> = sqlx::query_as(
        r#"
        select title, slug
        from quest
        where questmaster = $1
        "#,
    )
    .bind(account_id)
    .fetch_all(&app_state.db_pool)
    .await
    .context("Failed to fetch quests")?;

    Ok(ListQuestTemplate {
        config: &app_state.config,
        current_profile: &current_profile,
        quests: &quests,
    }
    .to_response())
}
