mod choose_profile;

use actix_web::cookie;
use actix_web::cookie::Cookie;
use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;
use askama_actix::TemplateToResponse;
use awc::Client;
use fred::clients::RedisPool;
use fred::error::RedisError;
use fred::interfaces::HashesInterface;
use fred::interfaces::KeysInterface;
use fred::interfaces::TransactionInterface;
use fred::prelude::RedisValue;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope,
    StandardRevocableToken, TokenResponse,
};
use rand::distributions::{Alphanumeric, DistString};
use regex::Regex;
use sqlx::Executor;
use sqlx::Row;
use std::collections::HashMap;

use crate::app_state::{CompiledRegexes, SESSION_ID_COOKIE};
use crate::key;
use crate::partials;

use crate::routes::prelude::*;

const OAUTH_EXPIRATION_SEC: i64 = 60 * 10;
const ACCOUNT_CREATION_TIMEOUT_SEC: i64 = 60 * 60;
const SESSION_TTL_DAYS: i64 = 30; // 30 days
const SESSION_TTL_SEC: i64 = SESSION_TTL_DAYS * 24 * 60 * 60; // 30 days

/// Add auth-related routes.
pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    let scope = choose_profile::add_routes(scope);
    scope
        .service(login_options)
        .service(discord_start)
        .service(discord_callback)
        .service(create_account)
        .service(test)
        .service(check_if_user_already_exists)
        .service(cancel_create_account)
        .service(logout)
}

/// Temporary endpoint for testing the auth page template.
#[get("/test")]
async fn test(app_state: web::Data<AppState>) -> impl Responder {
    CreateAccountTemplate {
        config: &app_state.config,
        logged_in: false,
        current_profile: &None,
        email: "test@test.com",
        secret: "mysecret",
    }
    .to_response()
}

#[derive(Template)]
#[template(path = "auth/login.html")]
struct LoginTemplate<'a> {
    config: &'a AppConfig,
    logged_in: bool,
    current_profile: &'a Option<ProfileRenderInfo>,
}

/// Login options page to present different oauth providers.
#[get("/")]
async fn login_options(app_state: web::Data<AppState>) -> Result<impl Responder> {
    Ok(LoginTemplate {
        config: &app_state.config,
        logged_in: false,
        current_profile: &None,
    }
    .to_response())
}

/// Start Discord oauth by generating a PKCE challenge and redirecting.
#[get("/discord/start")]
async fn discord_start(app_state: web::Data<AppState>) -> Result<impl Responder> {
    // Generate a PKCE challenge.
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate the full authorization URL.
    let (auth_url, csrf_token) = app_state
        .oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("identify".to_string()))
        // Required to read the user's email from Discord.
        .add_scope(Scope::new("email".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    app_state
        .redis_pool
        .set::<String, _, _>(
            key::oauth_secret(csrf_token.secret()),
            pkce_verifier.secret(),
            Some(fred::prelude::Expiration::EX(OAUTH_EXPIRATION_SEC)),
            None,
            false,
        )
        .await
        .context("Failed to store oauth challenge")?;
    trace!("Stored CSRF challenge successfully");

    Ok(web::Redirect::to(<oauth2::url::Url as Into<String>>::into(auth_url)).see_other())
}

/// URL params expected when Discord redirects back after oauth.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DiscordOauthRedirectParams {
    DiscordOauthOk {
        code: String,
        state: String,
    },
    /// Usually when the user rejected/cancelled login.>
    DiscordApiError {
        error: String,
        error_description: String,
        state: String,
    },
}

#[derive(Deserialize)]
struct DiscordUser {
    email: String,
}

#[derive(Template)]
#[template(path = "auth/create_account.html")]
struct CreateAccountTemplate<'a> {
    config: &'a AppConfig,
    logged_in: bool,
    current_profile: &'a Option<ProfileRenderInfo>,
    email: &'a str,
    secret: &'a str,
}

