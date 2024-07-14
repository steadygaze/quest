use crate::routes::prelude::*;

pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope.service(view).service(update)
}

#[derive(Template)]
#[template(path = "settings/view.html")]
struct SettingsTemplate<'a> {
    config: &'a AppConfig,
    current_profile: &'a Option<ProfileRenderInfo>,
    logged_in: bool,
    settings: &'a Settings,
    profiles: &'a Vec<(String, String)>,
}

/// Output object for settings.
#[derive(sqlx::FromRow, Debug, PartialEq, Eq)]
struct Settings {
    ask_for_profile_on_login: bool,
    default_profile_username: Option<String>,
}

#[get("/")]
async fn view(app_state: web::Data<AppState>, request: HttpRequest) -> Result<impl Responder> {
    view_helper(app_state, request).await
}

async fn view_helper(
    app_state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let SessionInfo {
        account_id,
        current_profile,
        ..
    } = app_state.require_session(request).await?;

    let (settings, profiles): (Settings, Vec<(String, String)>) = try_join!(
        sqlx::query_as(
            r#"
            select
              ask_for_profile_on_login,
              profile.username as default_profile_username
            from
              account
              left join profile on account.default_profile = profile.id
            where
              account.id = $1
            "#,
        )
        .bind(account_id)
        .fetch_one(&app_state.db_pool),
        sqlx::query_as(
            r#"
            select username, display_name
            from profile
            where account_id = $1
            "#,
        )
        .bind(account_id)
        .fetch_all(&app_state.db_pool)
    )
    .context("Failed to fetch account settings")?;

    Ok(SettingsTemplate {
        config: &app_state.config,
        current_profile: &current_profile,
        logged_in: true,
        settings: &settings,
        profiles: &profiles,
    }
    .to_response())
}

#[derive(Debug, Deserialize)]
struct SettingsForm {
    default_profile: String,
}

#[post("/")]
async fn update(
    app_state: web::Data<AppState>,
    request: HttpRequest,
    form: web::Form<SettingsForm>,
) -> Result<impl Responder> {
    // TODO - Implement update.

    view_helper(app_state, request).await
}
