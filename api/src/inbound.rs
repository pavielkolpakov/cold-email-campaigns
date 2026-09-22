use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

/// A message found in a connected mailbox. Headers are kept raw and lowercased
/// so classification stays a pure decision over them, testable on its own.
#[derive(Debug, Clone)]
pub struct InboundMessage {
    pub provider_message_id: String,
    pub thread_id: String,
    pub from: String,
    pub subject: String,
    pub headers: BTreeMap<String, String>,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    /// A human answered. Stop the sequence.
    Reply,
    /// A vacation responder or similar. The prospect has not read anything, so
    /// the sequence carries on.
    AutoReply,
    /// The address does not accept mail. Stop and suppress it.
    Bounce,
}

/// Bounces are checked first: they frequently carry auto-reply headers too, and
/// a dead address matters more than the machine that announced it.
pub fn classify(message: &InboundMessage) -> Classification {
    if is_bounce(message) {
        return Classification::Bounce;
    }
    if is_auto_reply(message) {
        return Classification::AutoReply;
    }
    Classification::Reply
}

fn is_bounce(message: &InboundMessage) -> bool {
    let from = message.from.to_lowercase();
    if from.contains("mailer-daemon") || from.contains("postmaster@") {
        return true;
    }
    // RFC 3464: a delivery status notification is a multipart/report.
    header(message, "content-type")
        .map(|value| value.contains("report-type=delivery-status"))
        .unwrap_or(false)
}

fn is_auto_reply(message: &InboundMessage) -> bool {
    // RFC 3834 plus the informal headers that predate it and are still common.
    if let Some(value) = header(message, "auto-submitted") {
        if value != "no" {
            return true;
        }
    }
    if header(message, "x-autoreply").is_some() || header(message, "x-autorespond").is_some() {
        return true;
    }
    matches!(
        header(message, "precedence").as_deref(),
        Some("auto_reply" | "bulk" | "junk")
    )
}

fn header(message: &InboundMessage, name: &str) -> Option<String> {
    message
        .headers
        .get(name)
        .map(|value| value.trim().to_lowercase())
}
