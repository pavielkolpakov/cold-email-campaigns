use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::invites;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/invites", get(list).post(create))
        .route("/invites/{id}", delete(revoke))
        // Public: the person accepting does not have an account yet.
        .route("/invites/{token}/accept", post(accept))
}

/// Adding teammates changes who can send from the org's mailboxes, so it stays
/// with owners.
fn require_owner(user: &CurrentUser) -> AppResult<()> {
    if user.role != "owner" {
        return Err(AppError::Forbidden(
            "only an owner can manage invitations".into(),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct CreateInvite {
    email: String,
    #[serde(default = "member")]
    role: String,
}

fn member() -> String {
    "member".into()
}

async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateInvite>,
) -> AppResult<Json<Value>> {
    require_owner(&user)?;

    let invite = invites::create(&state.pool, user.org_id, user.id, &body.email, &body.role)
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?;

    Ok(Json(json!({ "invite": invite })))
}

async fn list(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    require_owner(&user)?;
    let invites = invites::list(&state.pool, user.org_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "invites": invites })))
}

async fn revoke(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    require_owner(&user)?;
    invites::revoke(&state.pool, user.org_id, id)
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct AcceptInvite {
    name: String,
    password: String,
}

async fn accept(
    State(state): State<AppState>,
    jar: axum_extra::extract::cookie::CookieJar,
    Path(token): Path<String>,
    Json(body): Json<AcceptInvite>,
) -> AppResult<(axum_extra::extract::cookie::CookieJar, Json<Value>)> {
    let user_id = invites::accept(&state.pool, &token, &body.name, &body.password)
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?;

    let session =
        crate::auth::create_session(&state.pool, user_id, state.config.session_ttl_days).await?;
    let jar = jar.add(crate::auth::session_cookie(&state.config, session.to_string()));

    Ok((jar, Json(json!({ "ok": true }))))
}
