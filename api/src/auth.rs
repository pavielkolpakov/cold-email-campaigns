use std::str::FromStr;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// The authenticated caller. Every org-scoped query takes its `org_id` from here,
/// never from a request body or path parameter.
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: Uuid,
    pub org_id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
}

pub fn hash_password(password: &str) -> AppResult<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|err| AppError::Internal(anyhow::anyhow!("password hashing failed: {err}")))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    match PasswordHash::from_str(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(err) => {
            tracing::error!(error = ?err, "stored password hash is malformed");
            false
        }
    }
}

/// Finds the account for a Google-verified email, or makes one. A new person
/// gets their own org, the same as a password signup would give them.
pub async fn sign_in_with_google(pool: &PgPool, email: &str, name: &str) -> AppResult<CurrentUser> {
    let email = email.trim().to_lowercase();
    let existing = sqlx::query_as!(
        CurrentUser,
        "select id, org_id, email, name, role from users where lower(email) = $1",
        email
    )
    .fetch_optional(pool)
    .await?;
    if let Some(user) = existing {
        return Ok(user);
    }

    let name = match name.trim() {
        "" => email.split('@').next().unwrap_or_default(),
        name => name,
    };
    let org_name = format!("{name}'s workspace");

    let mut tx = pool.begin().await?;
    let org_id = sqlx::query_scalar!("insert into orgs (name) values ($1) returning id", org_name)
        .fetch_one(&mut *tx)
        .await?;
    let user = sqlx::query_as!(
        CurrentUser,
        r#"
        insert into users (org_id, email, name, role)
        values ($1, $2, $3, 'owner')
        returning id, org_id, email, name, role
        "#,
        org_id,
        email,
        name
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(user)
}

pub async fn create_session(pool: &PgPool, user_id: Uuid, ttl_days: i64) -> AppResult<Uuid> {
    let expires_at = Utc::now() + Duration::days(ttl_days);
    let id = sqlx::query_scalar!(
        "insert into sessions (user_id, expires_at) values ($1, $2) returning id",
        user_id,
        expires_at
    )
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn delete_session(pool: &PgPool, id: Uuid) -> AppResult<()> {
    sqlx::query!("delete from sessions where id = $1", id)
        .execute(pool)
        .await?;
    Ok(())
}

pub fn session_cookie(config: &Config, value: String) -> Cookie<'static> {
    let mut cookie = Cookie::new(config.session_cookie_name.clone(), value);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(config.secure_cookies());
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::days(config.session_ttl_days));
    cookie
}

pub fn clear_session_cookie(config: &Config) -> Cookie<'static> {
    let mut cookie = Cookie::new(config.session_cookie_name.clone(), "");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(config.secure_cookies());
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::ZERO);
    cookie
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let raw = jar
            .get(&state.config.session_cookie_name)
            .ok_or(AppError::Unauthorized)?;
        let session_id = Uuid::parse_str(raw.value()).map_err(|_| AppError::Unauthorized)?;

        sqlx::query_as!(
            CurrentUser,
            r#"
            select u.id, u.org_id, u.email, u.name, u.role
            from sessions s
            join users u on u.id = s.user_id
            where s.id = $1 and s.expires_at > now()
            "#,
            session_id
        )
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::Unauthorized)
    }
}
