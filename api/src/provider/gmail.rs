use anyhow::anyhow;
use chrono::{Duration, Utc};
use serde::Deserialize;

use super::{RefreshError, RefreshedToken, TokenRefresher};

pub const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

/// Google's OAuth endpoints. The token endpoint is injectable so tests can point
/// it at a stub server.
#[derive(Clone)]
pub struct GmailOAuth {
    client_id: String,
    client_secret: String,
    token_endpoint: String,
    auth_endpoint: String,
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
}

impl GmailOAuth {
    pub fn new(client_id: String, client_secret: String, token_endpoint: String) -> Self {
        Self {
            client_id,
            client_secret,
            token_endpoint,
            auth_endpoint: GOOGLE_AUTH_ENDPOINT.into(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl TokenRefresher for GmailOAuth {
    async fn refresh(&self, refresh_token: &str) -> Result<RefreshedToken, RefreshError> {
        let response = self
            .http
            .post(&self.token_endpoint)
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|err| RefreshError::Other(err.into()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            // Google answers a revoked or expired grant with 400 invalid_grant.
            if body.contains("invalid_grant") {
                return Err(RefreshError::Revoked);
            }
            return Err(RefreshError::Other(anyhow!(
                "token refresh failed with {status}: {body}"
            )));
        }

        let token: TokenResponse = response
            .json()
            .await
            .map_err(|err| RefreshError::Other(err.into()))?;
        Ok(RefreshedToken {
            access_token: token.access_token,
            expires_at: Utc::now() + Duration::seconds(token.expires_in),
        })
    }
}

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64_URL;

use crate::mailboxes::Credentials;
use crate::provider::{Mailer, OutboundMessage, SentMessage};

pub const GOOGLE_AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GMAIL_API_BASE: &str = "https://gmail.googleapis.com/gmail/v1";
pub const SCOPES: &str = "https://www.googleapis.com/auth/gmail.send \
https://www.googleapis.com/auth/gmail.readonly \
https://www.googleapis.com/auth/userinfo.email";

impl GmailOAuth {
    /// Where to send the user to grant access. `access_type=offline` plus
    /// `prompt=consent` is what makes Google hand back a refresh token.
    pub fn consent_url(&self, redirect_uri: &str, state: &str) -> String {
        let params = [
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("response_type", "code"),
            ("scope", SCOPES),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("state", state),
        ];
        let query = params
            .iter()
            .map(|(key, value)| format!("{key}={}", urlencode(value)))
            .collect::<Vec<_>>()
            .join("&");
        format!("{}?{query}", self.auth_endpoint)
    }

    /// Trades the one-time code from the callback for tokens.
    pub async fn exchange_code(&self, code: &str, redirect_uri: &str) -> anyhow::Result<Credentials> {
        #[derive(Deserialize)]
        struct CodeResponse {
            access_token: String,
            refresh_token: Option<String>,
            expires_in: i64,
        }

        let response = self
            .http
            .post(&self.token_endpoint)
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("code exchange failed with {status}: {body}"));
        }

        let token: CodeResponse = response.json().await?;
        let refresh_token = token.refresh_token.ok_or_else(|| {
            anyhow!("google did not return a refresh token; the account must be disconnected in its google security settings and reconnected")
        })?;

        Ok(Credentials {
            access_token: token.access_token,
            refresh_token,
            expires_at: Utc::now() + Duration::seconds(token.expires_in),
        })
    }

    /// Which address the granted tokens belong to.
    pub async fn mailbox_address(&self, access_token: &str) -> anyhow::Result<String> {
        #[derive(Deserialize)]
        struct Profile {
            email: String,
        }

        let profile: Profile = self
            .http
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .bearer_auth(access_token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(profile.email)
    }
}

/// Sends through the Gmail API.
pub struct GmailMailer {
    http: reqwest::Client,
    api_base: String,
}

impl Default for GmailMailer {
    fn default() -> Self {
        Self {
            http: reqwest::Client::new(),
            api_base: GMAIL_API_BASE.into(),
        }
    }
}

#[async_trait::async_trait]
impl Mailer for GmailMailer {
    async fn send(
        &self,
        from: &str,
        credentials: &Credentials,
        message: &OutboundMessage,
    ) -> anyhow::Result<SentMessage> {
        #[derive(Deserialize)]
        struct SendResponse {
            id: String,
            #[serde(rename = "threadId")]
            thread_id: String,
        }

        let mut raw = format!(
            "From: {from}\r\nTo: {}\r\nSubject: {}\r\n",
            message.to, message.subject
        );
        if let Some(in_reply_to) = &message.in_reply_to {
            // Both headers are needed for Gmail to thread the reply correctly.
            raw.push_str(&format!(
                "In-Reply-To: {in_reply_to}\r\nReferences: {in_reply_to}\r\n"
            ));
        }
        raw.push_str("Content-Type: text/plain; charset=UTF-8\r\n\r\n");
        raw.push_str(&message.body);

        let response = self
            .http
            .post(format!("{}/users/me/messages/send", self.api_base))
            .bearer_auth(&credentials.access_token)
            .json(&serde_json::json!({ "raw": BASE64_URL.encode(raw) }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("gmail send failed with {status}: {body}"));
        }

        let sent: SendResponse = response.json().await?;
        Ok(SentMessage {
            provider_message_id: sent.id,
            thread_id: sent.thread_id,
        })
    }
}

fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            b' ' => "%20".to_string(),
            other => format!("%{other:02X}"),
        })
        .collect()
}
