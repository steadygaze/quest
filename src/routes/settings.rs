use crate::routes::prelude::*;

pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope.service(view).service(update)
}

#[derive(sqlx::FromRow, Debug, PartialEq, Eq)]
struct Profile {
    username: String,
    display_name: String,
    bio: String,
}

#[derive(Template)]
#[template(path = "settings/view.html")]
struct SettingsTemplate<'a> {
    config: &'a AppConfig,
    current_profile: &'a Option<ProfileRenderInfo>,
    logged_in: bool,
    settings: &'a Settings,
    profiles: &'a Vec<Profile>,
    messages: &'a Vec<String>,
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
    view_fn(app_state, session_info, &Vec::new()).await
}

async fn view_fn(
    app_state: web::Data<AppState>,
    session_info: SessionInfo,
    messages: &Vec<String>,
) -> Result<impl Responder> {
    let SessionInfo {
        account_id,
        current_profile,
        ..
    } = session_info;

    let (settings, profiles): (Settings, Vec<Profile>) = try_join!(
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
            select username, display_name, bio
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
        messages: &messages,
    }
    .to_response())
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum SettingsForm {
    /// Account change.
    Account { default_profile: String },
    /// Profile details change.
    ProfileDetails {
        username: String,
        display_name: String,
        bio: String,
    },
    /// Profile username change.
    ProfileUsername {
        original_username: String,
        username: String,
    },
    /// Create a new profile.
    NewProfile {
        username: String,
        display_name: String,
        bio: String,
    },
}

#[post("/")]
async fn update(
    app_state: web::Data<AppState>,
    request: HttpRequest,
    form: web::Form<SettingsForm>,
) -> Result<impl Responder> {
    let session_info = app_state.require_session(request).await?;

    let mut messages = Vec::new();
    match form.into_inner() {
        SettingsForm::Account { default_profile } => {
            let default_profile = default_profile.as_str();
            match default_profile {
                "__ask" | "__reader" => {
                    if sqlx::query(
                        r#"
                        update account
                        set ask_for_profile_on_login = $1, default_profile = null
                        where id = $2
                        "#,
                    )
                    .bind(default_profile == "__ask")
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

                    messages.push(format!(
                        "Set default profile to {}",
                        if default_profile == "__ask" {
                            "\"Ask me every time\""
                        } else {
                            "no profile (reader mode)"
                        }
                    ));
                }
                _ => {
                    validation::username(default_profile)?;
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
                            .context("Failed to find profile to update")?;
                    }
                    messages.push(format!("Set default profile to @{}", default_profile));
                }
            }
        }
        SettingsForm::ProfileDetails {
            username,
            display_name,
            bio,
        } => {
            trace!("Bio: {}", bio);
            // TODO: verify ownership before update.
            validation::username(username.as_str())?;
            if sqlx::query(
                r#"
                update profile
                set display_name = $1, bio = $2
                where username = $3
                "#,
            )
            .bind(display_name)
            .bind(bio)
            .bind(&username)
            .execute(&app_state.db_pool)
            .await
            .context("Failed to set profile default")?
            .rows_affected()
                <= 0
            {
                return Err(sqlx::Error::RowNotFound)
                    .context("Failed to find account to update")?;
            }
            messages.push(format!("Updated display name and/or bio for @{}", username));
        }
        SettingsForm::ProfileUsername {
            original_username,
            username,
        } => {
            // TODO: verify ownership before update.
            validation::username(username.as_str())?;
            validation::username(original_username.as_str())?;
            if sqlx::query(
                r#"
                update profile
                set username = $1
                where username = $2
                "#,
            )
            .bind(&username)
            .bind(&original_username)
            .bind(session_info.account_id)
            .execute(&app_state.db_pool)
            .await
            .context("Failed to set username")?
            .rows_affected()
                <= 0
            {
                return Err(sqlx::Error::RowNotFound)
                    .context("Failed to find profile to update")?;
            }
            messages.push(format!(
                "Changed username from @{} to @{}",
                original_username, username
            ));
        }
        SettingsForm::NewProfile {
            username,
            display_name,
            bio,
        } => {
            validation::username(username.as_str())?;
            let (profile_count,): (i64,) = sqlx::query_as(
                r#"
                select count(*)
                from profile
                where account_id = $1
                "#,
            )
            .bind(session_info.account_id)
            .fetch_one(&app_state.db_pool)
            .await
            .context("Failed to check profile count")?;

            if profile_count >= 5 {
                return Err(Error::AppError(
                    "Too many existing profiles to create a new one".to_string(),
                ));
            }

            sqlx::query(
                r#"
                insert into profile (username, account_id, display_name, bio)
                values ($1, $2, $3, $4)
                returning id
                "#,
            )
            .bind(&username)
            .bind(session_info.account_id)
            .bind(display_name)
            .bind(bio)
            .execute(&app_state.db_pool)
            .await
            .context("Failed to create profile")?;

            messages.push(format!(
                "Created new profile @{} (visit \"Change profile\" to use it)",
                username
            ));
        }
    }

    view_fn(app_state, session_info, &messages).await
}
