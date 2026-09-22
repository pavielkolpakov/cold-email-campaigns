use anyhow::{Result, anyhow};
use chrono::{Duration, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto::Cipher;
use crate::mailboxes;
use crate::provider::{Mailer, MailerError, OutboundMessage, TokenRefresher};
use crate::render::{self, MergeValues};
use crate::state::AppState;

const TICK: std::time::Duration = std::time::Duration::from_secs(5);
/// How stale a mailbox's inbox may get before it is read again.
const SYNC_INTERVAL_MINUTES: i64 = 2;
const MAX_ATTEMPTS: i32 = 5;
const DEFAULT_APP_URL: &str = "http://localhost:3000";

/// Retrying only helps when the cause can change. A template that references a
/// value the lead does not have will fail identically forever, so it stops now.
#[derive(Debug, thiserror::Error)]
enum SendError {
    #[error("{0}")]
    Permanent(String),
    /// Throttled. Not a failure of ours, so it neither fails the lead nor
    /// spends one of its retries.
    #[error("rate limited")]
    RateLimited { retry_after: Option<Duration> },
    /// The mailbox stopped being usable mid-send.
    #[error("the mailbox is no longer authorized")]
    Unauthorized,
    #[error(transparent)]
    Transient(#[from] anyhow::Error),
}

impl From<MailerError> for SendError {
    fn from(err: MailerError) -> Self {
        match err {
            MailerError::RateLimited { retry_after } => Self::RateLimited {
                retry_after: retry_after.and_then(|after| Duration::from_std(after).ok()),
            },
            MailerError::InvalidRecipient(message) => Self::Permanent(message),
            MailerError::Unauthorized => Self::Unauthorized,
            MailerError::Other(err) => Self::Transient(err),
        }
    }
}

impl From<sqlx::Error> for SendError {
    fn from(err: sqlx::Error) -> Self {
        Self::Transient(err.into())
    }
}

/// Everything one send needs, gathered in a single query so the worker does not
/// hold a transaction open while talking to Gmail.
struct SendJob {
    job_id: Uuid,
    attempts: i32,
    campaign_lead_id: Uuid,
    org_id: Uuid,
    mailbox_id: Uuid,
    mailbox_email: String,
    daily_cap: i32,
    lead_email: String,
    unsubscribe_token: Uuid,
    step_id: Uuid,
    position: i32,
    subject: String,
    body: String,
    thread_id: Option<String>,
    /// Message-ID and subject of the message that opened this thread, so a
    /// followup can reply to it rather than start a new conversation.
    reply_to_message_id: Option<String>,
    thread_subject: Option<String>,
    merge_values: MergeValues,
}

pub async fn run_once(
    pool: &PgPool,
    cipher: &Cipher,
    refresher: &dyn TokenRefresher,
    mailer: &dyn Mailer,
    worker_id: &str,
    limit: i64,
) -> Result<usize> {
    run_once_with_url(
        pool,
        cipher,
        refresher,
        mailer,
        DEFAULT_APP_URL,
        worker_id,
        limit,
    )
    .await
}

/// Same as [`run_once`], with the base url that unsubscribe links point at.
#[allow(clippy::too_many_arguments)]
pub async fn run_once_with_url(
    pool: &PgPool,
    cipher: &Cipher,
    refresher: &dyn TokenRefresher,
    mailer: &dyn Mailer,
    app_url: &str,
    worker_id: &str,
    limit: i64,
) -> Result<usize> {
    let jobs = claim(pool, worker_id, limit).await?;
    let mut processed = 0;

    for job in jobs {
        match send_one(pool, cipher, refresher, mailer, app_url, &job).await {
            Ok(()) => {
                sqlx::query!("update jobs set status = 'done' where id = $1", job.job_id)
                    .execute(pool)
                    .await?;
                processed += 1;
            }
            Err(SendError::RateLimited { retry_after }) => {
                defer(pool, &job, retry_after.unwrap_or(Duration::minutes(5))).await?;
            }
            Err(SendError::Unauthorized) => {
                // Every other send from this mailbox will fail the same way.
                sqlx::query!(
                    "update mailboxes set status = 'disconnected', updated_at = now() where id = $1",
                    job.mailbox_id,
                )
                .execute(pool)
                .await?;
                defer(pool, &job, Duration::minutes(5)).await?;
                tracing::error!(mailbox = %job.mailbox_id, "mailbox deauthorized mid-send");
            }
            Err(error) => {
                let permanent = matches!(error, SendError::Permanent(_));
                record_failure(pool, &job, &error.to_string(), permanent).await?;
            }
        }
    }

    Ok(processed)
}

/// `for update skip locked` is what lets several workers share the queue: each
/// takes rows nobody else has locked, rather than queueing behind them.
async fn claim(pool: &PgPool, worker_id: &str, limit: i64) -> Result<Vec<SendJob>> {
    let rows = sqlx::query!(
        r#"
        with claimed as (
            select id from jobs
            where status = 'pending' and scheduled_at <= now() and kind = 'send'
            order by scheduled_at
            for update skip locked
            limit $1
        )
        update jobs j
        set status = 'running', locked_at = now(), locked_by = $2, attempts = j.attempts + 1
        from claimed
        where j.id = claimed.id
        returning
            j.id as job_id,
            j.attempts,
            j.payload
        "#,
        limit,
        worker_id,
    )
    .fetch_all(pool)
    .await?;

    let mut jobs = Vec::new();
    for row in rows {
        let campaign_lead_id = row
            .payload
            .get("campaign_lead_id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(|| anyhow!("job {} has no campaign_lead_id", row.job_id))?;

        let Some(job) = load(pool, row.job_id, row.attempts, campaign_lead_id).await? else {
            // The lead replied or was suppressed between scheduling and now.
            sqlx::query!("update jobs set status = 'done' where id = $1", row.job_id)
                .execute(pool)
                .await?;
            continue;
        };
        jobs.push(job);
    }

    Ok(jobs)
}

async fn load(
    pool: &PgPool,
    job_id: Uuid,
    attempts: i32,
    campaign_lead_id: Uuid,
) -> Result<Option<SendJob>> {
    let row = sqlx::query!(
        r#"
        select
            c.org_id,
            c.mailbox_id,
            m.email       as mailbox_email,
            m.daily_cap,
            l.email       as lead_email,
            l.unsubscribe_token,
            l.first_name,
            l.last_name,
            l.company,
            l.custom,
            cl.current_step,
            cl.thread_id,
            s.id          as step_id,
            s.position,
            s.subject,
            s.body
        from campaign_leads cl
        join campaigns c on c.id = cl.campaign_id
        join mailboxes m on m.id = c.mailbox_id
        join leads l on l.id = cl.lead_id
        join sequence_steps s
          on s.sequence_id = c.sequence_id and s.position = cl.current_step
        where cl.id = $1
          and cl.status = 'pending'
          and c.status = 'running'
          and m.status = 'active'
          and not exists (
              select 1 from suppressions sup
              where sup.org_id = c.org_id and lower(sup.email) = lower(l.email)
          )
        "#,
        campaign_lead_id,
    )
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    // The opening send of this thread; absent on the first step.
    let opener = sqlx::query!(
        r#"
        select message_id_header, subject
        from messages
        where campaign_lead_id = $1
        order by sent_at
        limit 1
        "#,
        campaign_lead_id,
    )
    .fetch_optional(pool)
    .await?;

    let lead = crate::leads::Lead {
        id: campaign_lead_id,
        email: row.lead_email.clone(),
        first_name: row.first_name,
        last_name: row.last_name,
        company: row.company,
        custom: row.custom,
        status: "active".into(),
    };

    Ok(Some(SendJob {
        job_id,
        attempts,
        campaign_lead_id,
        org_id: row.org_id,
        mailbox_id: row.mailbox_id,
        mailbox_email: row.mailbox_email,
        daily_cap: row.daily_cap,
        lead_email: row.lead_email,
        unsubscribe_token: row.unsubscribe_token,
        step_id: row.step_id,
        position: row.position,
        subject: row.subject,
        body: row.body,
        thread_id: row.thread_id,
        reply_to_message_id: opener.as_ref().and_then(|m| m.message_id_header.clone()),
        thread_subject: opener.map(|m| m.subject),
        merge_values: lead.merge_values(),
    }))
}

async fn send_one(
    pool: &PgPool,
    cipher: &Cipher,
    refresher: &dyn TokenRefresher,
    mailer: &dyn Mailer,
    app_url: &str,
    job: &SendJob,
) -> Result<(), SendError> {
    // Transient by design: the cap lifts at midnight, so the job waits.
    if sent_today(pool, job.mailbox_id).await? >= i64::from(job.daily_cap) {
        return Err(SendError::Transient(anyhow!(
            "daily cap reached for {}",
            job.mailbox_email
        )));
    }

    // A half-filled merge tag is a worse outcome than a send that never happens.
    let subject = render_field(&job.subject, &job.merge_values)?;
    let body = with_unsubscribe(
        render_field(&job.body, &job.merge_values)?,
        app_url,
        job.unsubscribe_token,
    );

    let credentials =
        mailboxes::fresh_credentials(pool, cipher, refresher, job.org_id, job.mailbox_id).await?;

    // A followup has no subject of its own: it reuses the opening one, which is
    // also what mail clients group on. Resolved once, so the row we store is
    // what actually went out.
    let subject = match (subject.is_empty(), &job.thread_subject) {
        (true, Some(original)) => format!("Re: {original}"),
        _ => subject,
    };

    let sent = mailer
        .send(
            &job.mailbox_email,
            &credentials,
            &OutboundMessage {
                to: job.lead_email.clone(),
                subject: subject.clone(),
                body: body.clone(),
                thread_id: job.thread_id.clone(),
                in_reply_to: job.reply_to_message_id.clone(),
            },
        )
        .await?;

    let mut tx = pool.begin().await?;

    sqlx::query!(
        r#"
        insert into messages
            (campaign_lead_id, step_id, provider_message_id, thread_id, message_id_header,
             subject, body)
        values ($1, $2, $3, $4, $5, $6, $7)
        "#,
        job.campaign_lead_id,
        job.step_id,
        sent.provider_message_id,
        sent.thread_id,
        sent.message_id_header,
        subject,
        body,
    )
    .execute(&mut *tx)
    .await?;

    advance(&mut tx, job, &sent.thread_id).await?;
    tx.commit().await?;

    Ok(())
}

/// Moves the lead to the next step, or finishes them if that was the last one.
async fn advance(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job: &SendJob,
    thread_id: &str,
) -> Result<()> {
    let next = sqlx::query!(
        r#"
        select s.delay_days
        from campaign_leads cl
        join campaigns c on c.id = cl.campaign_id
        join sequence_steps s
          on s.sequence_id = c.sequence_id and s.position = $2
        where cl.id = $1
        "#,
        job.campaign_lead_id,
        job.position + 1,
    )
    .fetch_optional(&mut **tx)
    .await?;

    match next {
        Some(next) => {
            let due = Utc::now() + Duration::days(i64::from(next.delay_days));
            sqlx::query!(
                r#"
                update campaign_leads
                set current_step = $2, next_run_at = $3, thread_id = coalesce(thread_id, $4)
                where id = $1
                "#,
                job.campaign_lead_id,
                job.position + 1,
                due,
                thread_id,
            )
            .execute(&mut **tx)
            .await?;
        }
        None => {
            sqlx::query!(
                r#"
                update campaign_leads
                set status = 'finished', next_run_at = null, thread_id = coalesce(thread_id, $2)
                where id = $1
                "#,
                job.campaign_lead_id,
                thread_id,
            )
            .execute(&mut **tx)
            .await?;
        }
    }

    Ok(())
}

/// CAN-SPAM and GDPR both require a working opt-out, so it is appended here
/// rather than trusted to whoever wrote the template. `{{unsubscribe_url}}`
/// places it explicitly instead.
fn with_unsubscribe(body: String, app_url: &str, token: Uuid) -> String {
    let link = format!("{}/u/{token}", app_url.trim_end_matches('/'));
    if body.contains("{{unsubscribe_url}}") {
        return body.replace("{{unsubscribe_url}}", &link);
    }
    format!("{body}\n\n---\nDon't want these emails? Unsubscribe: {link}")
}

fn render_field(template: &str, values: &MergeValues) -> Result<String, SendError> {
    render::render(template, values).map_err(|unfilled| {
        SendError::Permanent(format!(
            "merge tags with no value or fallback: {}",
            unfilled.join(", ")
        ))
    })
}

async fn sent_today(pool: &PgPool, mailbox_id: Uuid) -> Result<i64> {
    let count = sqlx::query_scalar!(
        r#"
        select count(*) from messages m
        join campaign_leads cl on cl.id = m.campaign_lead_id
        join campaigns c on c.id = cl.campaign_id
        where c.mailbox_id = $1 and m.sent_at >= date_trunc('day', now())
        "#,
        mailbox_id,
    )
    .fetch_one(pool)
    .await?;
    Ok(count.unwrap_or(0))
}

/// Puts a job back without spending an attempt, for when the provider asked us
/// to wait rather than the send being wrong.
async fn defer(pool: &PgPool, job: &SendJob, wait: Duration) -> Result<()> {
    sqlx::query!(
        r#"
        update jobs
        set status = 'pending', scheduled_at = $2, locked_at = null, locked_by = null,
            attempts = greatest(attempts - 1, 0),
            last_error = 'deferred: provider asked us to wait'
        where id = $1
        "#,
        job.job_id,
        Utc::now() + wait,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Retries with a widening backoff, then gives up and surfaces the error.
async fn record_failure(pool: &PgPool, job: &SendJob, error: &str, permanent: bool) -> Result<()> {
    if permanent || job.attempts >= MAX_ATTEMPTS {
        sqlx::query!(
            "update jobs set status = 'failed', last_error = $2 where id = $1",
            job.job_id,
            error,
        )
        .execute(pool)
        .await?;
        sqlx::query!(
            "update campaign_leads set status = 'failed', next_run_at = null, last_error = $2 where id = $1",
            job.campaign_lead_id,
            error,
        )
        .execute(pool)
        .await?;
        tracing::error!(job = %job.job_id, error, "giving up on send");
        return Ok(());
    }

    let backoff = Duration::minutes(2_i64.pow(job.attempts.min(8) as u32));
    sqlx::query!(
        r#"
        update jobs
        set status = 'pending', scheduled_at = $2, locked_at = null, locked_by = null,
            last_error = $3
        where id = $1
        "#,
        job.job_id,
        Utc::now() + backoff,
        error,
    )
    .execute(pool)
    .await?;
    tracing::warn!(job = %job.job_id, attempts = job.attempts, error, "send failed, will retry");

    Ok(())
}

pub async fn run(state: AppState) -> Result<()> {
    let worker_id = format!("worker-{}", Uuid::new_v4());
    tracing::info!(%worker_id, "worker started");

    loop {
        match run_once_with_url(
            &state.pool,
            &state.cipher,
            state.oauth.as_ref(),
            state.mailer.as_ref(),
            &state.config.app_url,
            &worker_id,
            10,
        )
        .await
        {
            Ok(0) => {}
            Ok(count) => tracing::info!(count, "sent"),
            Err(error) => tracing::error!(?error, "worker tick failed"),
        }

        if let Err(error) = sync_inboxes(&state).await {
            tracing::error!(?error, "inbox sync failed");
        }

        tokio::time::sleep(TICK).await;
    }
}

/// Reads every mailbox that has gone unchecked for a while, so a reply stops
/// the sequence within a couple of minutes.
async fn sync_inboxes(state: &AppState) -> Result<()> {
    let due =
        crate::sync::claim_due_mailboxes(&state.pool, Duration::minutes(SYNC_INTERVAL_MINUTES))
            .await?;

    for mailbox in due {
        match crate::sync::sync_mailbox(
            &state.pool,
            &state.cipher,
            state.oauth.as_ref(),
            state.inbox.as_ref(),
            mailbox.org_id,
            mailbox.mailbox_id,
        )
        .await
        {
            Ok(report) if report.replies > 0 || report.bounces > 0 => {
                tracing::info!(
                    replies = report.replies,
                    bounces = report.bounces,
                    mailbox = %mailbox.mailbox_id,
                    "inbox synced"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::error!(mailbox = %mailbox.mailbox_id, ?error, "mailbox sync failed")
            }
        }
    }

    Ok(())
}
