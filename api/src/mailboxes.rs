use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto::Cipher;
use crate::provider::{Mailer, OutboundMessage, RefreshError, SentMessage, TokenRefresher};

#[derive(Debug, Clone)]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewMailbox {
    pub provider: String,
    pub email: String,
    pub credentials: Credentials,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Mailbox {
    pub id: Uuid,
    pub org_id: Uuid,
    pub provider: String,
    pub email: String,
    pub status: String,
    pub daily_cap: i32,
}

pub async fn connect(
    pool: &PgPool,
    cipher: &Cipher,
    org_id: Uuid,
    mailbox: NewMailbox,
) -> Result<Mailbox> {
    let email = mailbox.email.trim().to_lowercase();
    let access_token_enc = cipher.encrypt(&mailbox.credentials.access_token)?;
    let refresh_token_enc = cipher.encrypt(&mailbox.credentials.refresh_token)?;

    let record = sqlx::query_as!(
        Mailbox,
        r#"
        insert into mailboxes
            (org_id, provider, email, access_token_enc, refresh_token_enc, token_expires_at)
        values ($1, $2, $3, $4, $5, $6)
        returning id, org_id, provider, email, status, daily_cap
        "#,
        org_id,
        mailbox.provider,
        email,
        access_token_enc,
        refresh_token_enc,
        mailbox.credentials.expires_at,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

pub async fn credentials(
    pool: &PgPool,
    cipher: &Cipher,
    org_id: Uuid,
    mailbox_id: Uuid,
) -> Result<Credentials> {
    let record = sqlx::query!(
        r#"
        select access_token_enc, refresh_token_enc, token_expires_at
        from mailboxes
        where id = $1 and org_id = $2
        "#,
        mailbox_id,
        org_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("mailbox not found"))?;

    Ok(Credentials {
        access_token: cipher.decrypt(&record.access_token_enc)?,
        refresh_token: cipher.decrypt(&record.refresh_token_enc)?,
        expires_at: record.token_expires_at,
    })
}

pub async fn list(pool: &PgPool, org_id: Uuid) -> Result<Vec<Mailbox>> {
    let mailboxes = sqlx::query_as!(
        Mailbox,
        r#"
        select id, org_id, provider, email, status, daily_cap
        from mailboxes
        where org_id = $1
        order by email
        "#,
        org_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(mailboxes)
}

/// Credentials guaranteed to be usable right now: refreshes and persists a new
/// access token when the stored one is at or near expiry.
pub async fn fresh_credentials(
    pool: &PgPool,
    cipher: &Cipher,
    refresher: &dyn TokenRefresher,
    org_id: Uuid,
    mailbox_id: Uuid,
) -> Result<Credentials> {
    let current = credentials(pool, cipher, org_id, mailbox_id).await?;

    // Refresh a little early so a token cannot expire mid-send.
    if current.expires_at > Utc::now() + Duration::seconds(60) {
        return Ok(current);
    }

    let refreshed = match refresher.refresh(&current.refresh_token).await {
        Ok(refreshed) => refreshed,
        Err(RefreshError::Revoked) => {
            // Permanent: stop retrying and surface it so the user can reconnect.
            sqlx::query!(
                "update mailboxes set status = 'disconnected', updated_at = now() where id = $1 and org_id = $2",
                mailbox_id,
                org_id,
            )
            .execute(pool)
            .await?;
            return Err(RefreshError::Revoked.into());
        }
        Err(RefreshError::Other(err)) => return Err(err),
    };
    let access_token_enc = cipher.encrypt(&refreshed.access_token)?;

    sqlx::query!(
        r#"
        update mailboxes
        set access_token_enc = $1, token_expires_at = $2, updated_at = now()
        where id = $3 and org_id = $4
        "#,
        access_token_enc,
        refreshed.expires_at,
        mailbox_id,
        org_id,
    )
    .execute(pool)
    .await?;

    Ok(Credentials {
        access_token: refreshed.access_token,
        refresh_token: current.refresh_token,
        expires_at: refreshed.expires_at,
    })
}

/// Proves a mailbox can actually send, by mailing its own address.
pub async fn send_test_email(
    pool: &PgPool,
    cipher: &Cipher,
    refresher: &dyn TokenRefresher,
    mailer: &dyn Mailer,
    org_id: Uuid,
    mailbox_id: Uuid,
) -> Result<SentMessage> {
    let address = sqlx::query_scalar!(
        "select email from mailboxes where id = $1 and org_id = $2",
        mailbox_id,
        org_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("mailbox not found"))?;

    let credentials = fresh_credentials(pool, cipher, refresher, org_id, mailbox_id).await?;

    mailer
        .send(
            &address,
            &credentials,
            &OutboundMessage {
                to: address.clone(),
                subject: "Your mailbox is connected".into(),
                body: "This is a test message confirming the connection works.".into(),
                thread_id: None,
                in_reply_to: None,
            },
        )
        .await
}

pub async fn disconnect(pool: &PgPool, org_id: Uuid, mailbox_id: Uuid) -> Result<()> {
    let deleted = sqlx::query!(
        "delete from mailboxes where id = $1 and org_id = $2",
        mailbox_id,
        org_id,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if deleted == 0 {
        return Err(anyhow!("mailbox not found"));
    }
    Ok(())
}