#[get("/discord/callback")]
async fn discord_callback(
    app_state: web::Data<AppState>,
    params: web::Query<DiscordOauthRedirectParams>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let params = params.into_inner();
    use DiscordOauthRedirectParams as Params;

    // Guard against Redis key injection.
    let state = match &params {
        Params::DiscordOauthOk { state, .. } => state,
        Params::DiscordApiError { state, .. } => state,
    };
    if state.len() > 64 || !app_state.regex.oauth_state_ok.is_match(state.as_str()) {
        return Err(Error::AppError("Bad oauth state token".to_string()));
    }

    let (oauth_code, oauth_state) = match params {
        Params::DiscordOauthOk { code, state } => (code, state),
        Params::DiscordApiError {
            error,
            error_description,
            state,
        } => {
            // We don't care if the query succeeds because there is a main error already.
            tokio::spawn(async move {
                match app_state
                    .redis_pool
                    .del::<i64, _>(key::oauth_secret(&state))
                    .await
                {
                    Ok(keys_deleted) => {
                        if keys_deleted <= 0 {
                            trace!(
                                "Ignored missing oauth entry when cleaning up; it likely expired"
                            );
                        }
                    }
                    Err(err) => {
                        warn!(
                            "Ignored Redis error when cleaning up pending oauth entry: {}",
                            err
                        );
                    }
                }
            });

            if error == "access_denied" {
                return Err(Error::AuthorizationError("Access denied by Discord. Try again, but accept the prompt to grant permissions.".to_string()));
            } else {
                return Err(Error::InternalError(anyhow::anyhow!(
                    "Unknown error response from Discord API: error: {}, error_description: {}",
                    error,
                    error_description
                )));
            }
        }
    };

    let pkce_verifier = match app_state
        .redis_pool
        .getdel::<Option<String>, _>(key::oauth_secret(&oauth_state))
        .await
        .context("Failed to retrieve pending oauth record")?
    {
        None => {
            return Err(Error::AppError(
                "Couldn't find pending oauth request".to_string(),
            ));
        }
        Some(verifier) => verifier,
    };

    // Now you can trade it for an access token.
    let token_response = app_state
        .oauth_client
        .exchange_code(AuthorizationCode::new(oauth_code))
        // Set the PKCE code verifier.
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        .request_async(async_http_client)
        .await
        .context("Failed to retrieve oauth token")?;

    let client = Client::default();
    let discord_response_result = client
        .get("https://discord.com/api/v10/users/@me")
        .insert_header(("Accept", "application/json"))
        .insert_header((
            "Authorization",
            format!("Bearer {}", token_response.access_token().secret()),
        ))
        .insert_header(("User-Agent", "awc/3.4"))
        .send()
        .await;

    // We have to set up revoking the token regardless of the result, which
    // requires this intervening block here.
    {
        trace!("About to revoke the token");
        let oauth_client = app_state.oauth_client.clone();
        // We don't care if revoking the token fails, because it's not on the
        // critical path, so we create a new thread.
        tokio::spawn(async move {
            let token_to_revoke: StandardRevocableToken = match token_response.refresh_token() {
                Some(token) => token.into(),
                None => token_response.access_token().into(),
            };
            match oauth_client.revoke_token(token_to_revoke) {
                Err(err) => {
                    warn!("Ignored configuration error revoking Discord token: {err}");
                }
                Ok(revocation_request) => {
                    if let Err(err) = revocation_request.request_async(async_http_client).await {
                        warn!("Ignored error revoking Discord token: {err}")
                    } else {
                        trace!("Token revocation successful");
                    }
                }
            };
        });
    }

    let discord_email = discord_response_result
        // There is probably some information loss here, but I'm not sure how to
        // get anyhow to accept a SendRequestError.
        .map_err(|e| anyhow::anyhow!(e.to_string()))
        .context("Failed to check user email with Discord API")?
        .json::<DiscordUser>()
        .await
        .context("Failed to decode response from Discord API")?
        .email;

    match sqlx::query_as::<_, (Uuid, bool, Option<String>, Option<String>)>(
        r#"
        select
          account.id,
          ask_for_profile_on_login,
          profile.username,
          profile.display_name
        from
          account
          left join profile on account.default_profile = profile.id
        where
          email = $1
          or $1 = any(secondary_email)
        limit 1
        "#,
    )
    .bind(&discord_email)
    .fetch_optional(&app_state.db_pool)
    .await
    .context("Failed to check if user's account exists")?
    {
        Some((account_id, ask_for_profile_on_login, username, display_name)) => {
            let previous_session = request.cookie(SESSION_ID_COOKIE);
            if previous_session.is_some() {
                trace!("Clearing a previous session on new login");
            }
            let profile = username.zip(display_name);
            // Regular login for existing user.
            let cookie = create_session(
                &app_state.redis_pool,
                account_id,
                previous_session,
                profile.clone(),
            )
            .await
            .context("Failed to record new session")?;

            let all_profiles: Vec<(String, String)> =
                choose_profile::get_profiles(&app_state.db_pool, account_id).await?;
            let mut response = if ask_for_profile_on_login {
                choose_profile::ChooseProfileTemplate {
                    config: &app_state.config,
                    logged_in: true,
                    current_profile: &profile.map(|(username, display_name)| ProfileRenderInfo {
                        username,
                        display_name,
                    }),
                    profiles: &all_profiles,
                }
                .to_response()
            } else {
                partials::MessagePageTemplate {
                    config: &app_state.config,
                    logged_in: true,
                    current_profile: &profile.map(|(username, display_name)| ProfileRenderInfo {
                        username,
                        display_name,
                    }),
                    page_title: &Some("Logged in"),
                    message: format!("You are now logged in as {discord_email}.").as_str(),
                }
                .to_response()
            };
            response
                .add_cookie(&cookie)
                .context("Error setting session cookie on request")?;
            return Ok(response);
        }
        None => (), // Success, but now we must create an account.
    }

    let mut new_account_secret = String::new();
    // Store an account secret to be passed back, indicating there is a pending
    // account creation. This is a loop because there is an extremely small
    // chance of accidentally generating a duplicate account secret.
    for i in 0..100 {
        new_account_secret = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
        match app_state
            .redis_pool
            .set::<Option<String>, _, _>(
                key::new_account_secret(&new_account_secret),
                &discord_email,
                Some(fred::types::Expiration::EX(ACCOUNT_CREATION_TIMEOUT_SEC)),
                Some(fred::types::SetOptions::NX), // Don't override existing secret.
                true,
            )
            .await
            .context("Failed to record in-progress account creation")?
        {
            Some(_existing_value) => {
                info!("Generated random duplicate account creation secret! This should never happen (1 in 62^32 chance); you are extremely \"lucky\". :)");
            }
            None => {
                break;
            } // Success.
        };
        if i >= 99 {
            // Failsafe in case there is a flaw in random number generation.
            return Err(Error::AppError(
                "Error generating account secret!".to_string(),
            ));
        }
    }

    Ok(CreateAccountTemplate {
        config: &app_state.config,
        logged_in: false,
        current_profile: &None,
        email: discord_email.as_str(),
        secret: new_account_secret.as_str(),
    }
    .to_response())
}
// TODO - Add session cookie.

