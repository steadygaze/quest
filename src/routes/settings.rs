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
    let session_info = app_state.require_session(request).await?;
    view_fn(app_state, session_info).await
}

async fn view_fn(
    app_state: web::Data<AppState>,
    session_info: SessionInfo,
) -> Result<impl Responder> {
    let SessionInfo {
        account_id,
        current_profile,
        ..
    } = session_info;

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
    let session_info = app_state.require_session(request).await?;

    let default_profile = form.default_profile.as_str();
    match default_profile {
        "@ask" | "@reader" => {
            if sqlx::query(
                r#"
                update account
                set ask_for_profile_on_login = $1, default_profile = null
                where id = $2
                "#,
            )
            .bind(default_profile == "@ask")
            .bind(session_info.account_id)
            .execute(&app_state.db_pool)
            .await
            .context("Failed to set profile default")?
            .rows_affected()
                <= 0
            {
                return Err(sqlx::Error::RowNotFound)
                    .context("Failed to find account to update")?;
            }
        }
        _ => {
            if !validation::username(default_profile) {
                return Err(Error::AppError(format!(
                    "Bad username \"{}\"",
                    default_profile
                )));
            }
            if sqlx::query(
                r#"
                update account
                set ask_for_profile_on_login = false, default_profile = (
                  select id from profile where username = $1 limit 1
                )
                where id = $2
                "#,
            )
            .bind(default_profile)
            .bind(session_info.account_id)
            .execute(&app_state.db_pool)
            .await
            .context("Failed to set profile default")?
            .rows_affected()
                <= 0
            {
                return Err(sqlx::Error::RowNotFound)
                    .context("Failed to find account to update")?;
            }
        }
    }

    view_fn(app_state, session_info).await
}
