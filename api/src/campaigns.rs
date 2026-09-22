use anyhow::{Result, anyhow};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Campaign {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub sequence_id: Uuid,
    pub list_id: Uuid,
    pub mailbox_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct NewCampaign {
    pub name: String,
    pub sequence_id: Uuid,
    pub list_id: Uuid,
    pub mailbox_id: Uuid,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct Stats {
    pub total: i64,
    pub pending: i64,
    pub sent: i64,
    pub replied: i64,
    pub bounced: i64,
    pub failed: i64,
}

pub async fn create(pool: &PgPool, org_id: Uuid, new: NewCampaign) -> Result<Campaign> {
    // Sequence, list and mailbox all arrive from the request, so each is
    // confirmed to belong to the caller before the campaign exists.
    let owned = sqlx::query_scalar!(
        r#"
        select
            exists (select 1 from sequences where id = $1 and org_id = $4)
            and exists (select 1 from lead_lists where id = $2 and org_id = $4)
            and exists (select 1 from mailboxes where id = $3 and org_id = $4)
        "#,
        new.sequence_id,
        new.list_id,
        new.mailbox_id,
        org_id,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    if !owned {
        return Err(anyhow!(
            "sequence, list and mailbox must all belong to this organization"
        ));
    }

    let campaign = sqlx::query_as!(
        Campaign,
        r#"
        insert into campaigns (org_id, name, sequence_id, list_id, mailbox_id)
        values ($1, $2, $3, $4, $5)
        returning id, org_id, name, sequence_id, list_id, mailbox_id, status
        "#,
        org_id,
        new.name.trim(),
        new.sequence_id,
        new.list_id,
        new.mailbox_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(campaign)
}

/// Snapshots the list into the campaign and makes the first step due now.
/// Returns how many leads were enrolled.
pub async fn launch(pool: &PgPool, org_id: Uuid, campaign_id: Uuid) -> Result<usize> {
    let campaign = get(pool, org_id, campaign_id).await?;

    let enrolled = sqlx::query!(
        r#"
        insert into campaign_leads (campaign_id, lead_id, current_step, next_run_at)
        select $1, l.id, 0, now()
        from leads l
        where l.list_id = $2
          and l.org_id = $3
          and l.status = 'active'
          and not exists (
              select 1 from suppressions s
              where s.org_id = l.org_id and lower(s.email) = lower(l.email)
          )
        on conflict (campaign_id, lead_id) do nothing
        "#,
        campaign.id,
        campaign.list_id,
        org_id,
    )
    .execute(pool)
    .await?
    .rows_affected();

    sqlx::query!(
        "update campaigns set status = 'running', started_at = coalesce(started_at, now()) where id = $1",
        campaign.id,
    )
    .execute(pool)
    .await?;

    Ok(enrolled as usize)
}

pub async fn get(pool: &PgPool, org_id: Uuid, campaign_id: Uuid) -> Result<Campaign> {
    let campaign = sqlx::query_as!(
        Campaign,
        r#"
        select id, org_id, name, sequence_id, list_id, mailbox_id, status
        from campaigns
        where id = $1 and org_id = $2
        "#,
        campaign_id,
        org_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("campaign not found"))?;
    Ok(campaign)
}

pub async fn stats(pool: &PgPool, org_id: Uuid, campaign_id: Uuid) -> Result<Stats> {
    get(pool, org_id, campaign_id).await?;

    let row = sqlx::query!(
        r#"
        select
            count(*)                                            as "total!",
            count(*) filter (where status = 'pending')          as "pending!",
            count(*) filter (where status in ('sent', 'finished')) as "sent!",
            count(*) filter (where status = 'replied')          as "replied!",
            count(*) filter (where status = 'bounced')          as "bounced!",
            count(*) filter (where status = 'failed')           as "failed!"
        from campaign_leads
        where campaign_id = $1
        "#,
        campaign_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(Stats {
        total: row.total,
        pending: row.pending,
        sent: row.sent,
        replied: row.replied,
        bounced: row.bounced,
        failed: row.failed,
    })
}

pub async fn pause(pool: &PgPool, org_id: Uuid, campaign_id: Uuid) -> Result<()> {
    set_status(pool, org_id, campaign_id, "paused").await
}

pub async fn resume(pool: &PgPool, org_id: Uuid, campaign_id: Uuid) -> Result<()> {
    set_status(pool, org_id, campaign_id, "running").await
}

async fn set_status(pool: &PgPool, org_id: Uuid, campaign_id: Uuid, status: &str) -> Result<()> {
    let updated = sqlx::query!(
        "update campaigns set status = $1 where id = $2 and org_id = $3",
        status,
        campaign_id,
        org_id,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if updated == 0 {
        return Err(anyhow!("campaign not found"));
    }
    Ok(())
}

pub async fn list(pool: &PgPool, org_id: Uuid) -> Result<Vec<Campaign>> {
    let campaigns = sqlx::query_as!(
        Campaign,
        r#"
        select id, org_id, name, sequence_id, list_id, mailbox_id, status
        from campaigns
        where org_id = $1
        order by created_at desc
        "#,
        org_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(campaigns)
}
