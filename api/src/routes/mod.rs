pub mod auth;
pub mod campaigns;
pub mod invites;
pub mod leads;
pub mod mailboxes;

use anyhow::Context;
use axum::Router;
use axum::http::{HeaderValue, Method, header};
use axum::routing::get;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

/// CSV uploads arrive as JSON, so the body limit has to fit a real lead list.
const MAX_BODY_BYTES: usize = 20 * 1024 * 1024;

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            state
                .config
                .app_url
                .parse::<HeaderValue>()
                .expect("APP_URL must be a valid origin"),
        )
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE])
        .allow_credentials(true);

    Router::new()
        .route("/health", get(auth::health))
        .merge(auth::routes())
        .merge(mailboxes::routes())
        .merge(leads::routes())
        .merge(campaigns::routes())
        .merge(invites::routes())
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(state: AppState) -> anyhow::Result<()> {
    let bind = state.config.api_bind.clone();
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("failed to bind {bind}"))?;
    tracing::info!(%bind, "api listening");
    axum::serve(listener, router(state)).await?;
    Ok(())
}
