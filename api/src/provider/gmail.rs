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
use crate::provider::{Mailer, MailerError, OutboundMessage, SentMessage};

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
    ) -> Result<SentMessage, MailerError> {
        #[derive(Deserialize)]
        struct SendResponse {
            id: String,
            #[serde(rename = "threadId")]
            thread_id: String,
        }

        // Stamping our own Message-ID means we know it without re-fetching the
        // sent message, and the next followup can reference it.
        let domain = from.split('@').nth(1).unwrap_or("localhost");
        let message_id = format!("<{}@{domain}>", uuid::Uuid::new_v4());

        let mut raw = format!(
            "From: {from}\r\nTo: {}\r\nSubject: {}\r\nMessage-ID: {message_id}\r\n",
            message.to, message.subject
        );
        if let Some(in_reply_to) = &message.in_reply_to {
            // In-Reply-To alone is not enough for every client; References is
            // what threads the conversation reliably.
            raw.push_str(&format!(
                "In-Reply-To: {in_reply_to}\r\nReferences: {in_reply_to}\r\n"
            ));
        }
        raw.push_str("Content-Type: text/plain; charset=UTF-8\r\n\r\n");
        raw.push_str(&message.body);

        // The API-level thread handle. Gmail needs this *as well as* the
        // headers above to attach the message rather than start a thread.
        let mut payload = serde_json::json!({ "raw": BASE64_URL.encode(raw) });
        if let Some(thread_id) = &message.thread_id {
            payload["threadId"] = serde_json::Value::String(thread_id.clone());
        }

        let response = self
            .http
            .post(format!("{}/users/me/messages/send", self.api_base))
            .bearer_auth(&credentials.access_token)
            .json(&payload)
            .send()
            .await
            .map_err(|err| MailerError::Other(err.into()))?;

        if !response.status().is_success() {
            let status = response.status();
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(std::time::Duration::from_secs);
            let body = response.text().await.unwrap_or_default();
            return Err(classify_send_failure(status, retry_after, &body));
        }

        let sent: SendResponse = response
            .json()
            .await
            .map_err(|err| MailerError::Other(err.into()))?;
        Ok(SentMessage {
            provider_message_id: sent.id,
            thread_id: sent.thread_id,
            message_id_header: message_id,
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

/// Google wraps the useful sentence in `{"error": {"message": ...}}`. Falling
/// back to the raw body keeps unexpected shapes debuggable.
fn google_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|json| {
            json.pointer("/error/message")
                .and_then(|message| message.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.chars().take(500).collect())
}

use crate::inbound::InboundMessage;
use crate::provider::{InboxPage, InboxReader};
use chrono::TimeZone;
use std::collections::BTreeMap;

/// Reads a Gmail mailbox incrementally via the history API.
pub struct GmailInbox {
    http: reqwest::Client,
    api_base: String,
}

impl Default for GmailInbox {
    fn default() -> Self {
        Self {
            http: reqwest::Client::new(),
            api_base: GMAIL_API_BASE.into(),
        }
    }
}

#[derive(Deserialize)]
struct Profile {
    #[serde(rename = "historyId")]
    history_id: String,
}

#[derive(Deserialize)]
struct HistoryList {
    #[serde(default)]
    history: Vec<HistoryRecord>,
    #[serde(rename = "historyId")]
    history_id: Option<String>,
}

#[derive(Deserialize)]
struct HistoryRecord {
    #[serde(rename = "messagesAdded", default)]
    messages_added: Vec<MessageAdded>,
}

#[derive(Deserialize)]
struct MessageAdded {
    message: MessageRef,
}

#[derive(Deserialize)]
struct MessageRef {
    id: String,
}

#[derive(Deserialize)]
struct GmailMessage {
    id: String,
    #[serde(rename = "threadId")]
    thread_id: String,
    #[serde(rename = "internalDate")]
    internal_date: Option<String>,
    payload: Option<MessagePayload>,
}

#[derive(Deserialize)]
struct MessagePayload {
    #[serde(default)]
    headers: Vec<GmailHeader>,
}

#[derive(Deserialize)]
struct GmailHeader {
    name: String,
    value: String,
}

#[async_trait::async_trait]
impl InboxReader for GmailInbox {
    async fn fetch(
        &self,
        credentials: &Credentials,
        cursor: Option<&str>,
    ) -> anyhow::Result<InboxPage> {
        let token = &credentials.access_token;

        // No cursor yet: take the mailbox's current position and read nothing.
        // Scanning history from the beginning would replay old conversations.
        let Some(cursor) = cursor else {
            let profile: Profile = self
                .get(&format!("{}/users/me/profile", self.api_base), token)
                .await?
                .json()
                .await?;
            return Ok(InboxPage {
                messages: Vec::new(),
                cursor: profile.history_id,
            });
        };

        let response = self
            .get(
                &format!(
                    "{}/users/me/history?startHistoryId={cursor}&historyTypes=messageAdded",
                    self.api_base
                ),
                token,
            )
            .await?;

        // Gmail drops history older than about a week; restart from the
        // current position rather than failing forever.
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            let profile: Profile = self
                .get(&format!("{}/users/me/profile", self.api_base), token)
                .await?
                .json()
                .await?;
            tracing::warn!("gmail history cursor expired, restarting from current position");
            return Ok(InboxPage {
                messages: Vec::new(),
                cursor: profile.history_id,
            });
        }

        let history: HistoryList = response.error_for_status()?.json().await?;
        let next_cursor = history.history_id.unwrap_or_else(|| cursor.to_string());

        let mut ids: Vec<String> = history
            .history
            .into_iter()
            .flat_map(|record| record.messages_added)
            .map(|added| added.message.id)
            .collect();
        ids.sort();
        ids.dedup();

        let mut messages = Vec::with_capacity(ids.len());
        for id in ids {
            match self.message(&id, token).await {
                Ok(message) => messages.push(message),
                // One unreadable message must not abandon the whole sync.
                Err(error) => tracing::warn!(%id, ?error, "skipping unreadable message"),
            }
        }

        Ok(InboxPage {
            messages,
            cursor: next_cursor,
        })
    }
}

impl GmailInbox {
    async fn get(&self, url: &str, token: &str) -> anyhow::Result<reqwest::Response> {
        Ok(self.http.get(url).bearer_auth(token).send().await?)
    }

    async fn message(&self, id: &str, token: &str) -> anyhow::Result<InboundMessage> {
        let message: GmailMessage = self
            .get(
                &format!("{}/users/me/messages/{id}?format=metadata", self.api_base),
                token,
            )
            .await?
            .error_for_status()?
            .json()
            .await?;

        let headers: BTreeMap<String, String> = message
            .payload
            .map(|payload| payload.headers)
            .unwrap_or_default()
            .into_iter()
            .map(|header| (header.name.to_lowercase(), header.value))
            .collect();

        Ok(InboundMessage {
            provider_message_id: message.id,
            thread_id: message.thread_id,
            from: extract_address(headers.get("from").map(String::as_str).unwrap_or_default()),
            subject: headers.get("subject").cloned().unwrap_or_default(),
            received_at: message
                .internal_date
                .and_then(|millis| millis.parse::<i64>().ok())
                .and_then(|millis| Utc.timestamp_millis_opt(millis).single())
                .unwrap_or_else(Utc::now),
            headers,
        })
    }
}

/// `From` is usually `Ada Lovelace <ada@example.com>`; we only want the address.
fn extract_address(from: &str) -> String {
    match (from.find('<'), from.find('>')) {
        (Some(start), Some(end)) if start < end => from[start + 1..end].trim().to_lowercase(),
        _ => from.trim().to_lowercase(),
    }
}

/// Maps Gmail's refusal onto something the worker can act on. Google signals
/// throttling with 429, and also with 403 plus a rate-limit reason, so the
/// status alone is not enough.
fn classify_send_failure(
    status: reqwest::StatusCode,
    retry_after: Option<std::time::Duration>,
    body: &str,
) -> MailerError {
    let message = google_message(body);
    let lower = message.to_lowercase();

    if status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || lower.contains("ratelimitexceeded")
        || lower.contains("rate limit")
        || lower.contains("user-rate limit")
    {
        return MailerError::RateLimited { retry_after };
    }
    if status == reqwest::StatusCode::UNAUTHORIZED
        || lower.contains("invalid credentials")
        || lower.contains("invalid_grant")
    {
        return MailerError::Unauthorized;
    }
    if status == reqwest::StatusCode::BAD_REQUEST {
        return MailerError::InvalidRecipient(message);
    }

    MailerError::Other(anyhow!("gmail send failed ({status}): {message}"))
}
