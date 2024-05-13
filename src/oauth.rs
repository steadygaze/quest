use actix_web::web::Either;
use actix_web::{get, web, HttpResponse, Responder};
use askama::Template;
use askama_actix::TemplateToResponse;
use awc::Client;
use log::{info, trace, warn};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, RevocationUrl, TokenUrl};
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope,
    StandardRevocableToken, TokenResponse,
};
use serde::Deserialize;
use std::thread;

use crate::models::AppState;

const DISCORD_APP_ID: &str = "1229875695316893737";
const DISCORD_CLIENT_SECRET: &str = "trm5sN6uM1VQjXwTUSKZZu3WLXa0x3lM";
const DISCORD_TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
const DISCORD_AUTH_URL: &str = "https://discord.com/oauth2/authorize";

/// Generates an oauth client for Discord.
pub fn oauth_client(port: u16) -> BasicClient {
    BasicClient::new(
        ClientId::new(DISCORD_APP_ID.to_string()),
        Some(ClientSecret::new(DISCORD_CLIENT_SECRET.to_string())),
        AuthUrl::new(DISCORD_AUTH_URL.to_string()).unwrap(),
        Some(TokenUrl::new(DISCORD_TOKEN_URL.to_string()).unwrap()),
    )
    // Set the URL the user will be redirected to after the authorization process.
    .set_redirect_uri(
        RedirectUrl::new(format!("http://127.0.0.1:{}/auth/discord/callback", port).to_string())
            .unwrap(),
    )
    .set_revocation_uri(
        RevocationUrl::new("https://discord.com/api/oauth2/token/revoke".to_string()).unwrap(),
    )
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

    // Record oauth challenge for verification after redirecting back.
    match sqlx::query(
        r#"
        insert into oauth_redirect_pending (csrf_token, pkce_verifier)
        values ($1, $2)
        "#,
    )
    .bind(csrf_token.secret())
    .bind(pkce_verifier.secret())
    .execute(&app_state.pool)
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
    }

    info!("auth url is {}", auth_url);
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
}

#[get("/auth/discord/callback")]
pub async fn discord_callback(
    app_state: web::Data<AppState>,
    params: web::Query<DiscordOauthRedirectParams>,
) -> impl Responder {
    trace!("Got oauth params: {:?}", params);

    let (oauth_code, oauth_state) = match params.into_inner() {
        DiscordOauthRedirectParams::DiscordOauthOk { code, state } => (code, state),
        DiscordOauthRedirectParams::DiscordApiError {
            error,
            error_description,
            state,
        } => {
            // We don't care if the query succeeds because there is a main error already.
            thread::spawn(|| async move {
                let state = state.clone();
                let pool = app_state.pool.clone();

                match sqlx::query(
                    r#"
                    delete from oauth_redirect_pending
                    where csrf_token = $1
                    "#,
                )
                .bind(&state)
                .execute(&pool)
                .await
                {
                    Ok(result) => {
                        if result.rows_affected() <= 0 {
                            info!(
                                "Ignored missing oauth entry when cleaning up; it likely expired"
                            );
                        }
                    }
                    Err(err) => {
                        warn!(
                            "Ignored database error when cleaning up pending oauth entry: {}",
                            err
                        );
                    }
                }
            });

            if error == "access_denied" {
                return HttpResponse::Unauthorized()
                    .body("Access denied. Try again, but grant permissions.");
            } else {
                return HttpResponse::InternalServerError().body(format!(
                    "Unknown error response from Discord API: error: {}, error_description: {}",
                    error, error_description
                ));
            }
        }
    };

    let result: Result<Option<(String,)>, sqlx::Error> = sqlx::query_as(
        r#"
        delete from oauth_redirect_pending
        where csrf_token = $1
        returning pkce_verifier
        "#,
    )
    .bind(&oauth_state)
    .fetch_optional(&app_state.pool)
    .await;

    let pkce_verifier = match result {
        Ok(None) => {
            return HttpResponse::Unauthorized().body("Couldn't find pending oauth request");
        }
        Err(err) => {
            return HttpResponse::InternalServerError().body(format!(
                "Database error while retrieving oauth record: {}",
                err
            ));
        }
        Ok(Some((verifier,))) => verifier,
    };
    // Invariant params.state == csrf_token already checked with SQL.

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
    .fetch_one(&app_state.pool)
    .await
    {
        Ok((true,)) => HttpResponse::Ok().body(format!("You are logged in as: {}", discord_email)),
        Ok((false,)) => CreateAccountTemplate {
            email: discord_email.as_str(),
        }
        .to_response(),
        Err(err) => HttpResponse::InternalServerError().body(format!(
            "Error checking account status for {}: {}",
            discord_email, err
        )),
    }
}
// TODO - Add session cookie.

/// URL params expected when Discord redirects back after oauth.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RegisterNewUserRequest {
    CancelRegistration { code: String },
    Register { code: String },
}

#[get("/auth/create_account")]
pub async fn create_account(
    app_state: web::Data<AppState>,
    params: web::Query<RegisterNewUserRequest>,
) -> impl Responder {
    HttpResponse::Ok().body("ok")
}

// TODO - Add logout mechanism.
