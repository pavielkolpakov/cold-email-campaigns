use anyhow::{Result, anyhow};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::hash_password;

#[derive(Debug, serde::Serialize)]
pub struct Invite {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub token: Uuid,
    pub accepted_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn create(
    pool: &PgPool,
    org_id: Uuid,
    invited_by: Uuid,
    email: &str,
    role: &str,
) -> Result<Invite> {
    let email = email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(anyhow!("a valid email is required"));
    }
    if sqlx::query_scalar!(
        "select exists (select 1 from users where lower(email) = $1)",
        email
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false)
    {
        return Err(anyhow!("that email already has an account"));
    }

    let invite = sqlx::query_as!(
        Invite,
        r#"
        insert into invites (org_id, email, role, invited_by)
        values ($1, $2, $3, $4)
        returning id, email, role, token, accepted_at
        "#,
        org_id,
        email,
        role,
        invited_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(invite)
}

pub async fn list(pool: &PgPool, org_id: Uuid) -> Result<Vec<Invite>> {
    let invites = sqlx::query_as!(
        Invite,
        r#"
        select id, email, role, token, accepted_at
        from invites
        where org_id = $1
        order by created_at desc
        "#,
        org_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(invites)
}

/// Turns an invite into a user. The invite decides the org and the email, so an
/// accepted invite can never land someone in a different org than intended.
pub async fn accept(pool: &PgPool, token: &str, name: &str, password: &str) -> Result<Uuid> {
    let token = Uuid::parse_str(token.trim()).map_err(|_| anyhow!("invalid invitation"))?;
    if password.chars().count() < 10 {
        return Err(anyhow!("password must be at least 10 characters"));
    }

    let mut tx = pool.begin().await?;

    let invite = sqlx::query!(
        r#"
        select id, org_id, email, role
        from invites
        where token = $1 and accepted_at is null and expires_at > now()
        for update
        "#,
        token,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| anyhow!("this invitation has expired or was already used"))?;

    let password_hash = hash_password(password).map_err(|err| anyhow!("{err}"))?;

    let user_id = sqlx::query_scalar!(
        r#"
        insert into users (org_id, email, password_hash, name, role)
        values ($1, $2, $3, $4, $5)
        returning id
        "#,
        invite.org_id,
        invite.email,
        password_hash,
        name.trim(),
        invite.role,
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        "update invites set accepted_at = now() where id = $1",
        invite.id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(user_id)
}

pub async fn revoke(pool: &PgPool, org_id: Uuid, invite_id: Uuid) -> Result<()> {
    let deleted = sqlx::query!(
        "delete from invites where id = $1 and org_id = $2 and accepted_at is null",
        invite_id,
        org_id,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if deleted == 0 {
        return Err(anyhow!("invitation not found"));
    }
    Ok(())
}
