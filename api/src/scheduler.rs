use anyhow::Result;
use sqlx::PgPool;
use std::time::Duration;

use crate::state::AppState;

const TICK: Duration = Duration::from_secs(10);
/// How long a job may sit claimed before its worker is presumed dead. Well
/// above a slow Gmail call, well below a delay anyone would notice.
const LEASE_MINUTES: i64 = 10;

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

        match requeue_stale_jobs(&state.pool, chrono::Duration::minutes(LEASE_MINUTES)).await {
            Ok(0) => {}
            Ok(count) => tracing::warn!(count, "requeued jobs from a stopped worker"),
            Err(error) => tracing::error!(?error, "stale job sweep failed"),
        }
        tokio::time::sleep(TICK).await;
    }
}

/// Returns jobs whose worker died mid-send. Without this a killed worker
/// silently swallows every send it had claimed.
pub async fn requeue_stale_jobs(pool: &PgPool, lease: chrono::Duration) -> Result<usize> {
    // Out of attempts: requeueing forever would just orphan it again.
    sqlx::query!(
        r#"
        update jobs
        set status = 'failed',
            last_error = 'abandoned by its worker and out of attempts'
        where status = 'running'
          and locked_at < now() - $1::interval
          and attempts >= 5
        "#,
        lease as _,
    )
    .execute(pool)
    .await?;

    let requeued = sqlx::query!(
        r#"
        update jobs
        set status = 'pending', locked_at = null, locked_by = null,
            last_error = 'requeued after its worker stopped responding'
        where status = 'running'
          and locked_at < now() - $1::interval
        "#,
        lease as _,
    )
    .execute(pool)
    .await?
    .rows_affected();

    Ok(requeued as usize)
}