/// URL params for account creation form.
#[derive(Debug, Deserialize)]
struct RegisterUserForm {
    secret: String,
    #[serde(rename = "create-profile")]
    create_profile: Option<String>, // "on" or "off"
    username: Option<String>,
    #[serde(rename = "display-name")]
    display_name: Option<String>, // "on" or "off"
    bio: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsernameExistsQuery {
    username: String,
}

#[get("/profile_exists_already")]
async fn check_if_user_already_exists(
    app_state: web::Data<AppState>,
    params: web::Query<UsernameExistsQuery>,
) -> impl Responder {
    let username = params.into_inner().username;
    match sqlx::query_as(
        r#"
        select exists(
          select 1
          from profile
          where username = $1
          limit 1
        )
        "#,
    )
    .bind(&username)
    .fetch_one(&app_state.db_pool)
    .await
    {
        Ok((true,)) => partials::FailureTemplate {
            text: format!("@{username} is already taken").as_str(),
        }
        .to_response(),
        Ok((false,)) => partials::SuccessTemplate {
            text: format!("@{username} is available").as_str(),
        }
        .to_response(),
        Err(_) => HttpResponse::InternalServerError().body("database error"),
    }
}

/// Initialize a new session in Redis.
async fn create_session<'a>(
    redis_pool: &'a RedisPool,
    account_id: Uuid,
    previous_session: Option<Cookie<'_>>,
    profile: Option<(String, String)>,
) -> std::result::Result<Cookie<'a>, RedisError> {
    let session_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let transaction = redis_pool.multi();
    if let Some(previous_session) = previous_session {
        trace!("Also cleaning up previous session as part of login");
        let _ = transaction.del::<String, _>(key::session(previous_session.name()));
    }
    let _ = transaction
        .hset::<i64, _, _>(
            key::session(&session_id),
            if let Some((username, display_name)) = profile {
                HashMap::from([
                    ("account_id", account_id.simple().to_string()),
                    ("username", username),
                    ("display_name", display_name),
                ])
            } else {
                HashMap::from([("account_id", account_id.simple().to_string())])
            },
        )
        .await;
    let _ = transaction
        .expire::<i64, _>(key::session(&session_id), SESSION_TTL_SEC)
        .await;
    transaction.exec::<(RedisValue, RedisValue)>(true).await?;

    let mut cookie_builder = Cookie::build(SESSION_ID_COOKIE, session_id)
        .path("/")
        .http_only(true)
        .max_age(cookie::time::Duration::seconds(SESSION_TTL_SEC))
        // Must be lax to be sent with login redirect and to be logged in when navigating from externally linked pages.
        .same_site(cookie::SameSite::Lax);
    if !cfg!(debug_assertions) {
        // In production, we will likely use a reverse proxy like Nginx or
        // Cloudflare to implement SSL. Otherwise we would have to set up
        // self-signed certificates in a dev environment, which would be a pain.
        cookie_builder = cookie_builder.secure(true);
    }
    return Ok(cookie_builder.finish());
}

