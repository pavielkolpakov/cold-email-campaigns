use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::crypto::Cipher;
use crate::provider::{InboxReader, Mailer};
use crate::provider::gmail::GmailOAuth;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub cipher: Cipher,
    pub oauth: Arc<GmailOAuth>,
    pub mailer: Arc<dyn Mailer>,
    pub inbox: Arc<dyn InboxReader>,
}
