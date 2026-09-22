use anyhow::{Context, Result};
use api::crypto::Cipher;
use api::provider::gmail::{GmailInbox, GmailMailer, GmailOAuth};
use api::{config, routes, scheduler, seed, state, worker};
use std::sync::Arc;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = config::Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("failed to connect to the database")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    let cipher = Cipher::from_base64_key(&config.encryption_key)?;
    let oauth = Arc::new(GmailOAuth::new(
        config.google_client_id.clone(),
        config.google_client_secret.clone(),
        api::provider::gmail::GOOGLE_TOKEN_ENDPOINT.to_string(),
    ));
    if !config.google_configured() {
        tracing::warn!("GOOGLE_CLIENT_ID/SECRET are unset; mailboxes cannot be connected");
    }

    let state = state::AppState {
        pool,
        config,
        cipher,
        oauth,
        mailer: Arc::new(GmailMailer::default()),
        inbox: Arc::new(GmailInbox::default()),
    };

    // One binary, three modes: `api`, `worker`, `scheduler`.
    match std::env::args().nth(1).as_deref().unwrap_or("api") {
        "api" => routes::serve(state).await,
        "worker" => worker::run(state).await,
        "scheduler" => scheduler::run(state).await,
        "seed" => seed::run(&state.pool).await,
        other => anyhow::bail!("unknown mode `{other}` (expected api, worker, scheduler, or seed)"),
    }
}
