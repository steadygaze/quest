// use actix_web::dev::HttpServiceFactory;
use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;
use actix_web::web::Either;
use actix_web::{get, web, HttpResponse, Responder};
use askama::Template;
use askama_actix::TemplateToResponse;
use awc::Client;
use fred::interfaces::KeysInterface;
use log::{trace, warn};
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope,
    StandardRevocableToken, TokenResponse,
};
use rand::distributions::{Alphanumeric, DistString};
use serde::Deserialize;
use std::thread;

use crate::app_state::AppState;

const OAUTH_EXPIRATION_SEC: i64 = 60 * 10;
const ACCOUNT_CREATION_TIMEOUT_SEC: i64 = 60 * 30;

/// Add oauth-related routes.
pub fn add_routes<T>(app: actix_web::App<T>) -> actix_web::App<T>
where
    T: ServiceFactory<ServiceRequest, Config = (), Error = actix_web::Error, InitError = ()>,
{
    app.service(discord_start)
        .service(discord_callback)
        .service(create_account)
        .service(test)
}

#[get("/auth/test")]
pub async fn test(app_state: web::Data<AppState>) -> impl Responder {
    CreateAccountTemplate {
        email: "test@test.com",
        secret: "mysecret",
    }
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
            format!("oauth:secret:{}", csrf_token.secret()),
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
    email: &'a str,
    secret: &'a str,
}

#[get("/auth/discord/callback")]
pub async fn discord_callback(
    app_state: web::Data<AppState>,
    params: web::Query<DiscordOauthRedirectParams>,
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
                    .del::<i64, _>(format!("oauth:secret:{state}"))
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
        .getdel::<Option<String>, _>(format!("oauth:secret:{oauth_state}"))
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

    match sqlx::query_as(
        r#"
        select exists(
          select 1
          from account
          where email = $1
          limit 1
        )
        "#,
    )
    .bind(&discord_email)
    .fetch_one(&app_state.db_pool)
    .await
    {
        Ok((true,)) => {
            return HttpResponse::Ok().body(format!("You are logged in as: {}", discord_email));
        }
        Err(err) => {
            return HttpResponse::InternalServerError().body(format!(
                "Error checking account status for {}: {}",
                discord_email, err
            ));
        }
        Ok((false,)) => (), // Success, but now we must create an account.
    }

    // Store an account secret to be passed back, indicating there is a pending
    // account creation.
    let new_account_secret = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    match app_state
        .redis_pool
        .set::<Option<String>, _, _>(
            format!("account:new:secret:{new_account_secret}"),
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
        email: discord_email.as_str(),
        secret: new_account_secret.as_str(),
    }
    .to_response()
}
// TODO - Add session cookie.

/// URL params for account creation form.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RegisterNewUserRequest {
    Register { code: String },
    Cancel { code: String },
}

#[get("/auth/create_account")]
pub async fn create_account(
    app_state: web::Data<AppState>,
    params: web::Query<RegisterNewUserRequest>,
) -> impl Responder {
    HttpResponse::Ok().body("ok")
}

// TODO - Add logout mechanism.
