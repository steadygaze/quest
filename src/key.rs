/// Redis key generation. These are a bunch of simple helpers for generating
/// Redis keys. We use helpers to prevent dumb typos.

pub fn oauth_secret(secret: &str) -> String {
    format!("oauth:secret:{secret}")
}

pub fn new_account_secret(secret: &str) -> String {
    format!("account:new:secret:{secret}")
}

pub fn session(session_id: &str) -> String {
    format!("session:{session_id}")
}
