use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto::Cipher;
use crate::inbound::{Classification, InboundMessage, classify};
use crate::mailboxes;
use crate::provider::{InboxReader, TokenRefresher};
use crate::suppressions;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub replies: usize,
    pub bounces: usize,
    pub ignored: usize,
}

/// Reads a mailbox and stops the sequence for anyone who answered.
pub async fn sync_mailbox(
    pool: &PgPool,
    cipher: &Cipher,
    refresher: &dyn TokenRefresher,
    reader: &dyn InboxReader,
    org_id: Uuid,
    mailbox_id: Uuid,
) -> Result<SyncReport> {
    let credentials =
        mailboxes::fresh_credentials(pool, cipher, refresher, org_id, mailbox_id).await?;

    let cursor = sqlx::query_scalar!(
        "select sync_cursor from mailboxes where id = $1 and org_id = $2",
        mailbox_id,
        org_id,
    )
    .fetch_one(pool)
    .await?;

    let page = reader.fetch(&credentials, cursor.as_deref()).await?;
    let mut report = SyncReport::default();

    for message in &page.messages {
        match apply(pool, org_id, mailbox_id, message).await? {
            Some(Classification::Reply) => report.replies += 1,
            Some(Classification::Bounce) => report.bounces += 1,
            _ => report.ignored += 1,
        }
    }

    sqlx::query!(
        "update mailboxes set sync_cursor = $1, last_synced_at = now() where id = $2 and org_id = $3",
        page.cursor,
        mailbox_id,
        org_id,
    )
    .execute(pool)
    .await?;

    Ok(report)
}

/// Returns the classification only when the message actually belonged to a
/// campaign we are running, so unrelated inbox traffic is simply ignored.
async fn apply(
    pool: &PgPool,
    org_id: Uuid,
    mailbox_id: Uuid,
    message: &InboundMessage,
) -> Result<Option<Classification>> {
    // The thread ties an inbound message to the lead we mailed. Scoped by
    // mailbox and org so one tenant's sync can never touch another's campaign.
    let matched = sqlx::query!(
        r#"
        select cl.id, l.email
        from campaign_leads cl
        join campaigns c on c.id = cl.campaign_id
        join leads l on l.id = cl.lead_id
        where cl.thread_id = $1
          and c.org_id = $2
          and c.mailbox_id = $3
          and cl.status not in ('replied', 'bounced')
        "#,
        message.thread_id,
        org_id,
        mailbox_id,
    )
    .fetch_optional(pool)
    .await?;

    let Some(matched) = matched else {
        return Ok(None);
    };

    // Our own sent copy shows up in the same thread; it is not an answer.
    if matched.email.to_lowercase() != message.from.to_lowercase()
        && !message.from.to_lowercase().contains("mailer-daemon")
        && !message.from.to_lowercase().contains("postmaster@")
    {
        return Ok(None);
    }

    let classification = classify(message);

    match classification {
        Classification::Reply => {
            sqlx::query!(
                r#"
                update campaign_leads
                set status = 'replied', replied_at = now(), next_run_at = null,
                    reply_message_id = $2
                where id = $1
                "#,
                matched.id,
                message.provider_message_id,
            )
            .execute(pool)
            .await?;
            cancel_queued_jobs(pool, matched.id).await?;
        }
        Classification::Bounce => {
            sqlx::query!(
                r#"
                update campaign_leads
                set status = 'bounced', bounced_at = now(), next_run_at = null,
                    reply_message_id = $2
                where id = $1
                "#,
                matched.id,
                message.provider_message_id,
            )
            .execute(pool)
            .await?;
            cancel_queued_jobs(pool, matched.id).await?;
            // A dead address must not be mailed by any future campaign either.
            suppressions::add(pool, org_id, &matched.email, "bounced").await?;
        }
        // An out-of-office changes nothing: they have not read it.
        Classification::AutoReply => {}
    }

    Ok(Some(classification))
}

/// A job already sitting in the queue would otherwise still go out.
async fn cancel_queued_jobs(pool: &PgPool, campaign_lead_id: Uuid) -> Result<()> {
    sqlx::query!(
        r#"
        update jobs
        set status = 'done', last_error = 'cancelled: lead is no longer mailable'
        where status = 'pending'
          and payload ->> 'campaign_lead_id' = $1::text
        "#,
        campaign_lead_id.to_string(),
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// A mailbox due for sync, already claimed by this caller.
pub struct DueMailbox {
    pub org_id: Uuid,
    pub mailbox_id: Uuid,
}

/// Claims mailboxes that have not been synced within `interval`. Stamping
/// `last_synced_at` in the same statement that selects them is what keeps two
/// workers off the same mailbox — no separate queue needed, since a sync is
/// idempotent and there is at most one outstanding per mailbox.
pub async fn claim_due_mailboxes(
    pool: &PgPool,
    interval: chrono::Duration,
) -> Result<Vec<DueMailbox>> {
    let rows = sqlx::query!(
        r#"
        update mailboxes
        set last_synced_at = now()
        where id in (
            select id from mailboxes
            where status = 'active'
              and (last_synced_at is null or last_synced_at < now() - $1::interval)
            for update skip locked
        )
        returning org_id, id
        "#,
        interval as _,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| DueMailbox {
            org_id: row.org_id,
            mailbox_id: row.id,
        })
        .collect())
}
