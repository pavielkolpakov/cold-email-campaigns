use crate::state::AppState;

/// Enqueues due work. Campaign step scheduling arrives in Phase 3.
pub async fn run(_state: AppState) -> anyhow::Result<()> {
    tracing::info!("scheduler started (nothing to enqueue yet)");
    std::future::pending::<()>().await;
    Ok(())
}
