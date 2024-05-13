use actix_web::middleware::Logger;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use awc::Client;
use env_logger::Env;
use log::{info, trace, warn};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, RevocationUrl, Scope, StandardRevocableToken, TokenResponse,
    TokenUrl,
};
use serde::Deserialize;
use sqlx::postgres::PgPool;
use std::thread;

#[get("/")]
pub async fn index(app_state: web::Data<AppState>) -> impl Responder {
    // Create an OAuth2 client by specifying the client ID, client secret,
    // authorization URL and token URL.

    // Generate a PKCE challenge.
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate the full authorization URL.
    let (auth_url, csrf_token) = app_state
        .oauth_client
        .authorize_url(CsrfToken::new_random)
        // Set the desired scopes.
        .add_scope(Scope::new("identify".to_string()))
        .add_scope(Scope::new("email".to_string()))
        // Set the PKCE code challenge.
        .set_pkce_challenge(pkce_challenge)
        .url();

    match sqlx::query(
        r#"
        insert into oauth_preredirect_pending (csrf_token, pkce_verifier)
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
            return HttpResponse::InternalServerError().body(
                format!("Tried to store oauth challenge but got error: {}", err).to_string(),
            );
        }
    }

    // This is the URL you should redirect the user to, in order to trigger the
    // authorization process.
    HttpResponse::Ok().body(
        format!(
            "<p>You are currently logged out. To log in: <a href=\"{}\">{}</a></p>",
            auth_url, auth_url
        )
        .to_string(),
    )
}

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

#[get("/auth/discord")]
pub async fn auth_discord(
    app_state: web::Data<AppState>,
    params: web::Query<DiscordOauthRedirectParams>,
) -> impl Responder {
    // Once the user has been redirected to the redirect URL, you'll have access
    // to the authorization code. For security reasons, your code should verify
    // that the `state` parameter returned by the server matches `csrf_state`.
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
                    delete from oauth_preredirect_pending
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
                return HttpResponse::InternalServerError().body(
                    format!(
                        "Unknown error response from Discord API: error: {}, error_description: {}",
                        error, error_description
                    )
                    .to_string(),
                );
            }
        }
    };

    let result: Result<Option<(String,)>, sqlx::Error> = sqlx::query_as(
        r#"
        delete from oauth_preredirect_pending
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
            return HttpResponse::InternalServerError().body(
                format!("Database error while retrieving oauth record: {}", err).to_string(),
            );
        }
        Ok(Some((verifier,))) => verifier,
    };
    // Invariant params.state == csrf_token already checked with SQL.

    // Now you can trade it for an access token.
    let token_response = match app_state
        .oauth_client
        .exchange_code(AuthorizationCode::new(oauth_code.to_string()))
        // Set the PKCE code verifier.
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        .request_async(async_http_client)
        .await
    {
        Ok(resp) => resp,
        Err(err) => {
            return HttpResponse::InternalServerError()
                .body(format!("Error while retrieving oauth token: {}", err).to_string());
        }
    };

    let client = Client::default();
    let discord_user = {
        let mut response = match client
            .get("https://discord.com/api/v10/users/@me")
            .insert_header(("Accept", "application/json"))
            .insert_header((
                "Authorization",
                format!("Bearer {}", token_response.access_token().secret()),
            ))
            .insert_header(("User-Agent", "awc/3.4"))
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(err) => {
                return HttpResponse::InternalServerError()
                    .body(format!("Error getting email from Discord API: {}", err).to_string())
            }
        };

        match response.json::<DiscordUser>().await {
            Ok(user) => user,
            Err(err) => {
                return HttpResponse::InternalServerError()
                    .body(format!("Error decoding response from Discord API: {}", err))
            }
        }
    };

    // We don't care if revoking the token fails, because the main work is done.
    thread::spawn(move || {
        let token_to_revoke: StandardRevocableToken = match token_response.refresh_token() {
            Some(token) => token.into(),
            None => token_response.access_token().into(),
        };
        if let Some(err) = app_state
            .oauth_client
            .clone()
            .revoke_token(token_to_revoke)
            .err()
        {
            warn!("Ignored error revoking Discord token: {}", err);
        } else {
            trace!("Token revocation successful");
        }
    });

    // Unwrapping token_response will either produce a Token or a RequestTokenError.
    HttpResponse::Ok().body(format!("You are logged in as: {}", discord_user.email))
}

// #[derive(Deserialize)]
// struct LogoutRequest {

// }

// #[get("/auth/logout_discord")]
// async fn logout_discord(params: web::Query<
