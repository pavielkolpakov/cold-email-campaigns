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

/// A message to send. Threading a followup needs both identifiers: the
/// provider's own thread handle, and the RFC822 `Message-ID` of the message
/// being replied to. Gmail silently opens a new conversation without the latter.
#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub to: String,
    pub subject: String,
    pub body: String,
    /// Provider-side thread handle, e.g. Gmail's `threadId`.
    pub thread_id: Option<String>,
    /// `Message-ID` header of the message this replies to.
    pub in_reply_to: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SentMessage {
    pub provider_message_id: String,
    pub thread_id: String,
    /// The `Message-ID` we stamped on the outgoing mail, kept so the next
    /// followup can point `In-Reply-To` at it.
    pub message_id_header: String,
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

/// One page of a mailbox's recent messages, plus the cursor to resume from.
pub struct InboxPage {
    pub messages: Vec<crate::inbound::InboundMessage>,
    pub cursor: String,
}

/// Reads a connected mailbox looking for answers to what we sent.
#[async_trait::async_trait]
pub trait InboxReader: Send + Sync {
    async fn fetch(
        &self,
        credentials: &crate::mailboxes::Credentials,
        cursor: Option<&str>,
    ) -> anyhow::Result<InboxPage>;
}
