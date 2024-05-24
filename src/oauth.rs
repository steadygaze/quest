use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, RevocationUrl, TokenUrl};

use crate::app_state::AppConfig;

const DISCORD_TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
const DISCORD_AUTH_URL: &str = "https://discord.com/oauth2/authorize";

/// Generates an oauth client for Discord.
pub fn oauth_client(config: &AppConfig) -> BasicClient {
    BasicClient::new(
        ClientId::new(config.discord_app_id.clone()),
        Some(ClientSecret::new(config.discord_client_secret.clone())),
        AuthUrl::new(DISCORD_AUTH_URL.to_string()).unwrap(),
        Some(TokenUrl::new(DISCORD_TOKEN_URL.to_string()).unwrap()),
    )
    // Set the URL the user will be redirected to after the authorization process.
    .set_redirect_uri(
        RedirectUrl::new(
            format!("http://127.0.0.1:{}/auth/discord/callback", config.port).to_string(),
        )
        .unwrap(),
    )
    .set_revocation_uri(
        RevocationUrl::new("https://discord.com/api/oauth2/token/revoke".to_string()).unwrap(),
    )
}
