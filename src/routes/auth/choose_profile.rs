use fred::interfaces::HashesInterface;

use crate::key;
use crate::routes::auth;
use crate::routes::prelude::*;

pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(choose_profile_form)
        .service(choose_profile_submit)
}

#[derive(Template)]
#[template(path = "auth/choose_profile.html")]
pub struct ChooseProfileTemplate<'a> {
    pub config: &'a AppConfig,
    pub logged_in: bool,
    pub current_profile: &'a Option<ProfileRenderInfo>,
    pub profiles: &'a Vec<(String, String)>,
}

pub async fn get_profiles(
    db_pool: &sqlx::postgres::PgPool,
    account_id: Uuid,
) -> Result<Vec<(String, String)>> {
    Ok(sqlx::query_as(
        r#"
        select username, display_name
        from profile
        where account_id = $1
        order by display_name asc
        "#,
    )
    .bind(account_id)
    .fetch_all(db_pool)
    .await
    .context("Failed to get profiles")?)
}

#[get("/choose_profile")]
pub async fn choose_profile_form(
    app_state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let SessionInfo {
        account_id,
        current_profile,
        ..
    } = app_state.require_session(request).await?;

    let profiles: Vec<(String, String)> = get_profiles(&app_state.db_pool, account_id).await?;

    Ok(ChooseProfileTemplate {
        config: &app_state.config,
        logged_in: true,
        current_profile: &current_profile,
        profiles: &profiles,
    }
    .to_response())
}

#[derive(Debug, Deserialize)]
struct ChooseProfileForm {
    profile: String,
}

#[post("/choose_profile")]
pub async fn choose_profile_submit(
    app_state: web::Data<AppState>,
    form: web::Form<ChooseProfileForm>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let SessionInfo {
        account_id,
        current_profile,
        session_id,
        ..
    } = app_state.require_session(request).await?;

    if form.profile == "@" {
        if current_profile.is_some() {
            let _: () = app_state
                .redis_pool
                .hdel(
                    key::session(session_id.as_str()),
                    ("username", "display_name"),
                )
                .await
                .context("Failed to clear active profile")?;
        }
        return Ok(MessagePageTemplate {
            config: &app_state.config,
            logged_in: true,
            current_profile: &current_profile,
            page_title: &Some("Set profile"),
            message: "Cleared the active profile. Now in reader mode.",
        }
        .to_response());
    }

    if !auth::valid_username(&app_state.regex.alphanumeric, &form.profile) {
        return Err(Error::AppError("Bad username".to_string()));
    }

    match sqlx::query_as::<_, (String,)>(
        r#"
        select display_name
        from profile
        where account_id = $1 and username = $2
        limit 1
        "#,
    )
    .bind(&account_id)
    .bind(&form.profile)
    .fetch_optional(&app_state.db_pool)
    .await
    .context("Failed to check profile info")?
    {
        // E.g. if the user tries to log in as a profile they don't own.
        None => Err(Error::AppError("Bad username".to_string())),
        Some((display_name,)) => {
            let _: () = app_state
                .redis_pool
                .hset(
                    key::session(session_id.as_str()),
                    [
                        ("username", form.profile.as_str()),
                        ("display_name", display_name.as_str()),
                    ],
                )
                .await
                .context("Failed to set session profile info")?;
            Ok(MessagePageTemplate {
                config: &app_state.config,
                logged_in: true,
                current_profile: &current_profile,
                page_title: &Some("Set profile"),
                message: format!("Profile set to @{}.", form.profile).as_str(),
            }
            .to_response())
        }
    }
}
