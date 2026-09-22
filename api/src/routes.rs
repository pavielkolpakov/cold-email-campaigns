use anyhow::Context;
use axum::extract::State;
use axum::http::{HeaderValue, Method, header};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::auth::{
    CurrentUser, clear_session_cookie, create_session, delete_session, hash_password,
    session_cookie, verify_password,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            state
                .config
                .app_url
                .parse::<HeaderValue>()
                .expect("APP_URL must be a valid origin"),
        )
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE])
        .allow_credentials(true);

    Router::new()
        .route("/health", get(health))
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(state: AppState) -> anyhow::Result<()> {
    let bind = state.config.api_bind.clone();
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("failed to bind {bind}"))?;
    tracing::info!(%bind, "api listening");
    axum::serve(listener, router(state)).await?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> AppResult<Json<Value>> {
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
struct UserResponse {
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
    if !verify_password(&body.password, &record.password_hash) {
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