#[post("/create_account")]
async fn create_account(
    app_state: web::Data<AppState>,
    form: web::Form<RegisterUserForm>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let profile = if form.create_profile.as_ref().is_some_and(|x| x == "on") {
        let username = form
            .username
            .clone()
            .context("Expected username when creating a profile")?;
        validation::username(&username)?;
        // TODO - Additional validation of display_name, etc.
        let display_name = form
            .display_name
            .clone()
            .context("Expected display_name when creating a profile")?;
        Some((username, display_name))
    } else {
        None
    };

    let email = match app_state
        .redis_pool
        .getdel::<Option<String>, _>(key::new_account_secret(&form.secret))
        .await
        .context("Failed to get pending account creation state")?
    {
        Some(value) => value,
        None => {
            return Err(Error::AppError(
                "No record of pending account creation. It could've expired.".to_string(),
            ));
        }
    };

    // Create a transaction for both creating the account and the profile.
    let mut transaction = app_state
        .db_pool
        .begin()
        .await
        .context("Failed to create transaction for account creation")?;

    let id: Uuid = transaction
        .fetch_one(
            sqlx::query(
                r#"
                insert into account (email)
                values ($1)
                returning id
                "#,
            )
            .bind(&email),
        )
        .await
        .context("Failed to create new account")?
        .get(0);

    if form.create_profile.as_ref().is_some_and(|x| x == "on") {
        let profile_id: Uuid = transaction
            .fetch_one(
                sqlx::query(
                    r#"
                    insert into profile (username, account_id, display_name, bio)
                    values ($1, $2, $3, $4)
                    returning id
                    "#,
                )
                .bind(&form.username)
                .bind(id)
                .bind(&form.display_name)
                .bind(&form.bio),
            )
            .await
            .context("Failed to create profile")?
            .get(0);

        if transaction
            .execute(
                sqlx::query(
                    r#"
                    update account
                    set default_profile = $1
                    where id = $2
                    "#,
                )
                .bind(profile_id)
                .bind(id),
            )
            .await
            .context("Failed to set profile default")?
            .rows_affected()
            <= 0
        {
            return Err(sqlx::Error::RowNotFound).context("Failed to find account to update")?;
        }
    }

    transaction
        .commit()
        .await
        .context("Failed to commit account creation")?;

    let cookie = create_session(
        &app_state.redis_pool,
        id,
        request.cookie(SESSION_ID_COOKIE),
        profile.clone(),
    )
    .await
    .context("Failed to create new session after account creation")?;
    let profile_render_info = profile.map(|(username, display_name)| ProfileRenderInfo {
        username,
        display_name,
    });
    let mut response = partials::MessagePageTemplate {
        config: &app_state.config,
        logged_in: true,
        current_profile: &profile_render_info,
        page_title: &Some("Logged in"),
        message: "Account created successfully. You are now logged in.",
    }
    .to_response();
    response
        .add_cookie(&cookie)
        .context("Couldn't set session cookie")?;
    Ok(response)
}

#[post("/cancel_create_account")]
async fn cancel_create_account(
    app_state: web::Data<AppState>,
    form: web::Form<RegisterUserForm>,
) -> Result<impl Responder> {
    // We don't care if we didn't remove anything; the key could've expired.
    let _rows_deleted = app_state
        .redis_pool
        .del::<String, _>(key::new_account_secret(&form.secret))
        .await
        .context("Failed to clean up account creation secret")?;

    Ok(partials::MessagePageTemplate {
        config: &app_state.config,
        logged_in: true,
        current_profile: &None,
        page_title: &Some("Cancelled"),
        message: "Account creation cancelled; your information has been forgotten. If you want to create a new account, start over.",
    }
    .to_response())
}

#[get("/logout")]
async fn logout(app_state: web::Data<AppState>, request: HttpRequest) -> Result<impl Responder> {
    if let Some(mut session_id) = request.cookie(SESSION_ID_COOKIE) {
        trace!("Logging out session {:?}", session_id);
        app_state
            .redis_pool
            .del::<String, _>(key::session(&session_id.value()))
            .await
            .context("Failed to clear session")?;

        let mut response = partials::MessagePageTemplate {
            config: &app_state.config,
            logged_in: false,
            current_profile: &None,
            page_title: &Some("Logged out"),
            message: "You are now logged out. Goodbye.",
        }
        .to_response();

        // We must re-set some attributes that aren't transmitted with the
        // cookie in the request, otherwise removal won't work.
        session_id.set_path("/");
        response
            .add_removal_cookie(&session_id)
            .context("Failed to set removal cookie")?;
        Ok(response)
    } else {
        Ok(partials::MessagePageTemplate {
            config: &app_state.config,
            logged_in: false,
            current_profile: &None,
            page_title: &Some("Logged out"),
            message: "You were already logged out.",
        }
        .to_response())
    }
}
