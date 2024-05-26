// use actix_web::dev::HttpServiceFactory;
use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;
use actix_web::web::Either;
use actix_web::HttpRequest;
use actix_web::{cookie, get, post, web, HttpResponse, Responder};
use askama::Template;
use askama_actix::TemplateToResponse;
use awc::cookie::Cookie;
use awc::Client;
use fred::clients::RedisPool;
use fred::error::RedisError;
use fred::interfaces::HashesInterface;
use fred::interfaces::KeysInterface;
use fred::interfaces::TransactionInterface;
use fred::prelude::RedisValue;
use log::{error, trace, warn};
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope,
    StandardRevocableToken, TokenResponse,
};
use rand::distributions::{Alphanumeric, DistString};
use serde::Deserialize;
use sqlx::Executor;
use sqlx::Row;
use std::collections::HashMap;
use std::thread;
use uuid::Uuid;

use crate::app_state::CompiledRegex;
use crate::app_state::{AppConfig, AppState};
use crate::key;
use crate::partials;

const OAUTH_EXPIRATION_SEC: i64 = 60 * 10;
const ACCOUNT_CREATION_TIMEOUT_SEC: i64 = 60 * 30;
const SESSION_TTL_DAYS: i64 = 30; // 30 days
const SESSION_TTL_SEC: i64 = SESSION_TTL_DAYS * 24 * 60 * 60; // 30 days

const SESSION_ID_COOKIE: &str = "sid";

/// Add oauth-related routes.
pub fn add_routes<T>(app: actix_web::App<T>) -> actix_web::App<T>
where
    T: ServiceFactory<ServiceRequest, Config = (), Error = actix_web::Error, InitError = ()>,
{
    app.service(discord_start)
        .service(discord_callback)
        .service(create_account)
        .service(test)
        .service(check_if_user_already_exists)
        .service(cancel_create_account)
        .service(logout)
}

#[get("/auth/test")]
pub async fn test(app_state: web::Data<AppState>) -> impl Responder {
    CreateAccountTemplate {
        config: &app_state.config,
        email: "test@test.com",
        secret: "mysecret",
    }
    .to_response()
}

/// Start Discord oauth by generating a PKCE challenge and redirecting.
#[get("/auth/discord/start")]
pub async fn discord_start(app_state: web::Data<AppState>) -> impl Responder {
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

    match app_state
        .redis_pool
        .set::<String, _, _>(
            key::oauth_secret(csrf_token.secret()),
            pkce_verifier.secret(),
            Some(fred::prelude::Expiration::EX(OAUTH_EXPIRATION_SEC)),
            None,
            false,
        )
        .await
    {
        Ok(_) => {
            trace!("Stored CSRF challenge successfully");
        }
        Err(err) => {
            return Either::Left(HttpResponse::InternalServerError().body(format!(
                "Tried to store oauth challenge but got error: {}",
                err
            )));
        }
    };

    Either::Right(web::Redirect::to(<oauth2::url::Url as Into<String>>::into(auth_url)).see_other())
}

/// URL params expected when Discord redirects back after oauth.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DiscordOauthRedirectParams {
    DiscordOauthOk {
        code: String,
        state: String,
    },
    /// Usually when the user rejected/cancelled login.
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
#[template(path = "create_account.html")]
struct CreateAccountTemplate<'a> {
    config: &'a AppConfig,
    email: &'a str,
    secret: &'a str,
}

