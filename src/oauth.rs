use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, RevocationUrl, TokenUrl};

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
