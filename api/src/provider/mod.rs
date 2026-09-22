pub mod gmail;

use chrono::{DateTime, Utc};

/// A newly issued access token. Google does not reissue the refresh token on
/// refresh, so callers keep the one they already hold.
#[derive(Debug, Clone)]
pub struct RefreshedToken {
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
}

/// Why a refresh failed. The distinction matters: a revoked grant is permanent
/// and the user must reconnect, while anything else is worth retrying.
#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    #[error("the mailbox grant was revoked or expired")]
    Revoked,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Exchanges a refresh token for a fresh access token.
#[async_trait::async_trait]
pub trait TokenRefresher: Send + Sync {
    async fn refresh(&self, refresh_token: &str) -> Result<RefreshedToken, RefreshError>;
}

/// A message to send. Followups carry `in_reply_to` so they land in the thread.
#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub in_reply_to: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SentMessage {
    pub provider_message_id: String,
    pub thread_id: String,
}

/// Sends on behalf of a connected mailbox. The seam that keeps campaign logic
/// independent of Gmail, Outlook or SMTP.
#[async_trait::async_trait]
pub trait Mailer: Send + Sync {
    async fn send(
        &self,
        from: &str,
        credentials: &crate::mailboxes::Credentials,
        message: &OutboundMessage,
    ) -> anyhow::Result<SentMessage>;
}
