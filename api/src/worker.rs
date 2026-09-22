use crate::state::AppState;

/// Claims and runs jobs. Job kinds arrive in Phase 3.
pub async fn run(_state: AppState) -> anyhow::Result<()> {
    tracing::info!("worker started (no job kinds registered yet)");
    std::future::pending::<()>().await;
    Ok(())
}
