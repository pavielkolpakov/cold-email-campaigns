use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, serde::Serialize)]
pub struct MailboxUsage {
    pub id: Uuid,
    pub email: String,
    pub status: String,
    pub daily_cap: i32,
    pub sent_today: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct RecentReply {
    pub lead_email: String,
    pub campaign_name: String,
    pub replied_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, serde::Serialize)]
pub struct Problem {
    pub campaign_id: Uuid,
    pub campaign_name: String,
    pub lead_email: String,
    pub error: String,
}

#[derive(Debug, serde::Serialize)]
pub struct Summary {
    pub active_campaigns: i64,
    pub mailboxes: Vec<MailboxUsage>,
    pub recent_replies: Vec<RecentReply>,
    /// Leads that stopped because something went wrong, newest first.
    pub problems: Vec<Problem>,
}

pub async fn summary(pool: &PgPool, org_id: Uuid) -> Result<Summary> {
    let active_campaigns = sqlx::query_scalar!(
        "select count(*) from campaigns where org_id = $1 and status = 'running'",
        org_id,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    let mailboxes = sqlx::query_as!(
        MailboxUsage,
        r#"
        select
            m.id,
            m.email,
            m.status,
            m.daily_cap,
            (
                select count(*)
                from messages msg
                join campaign_leads cl on cl.id = msg.campaign_lead_id
                join campaigns c on c.id = cl.campaign_id
                where c.mailbox_id = m.id and msg.sent_at >= date_trunc('day', now())
            ) as "sent_today!"
        from mailboxes m
        where m.org_id = $1
        order by m.email
        "#,
        org_id,
    )
    .fetch_all(pool)
    .await?;

    let recent_replies = sqlx::query_as!(
        RecentReply,
        r#"
        select l.email as lead_email, c.name as campaign_name, cl.replied_at as "replied_at!"
        from campaign_leads cl
        join campaigns c on c.id = cl.campaign_id
        join leads l on l.id = cl.lead_id
        where c.org_id = $1 and cl.replied_at is not null
        order by cl.replied_at desc
        limit 10
        "#,
        org_id,
    )
    .fetch_all(pool)
    .await?;

    let problems = sqlx::query_as!(
        Problem,
        r#"
        select c.id as campaign_id, c.name as campaign_name, l.email as lead_email,
               cl.last_error as "error!"
        from campaign_leads cl
        join campaigns c on c.id = cl.campaign_id
        join leads l on l.id = cl.lead_id
        where c.org_id = $1 and cl.status = 'failed' and cl.last_error is not null
        order by c.created_at desc
        limit 10
        "#,
        org_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(Summary {
        active_campaigns,
        mailboxes,
        recent_replies,
        problems,
    })
}
