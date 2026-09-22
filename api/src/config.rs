use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub api_bind: String,
    /// Origin of the Next.js app. Drives CORS and the `Secure` cookie flag.
    pub app_url: String,
    pub session_cookie_name: String,
    pub session_ttl_days: i64,
    pub encryption_key: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_uri: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: var("DATABASE_URL")?,
            api_bind: std::env::var("API_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            app_url: std::env::var("APP_URL").unwrap_or_else(|_| "http://localhost:3000".into()),
            session_cookie_name: std::env::var("SESSION_COOKIE_NAME")
                .unwrap_or_else(|_| "ce_session".into()),
            session_ttl_days: 30,
            encryption_key: var("ENCRYPTION_KEY")?,
            google_client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
            google_client_secret: std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
            google_redirect_uri: std::env::var("GOOGLE_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:3000/api/mailboxes/gmail/callback".into()),
        })
    }

    /// Whether a mailbox can actually be connected on this deployment.
    pub fn google_configured(&self) -> bool {
        !self.google_client_id.is_empty() && !self.google_client_secret.is_empty()
    }

    pub fn secure_cookies(&self) -> bool {
        self.app_url.starts_with("https://")
    }
}

fn var(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("missing required env var {key}"))
}
