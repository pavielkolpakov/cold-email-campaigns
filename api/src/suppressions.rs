use anyhow::{Result, anyhow};
use sqlx::PgPool;
use uuid::Uuid;

/// Adds an address to the org's do-not-contact list. Idempotent, because the
/// same unsubscribe link can be clicked twice.
pub async fn add(pool: &PgPool, org_id: Uuid, email: &str, reason: &str) -> Result<()> {
    let email = email.trim().to_lowercase();

    sqlx::query!(
        r#"
        insert into suppressions (org_id, email, reason)
        values ($1, $2, $3)
        on conflict (org_id, lower(email)) do nothing
        "#,
        org_id,
        email,
        reason,
    )
    .execute(pool)
    .await?;

    // Stop any campaign already in flight for this person.
    sqlx::query!(
        r#"
        update campaign_leads
        set status = 'finished', next_run_at = null
        from leads l
        where campaign_leads.lead_id = l.id
          and l.org_id = $1
          and lower(l.email) = $2
          and campaign_leads.status = 'pending'
        "#,
        org_id,
        email,
    )
    .execute(pool)
    .await?;

    sqlx::query!(
        "update leads set status = 'unsubscribed' where org_id = $1 and lower(email) = $2",
        org_id,
        email,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn is_suppressed(pool: &PgPool, org_id: Uuid, email: &str) -> Result<bool> {
    let suppressed = sqlx::query_scalar!(
        "select exists (select 1 from suppressions where org_id = $1 and lower(email) = $2)",
        org_id,
        email.trim().to_lowercase(),
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    Ok(suppressed)
}

pub async fn list(pool: &PgPool, org_id: Uuid) -> Result<Vec<String>> {
    let emails = sqlx::query_scalar!(
        "select email from suppressions where org_id = $1 order by created_at desc",
        org_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(emails)
}

/// Honours an unsubscribe link. The token identifies the lead on its own, so no
/// session is needed — a recipient clicking from their inbox is not signed in.
pub async fn unsubscribe(pool: &PgPool, token: &str) -> Result<()> {
    let token = Uuid::parse_str(token.trim()).map_err(|_| anyhow!("invalid unsubscribe link"))?;

    let lead = sqlx::query!(
        "select org_id, email from leads where unsubscribe_token = $1",
        token,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("invalid unsubscribe link"))?;

    add(pool, lead.org_id, &lead.email, "unsubscribed").await
}