#[get("/auth/discord/callback")]
pub async fn discord_callback(
    app_state: web::Data<AppState>,
    params: web::Query<DiscordOauthRedirectParams>,
    request: HttpRequest,
) -> impl Responder {
    trace!("Got oauth params: {:?}", params);
    let params = params.into_inner();

    // Guard against Redis key injection.
    let state = match &params {
        DiscordOauthRedirectParams::DiscordOauthOk { state, .. } => state,
        DiscordOauthRedirectParams::DiscordApiError { state, .. } => state,
    };
    if state.len() > 64 || !app_state.regex.oauth_state_ok.is_match(state.as_str()) {
        return HttpResponse::BadRequest().body("Bad oauth state token");
    }

    let (oauth_code, oauth_state) = match params {
        DiscordOauthRedirectParams::DiscordOauthOk { code, state } => (code, state),
        DiscordOauthRedirectParams::DiscordApiError {
            error,
            error_description,
            state,
        } => {
            // We don't care if the query succeeds because there is a main error already.
            thread::spawn(|| async move {
                let state = state.clone();
                let pool = app_state.db_pool.clone();

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
                return HttpResponse::Unauthorized()
                    .body("Access denied by Discord. Try again, but accept the prompt to grant permissions.");
            } else {
                return HttpResponse::InternalServerError().body(format!(
                    "Unknown error response from Discord API: error: {}, error_description: {}",
                    error, error_description
                ));
            }
        }
    };

    let result = app_state
        .redis_pool
        .getdel::<Option<String>, _>(key::oauth_secret(&oauth_state))
        .await;

    let pkce_verifier = match result {
        Ok(None) => {
            return HttpResponse::BadRequest().body("Couldn't find pending oauth request");
        }
        Err(err) => {
            return HttpResponse::InternalServerError().body(format!(
                "Redis error while retrieving oauth record: {}",
                err
            ));
        }
        Ok(Some(verifier)) => verifier,
    };
    // Invariant params.state == csrf_token already checked with Redis.

    // Now you can trade it for an access token.
    let token_response = match app_state
        .oauth_client
        .exchange_code(AuthorizationCode::new(oauth_code))
        // Set the PKCE code verifier.
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        .request_async(async_http_client)
        .await
    {
        Ok(resp) => resp,
        Err(err) => {
            return HttpResponse::InternalServerError()
                .body(format!("Error while retrieving oauth token: {}", err));
        }
    };

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
        let oauth_client = app_state.oauth_client.clone();
        // Token revocation is not async for some reason, and also we don't care
        // if revoking the token fails, because it's not on the critical path,
        // so we create a new thread.
        thread::spawn(move || {
            let token_to_revoke: StandardRevocableToken = match token_response.refresh_token() {
                Some(token) => token.into(),
                None => token_response.access_token().into(),
            };
            if let Some(err) = oauth_client.revoke_token(token_to_revoke).err() {
                warn!("Ignored error revoking Discord token: {}", err);
            } else {
                trace!("Token revocation successful");
            }
        });
    }

    let discord_email = match discord_response_result {
        Ok(mut response) => match response.json::<DiscordUser>().await {
            Ok(user) => user,
            Err(err) => {
                return HttpResponse::InternalServerError()
                    .body(format!("Error decoding response from Discord API: {}", err));
            }
        },
        Err(err) => {
            return HttpResponse::InternalServerError()
                .body(format!("Error getting email from Discord API: {}", err));
        }
    }
    .email;

    match sqlx::query_as::<_, (Uuid,)>(
        r#"
        select id
        from account
        where email = $1
        limit 1
        "#,
    )
    .bind(&discord_email)
    .fetch_optional(&app_state.db_pool)
    .await
    {
        Ok(Some((user_id,))) => {
            let previous_session = request.cookie(SESSION_ID_COOKIE);
            if previous_session.is_some() {
                trace!("Clearing a previous session on new login");
            }
            // Regular login for existing user.
            return match create_session(
                &app_state.redis_pool,
                user_id.to_string(),
                previous_session,
            )
            .await
            {
                Ok(cookie) => {
                    let mut response =
                        HttpResponse::Ok().body(format!("You are logged in as: {}", discord_email));
                    if let Err(err) = response.add_cookie(&cookie) {
                        error!("Error setting session cookie: {err}");
                    }
                    response
                }
                Err(err) => {
                    error!("Error creating session: {err}");
                    return HttpResponse::InternalServerError()
                        .body("Error logging in. Try again?");
                }
            };
        }
        Err(err) => {
            return HttpResponse::InternalServerError().body(format!(
                "Error checking account status for {}: {}",
                discord_email, err
            ));
        }
        Ok(None) => (), // Success, but now we must create an account.
    }

    // Store an account secret to be passed back, indicating there is a pending
    // account creation.
    let new_account_secret = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    match app_state
        .redis_pool
        .set::<Option<String>, _, _>(
            key::new_account_secret(&new_account_secret),
            &discord_email,
            Some(fred::types::Expiration::EX(ACCOUNT_CREATION_TIMEOUT_SEC)),
            Some(fred::types::SetOptions::NX),
            true,
        )
        .await
    {
        Ok(Some(existing_value)) => {
            return HttpResponse::InternalServerError()
                .body("Generated random duplicate account secret! Try again.");
        }
        Err(err) => {
            return HttpResponse::InternalServerError()
                .body(format!("Error storing account secret: {}", err));
        }
        Ok(None) => (), // Success.
    };

    CreateAccountTemplate {
        config: &app_state.config,
        email: discord_email.as_str(),
        secret: new_account_secret.as_str(),
    }
    .to_response()
}
// TODO - Add session cookie.

