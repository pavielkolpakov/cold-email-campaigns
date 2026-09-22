use anyhow::{Context, Result};
use api::{config, routes, scheduler, state, worker};
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

    let state = state::AppState { pool, config };

    // One binary, three modes: `api`, `worker`, `scheduler`.
    match std::env::args().nth(1).as_deref().unwrap_or("api") {
        "api" => routes::serve(state).await,
        "worker" => worker::run(state).await,
        "scheduler" => scheduler::run(state).await,
        other => anyhow::bail!("unknown mode `{other}` (expected api, worker, or scheduler)"),
    }
}
