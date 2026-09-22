#![allow(dead_code)]

use api::crypto::Cipher;
use sqlx::PgPool;
use uuid::Uuid;

pub const TEST_KEY: &str = "bTfLDZ0kFqfhy5qTmdsmMxTjy0/6sdtIBOFkDFCKVGE=";

pub fn cipher() -> Cipher {
    Cipher::from_base64_key(TEST_KEY).unwrap()
}

pub async fn seed_org(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar!("insert into orgs (name) values ($1) returning id", name)
        .fetch_one(pool)
        .await
        .unwrap()
}

use api::mailboxes::Credentials;
use api::provider::{Mailer, OutboundMessage, SentMessage};
use std::sync::Mutex;

/// Records what would have been sent, so tests can assert on it.
#[derive(Default)]
pub struct FakeMailer {
    pub sent: Mutex<Vec<(String, OutboundMessage)>>,
}

#[async_trait::async_trait]
impl Mailer for FakeMailer {
    async fn send(
        &self,
        from: &str,
        credentials: &Credentials,
        message: &OutboundMessage,
    ) -> anyhow::Result<SentMessage> {
        self.sent
            .lock()
            .unwrap()
            .push((credentials.access_token.clone(), message.clone()));

        let count = self.sent.lock().unwrap().len();
        Ok(SentMessage {
            provider_message_id: format!("msg-{from}"),
            thread_id: "thread-1".into(),
            message_id_header: format!("<message-id-{count}@test>"),
        })
    }
}

use api::config::Config;
use api::provider::gmail::GmailOAuth;
use api::state::AppState;
use axum::Router;
use std::sync::Arc;

pub fn test_config() -> Config {
    Config {
        database_url: String::new(),
        api_bind: "127.0.0.1:0".into(),
        app_url: "http://localhost:3000".into(),
        session_cookie_name: "ce_session".into(),
        session_ttl_days: 30,
        encryption_key: TEST_KEY.into(),
        google_client_id: "client-id".into(),
        google_client_secret: "client-secret".into(),
        google_redirect_uri: "http://localhost:3000/api/mailboxes/gmail/callback".into(),
    }
}

/// The API wired to a fake mailer, so route tests never touch Google.
pub fn test_app(pool: PgPool, mailer: Arc<FakeMailer>) -> Router {
    api::routes::router(AppState {
        pool,
        config: test_config(),
        cipher: cipher(),
        oauth: Arc::new(GmailOAuth::new(
            "client-id".into(),
            "client-secret".into(),
            "http://localhost:1/token".into(),
        )),
        mailer,
    })
}
