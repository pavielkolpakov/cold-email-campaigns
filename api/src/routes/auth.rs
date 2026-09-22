use axum::Router;
use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::{get, post};

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/auth/google/authorize", get(google_authorize))
        .route("/auth/google/callback", get(google_callback))
}

use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::{
    CurrentUser, clear_session_cookie, create_session, delete_session, hash_password,
    session_cookie, sign_in_with_google, verify_password,
};
use crate::error::{AppError, AppResult};

pub async fn health(State(state): State<AppState>) -> AppResult<Json<Value>> {
    sqlx::query_scalar!("select 1").fetch_one(&state.pool).await?;
    Ok(Json(json!({ "status": "ok" })))
}

#[derive(Deserialize)]
struct SignupRequest {
    org_name: String,
    name: String,
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
pub struct UserResponse {
    id: Uuid,
    org_id: Uuid,
    email: String,
    name: String,
    role: String,
}

impl From<CurrentUser> for UserResponse {
    fn from(user: CurrentUser) -> Self {
        Self {
            id: user.id,
            org_id: user.org_id,
            email: user.email,
            name: user.name,
            role: user.role,
        }
    }
}

/// Signup creates the org and its first user together; there is no other way to
/// make an org, so every user has exactly one from the moment they exist.
async fn signup(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<SignupRequest>,
) -> AppResult<(CookieJar, Json<UserResponse>)> {
    let org_name = body.org_name.trim();
    let name = body.name.trim();
    let email = body.email.trim().to_lowercase();

    if org_name.is_empty() {
        return Err(AppError::BadRequest("organization name is required".into()));
    }
    if name.is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }
    if !email.contains('@') {
        return Err(AppError::BadRequest("a valid email is required".into()));
    }
    if body.password.chars().count() < 10 {
        return Err(AppError::BadRequest(
            "password must be at least 10 characters".into(),
        ));
    }

    let password_hash = hash_password(&body.password)?;

    let mut tx = state.pool.begin().await?;
    let org_id = sqlx::query_scalar!("insert into orgs (name) values ($1) returning id", org_name)
        .fetch_one(&mut *tx)
        .await?;

    let user = sqlx::query_as!(
        CurrentUser,
        r#"
        insert into users (org_id, email, password_hash, name, role)
        values ($1, $2, $3, $4, 'owner')
        returning id, org_id, email, name, role
        "#,
        org_id,
        email,
        password_hash,
        name
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|err| match err {
        sqlx::Error::Database(ref db) if db.is_unique_violation() => {
            AppError::Conflict("an account with that email already exists".into())
        }
        other => AppError::from(other),
    })?;
    tx.commit().await?;

    let session_id = create_session(&state.pool, user.id, state.config.session_ttl_days).await?;
    let jar = jar.add(session_cookie(&state.config, session_id.to_string()));
    Ok((jar, Json(user.into())))
}

async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> AppResult<(CookieJar, Json<UserResponse>)> {
    let email = body.email.trim().to_lowercase();

    let record = sqlx::query!(
        r#"
        select id, org_id, email, name, role, password_hash
        from users
        where lower(email) = $1
        "#,
        email
    )
    .fetch_optional(&state.pool)
    .await?;

    let record = record.ok_or(AppError::InvalidCredentials)?;
    // Google-only accounts have no password, so no password can match.
    let Some(password_hash) = &record.password_hash else {
        return Err(AppError::InvalidCredentials);
    };
    if !verify_password(&body.password, password_hash) {
        return Err(AppError::InvalidCredentials);
    }

    let session_id = create_session(&state.pool, record.id, state.config.session_ttl_days).await?;
    let jar = jar.add(session_cookie(&state.config, session_id.to_string()));

    Ok((
        jar,
        Json(UserResponse {
            id: record.id,
            org_id: record.org_id,
            email: record.email,
            name: record.name,
            role: record.role,
        }),
    ))
}

async fn logout(State(state): State<AppState>, jar: CookieJar) -> AppResult<(CookieJar, Json<Value>)> {
    if let Some(cookie) = jar.get(&state.config.session_cookie_name)
        && let Ok(session_id) = Uuid::parse_str(cookie.value())
    {
        delete_session(&state.pool, session_id).await?;
    }
    let jar = jar.add(clear_session_cookie(&state.config));
    Ok((jar, Json(json!({ "ok": true }))))
}

async fn me(user: CurrentUser) -> Json<UserResponse> {
    Json(user.into())
}

const GOOGLE_STATE_COOKIE: &str = "ce_google_sign_in_state";

/// Starts Google sign-in. As with connecting a mailbox, the `state` round-trip
/// is checked against a cookie so a callback cannot be forged.
async fn google_authorize(State(state): State<AppState>, jar: CookieJar) -> AppResult<(CookieJar, Redirect)> {
    if !state.config.google_configured() {
        return Err(AppError::BadRequest(
            "google oauth is not configured on this server".into(),
        ));
    }

    let nonce = Uuid::new_v4().to_string();
    let url = state
        .oauth
        .sign_in_url(&state.config.google_sign_in_redirect_uri(), &nonce);

    let mut cookie = Cookie::new(GOOGLE_STATE_COOKIE, nonce);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(state.config.secure_cookies());
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::minutes(10));

    Ok((jar.add(cookie), Redirect::to(&url)))
}

#[derive(Deserialize)]
struct GoogleCallback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// The browser lands here from Google, so every failure goes back to the login
/// page rather than rendering a bare JSON error.
async fn google_callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(params): Query<GoogleCallback>,
) -> (CookieJar, Redirect) {
    let mut cleared = Cookie::new(GOOGLE_STATE_COOKIE, "");
    cleared.set_path("/");
    cleared.set_max_age(time::Duration::ZERO);
    let expected = jar.get(GOOGLE_STATE_COOKIE).map(|cookie| cookie.value().to_string());
    let jar = jar.add(cleared);

    let login = format!("{}/login", state.config.app_url);
    let outcome = async {
        if let Some(error) = params.error {
            tracing::warn!(%error, "google returned a sign-in error");
            return Err("denied");
        }
        if expected.is_none() || expected != params.state {
            return Err("expired");
        }
        let code = params.code.ok_or("failed")?;

        let profile = state
            .oauth
            .sign_in_profile(&code, &state.config.google_sign_in_redirect_uri())
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "google sign-in code exchange failed");
                "failed"
            })?;
        if !profile.verified_email {
            return Err("unverified");
        }

        let user = sign_in_with_google(&state.pool, &profile.email, &profile.name)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "google sign-in could not load the account");
                "failed"
            })?;
        create_session(&state.pool, user.id, state.config.session_ttl_days)
            .await
            .map_err(|_| "failed")
    }
    .await;

    match outcome {
        Ok(session_id) => (
            jar.add(session_cookie(&state.config, session_id.to_string())),
            Redirect::to(&format!("{}/dashboard", state.config.app_url)),
        ),
        Err(reason) => (jar, Redirect::to(&format!("{login}?error=google_{reason}"))),
    }
}
