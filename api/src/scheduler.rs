use anyhow::Result;
use sqlx::PgPool;
use std::time::Duration;

use crate::state::AppState;

const TICK: Duration = Duration::from_secs(10);

/// Turns due sequence steps into send jobs. Claiming the lead (clearing
/// `next_run_at`) in the same statement that writes the job is what stops a
/// second tick from enqueueing the same send twice.
pub async fn enqueue_due(pool: &PgPool) -> Result<usize> {
    let enqueued = sqlx::query!(
        r#"
        with due as (
            update campaign_leads cl
            set next_run_at = null
            from campaigns c
            where cl.campaign_id = c.id
              and c.status = 'running'
              and cl.status = 'pending'
              and cl.next_run_at is not null
              and cl.next_run_at <= now()
            returning cl.id, c.org_id
        )
        insert into jobs (org_id, kind, payload)
        select due.org_id, 'send', jsonb_build_object('campaign_lead_id', due.id)
        from due
        "#,
    )
    .execute(pool)
    .await?
    .rows_affected();

    Ok(enqueued as usize)
}

pub async fn run(state: AppState) -> Result<()> {
    tracing::info!("scheduler started");
    loop {
        match enqueue_due(&state.pool).await {
            Ok(0) => {}
            Ok(count) => tracing::info!(count, "enqueued send jobs"),
            Err(error) => tracing::error!(?error, "scheduler tick failed"),
        }
        tokio::time::sleep(TICK).await;
    }
}