/// URL params for account creation form.
#[derive(Debug, Deserialize)]
struct RegisterUserFormData {
    secret: String,
    #[serde(rename = "tos-ack")]
    tos_ack: String, // "on" or "off"
    #[serde(rename = "create-profile")]
    create_profile: Option<String>, // "on" or "off"
    username: Option<String>,
    bio: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsernameExistsQuery {
    username: String,
}

#[get("/user/exists_already")]
pub async fn check_if_user_already_exists(
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
        Err(err) => HttpResponse::InternalServerError().body("database error"),
    }
}

async fn create_session<'a>(
    redis_pool: &'a RedisPool,
    user_id: String,
    previous_session: Option<Cookie<'_>>,
) -> Result<Cookie<'a>, RedisError> {
    let session_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let transaction = redis_pool.multi();
    if let Some(previous_session) = previous_session {
        trace!("Also cleaning up previous session as part of login");
        let _ = transaction.del::<String, _>(key::session(previous_session.name()));
    }
    let _ = transaction
        .hset::<i64, _, _>(
            key::session(&session_id),
            HashMap::from([("user_id", user_id)]),
        )
        .await;
    let _ = transaction
        .expire::<i64, _>(key::session(&session_id), SESSION_TTL_SEC)
        .await;
    transaction.exec::<(RedisValue, RedisValue)>(true).await?;

    Ok(Cookie::build(SESSION_ID_COOKIE, session_id)
        .path("/")
        .http_only(true)
        // .secure(true) // TODO: Set up https testing.
        .max_age(cookie::time::Duration::seconds(SESSION_TTL_SEC))
        // Must be lax to be sent with login redirect and to be logged in when navigating from externally linked pages.
        .same_site(cookie::SameSite::Lax)
        .finish())
}

fn valid_username(regex: &CompiledRegex, username: &str) -> bool {
    username.len() >= 3 && regex.alphanumeric.is_match(username)
}

#[post("/auth/create_account")]
pub async fn create_account(
    app_state: web::Data<AppState>,
    form: web::Form<RegisterUserFormData>,
    request: HttpRequest,
) -> impl Responder {
    if form.create_profile.as_ref().is_some_and(|x| x == "on")
        && (form.username.is_none()
            || form
                .username
                .as_ref()
                .is_some_and(|x| valid_username(&app_state.regex, x)))
    {
        return HttpResponse::BadRequest().body("Bad username");
    }

    let email = match app_state
        .redis_pool
        .getdel::<Option<String>, _>(key::new_account_secret(&form.secret))
        .await
    {
        Err(err) => {
            warn!("Couldn't get pending account creation state: {}", err);
            return HttpResponse::InternalServerError()
                .body("Couldn't get pending account creation state");
        }
        Ok(Some(value)) => value,
        Ok(None) => {
            return HttpResponse::InternalServerError().body("No pending account creation");
        }
    };

    // Create a transaction for both creating the account and the profile.
    let mut transaction = match app_state.db_pool.begin().await {
        Err(err) => {
            error!("Database error when creating transaction: {err}");
            return HttpResponse::InternalServerError().body("Error creating account. Try again?");
        }
        Ok(transaction) => transaction,
    };

    let id: Uuid = match transaction
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
    {
        Err(err) => {
            return HttpResponse::InternalServerError().body("Error creating account. Try again?")
        }
        Ok(row) => row.get(0),
    };

    if let Err(err) = transaction
        .execute(
            sqlx::query(
                r#"
                insert into profile (id, username, bio)
                values ($1, $2, $3)
                "#,
            )
            .bind(id)
            .bind(&form.username)
            .bind(&form.bio),
        )
        .await
    {
        error!("Database error when creating profile: {err}");
        return HttpResponse::InternalServerError().body("Error creating account. Try again?");
    }

    if let Err(err) = transaction.commit().await {
        error!("Database error when committing: {err}");
        return HttpResponse::InternalServerError().body("Error creating account. Try again?");
    }

    match create_session(
        &app_state.redis_pool,
        id.to_string(),
        request.cookie(SESSION_ID_COOKIE),
    )
    .await
    {
        Ok(cookie) => {
            let mut response = partials::MessagePageTemplate {
                config: &app_state.config,
                message: "Account created successfully. You are now logged in.",
            }
            .to_response();
            if let Err(err) = response.add_cookie(&cookie) {
                error!("Couldn't set cookie: {}", err);
            }
            response
        }
        Err(err) => {
            error!("Couldn't create session (after creating new user): {err}");
            return partials::FailureTemplate {
                text: "Couldn't create session; try logging in again.",
            }
            .to_response();
        }
    }
}

#[post("/auth/cancel_create_account")]
pub async fn cancel_create_account(
    app_state: web::Data<AppState>,
    form: web::Form<RegisterUserFormData>,
) -> impl Responder {
    match app_state
        .redis_pool
        .del::<String, _>(key::new_account_secret(&form.secret))
        .await
    {
        Err(err) => {
            warn!(
                "Error talking to Redis when cancelling account creation: {}",
                err
            );
            // This doesn't need to be a user-visible error. The key should
            // expire anyway.
        }
        Ok(_) => (), // We don't care if we didn't remove anything; the key
                     // could've expired.
    };

    partials::MessagePageTemplate {
        config: &app_state.config,
        message: "Account creation cancelled; your information has been forgotten. If you want to create a new account, start over.",
    }
    .to_response()
}

#[get("/auth/logout")]
pub async fn logout(app_state: web::Data<AppState>, request: HttpRequest) -> impl Responder {
    if let Some(mut session_id) = request.cookie(SESSION_ID_COOKIE) {
        trace!("Logging out session {:?}", session_id);
        if let Err(err) = app_state
            .redis_pool
            .del::<String, _>(key::session(&session_id.value()))
            .await
        {
            error!("Failed to clear session {}: {}", &session_id.value(), err)
        }

        let mut response = partials::MessagePageTemplate {
            config: &app_state.config,
            message: "You are now logged out. Goodbye.",
        }
        .to_response();

        // We must re-set some attributes that aren't transmitted with the
        // cookie in the request, otherwise removal won't work.
        session_id.set_path("/");
        if let Err(err) = response.add_removal_cookie(&session_id) {
            error!("Failed to set removal cookie");
            return partials::MessagePageTemplate {
                config: &app_state.config,
                message: "Couldn't log you out for some reason. Try again?",
            }
            .to_response();
        };

        response
    } else {
        partials::MessagePageTemplate {
            config: &app_state.config,
            message: "You were already logged out.",
        }
        .to_response()
    }
}

// TODO - Add logout mechanism.
