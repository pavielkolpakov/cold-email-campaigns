use axum::Router;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{delete, get, post};
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::mailboxes;
use crate::state::AppState;

const STATE_COOKIE: &str = "ce_oauth_state";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/mailboxes", get(list))
        .route("/mailboxes/gmail/authorize", get(authorize))
        .route("/mailboxes/gmail/callback", get(callback))
        .route("/mailboxes/{id}/test", post(send_test))
        .route("/mailboxes/{id}", delete(disconnect))
}

async fn list(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    let mailboxes = mailboxes::list(&state.pool, user.org_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "mailboxes": mailboxes })))
}

/// Starts the consent flow. The `state` value is echoed back by Google and
/// compared against a cookie, so a callback cannot be forged.
async fn authorize(
    State(state): State<AppState>,
    jar: CookieJar,
    _user: CurrentUser,
) -> AppResult<(CookieJar, Redirect)> {
    if !state.config.google_configured() {
        return Err(AppError::BadRequest(
            "google oauth is not configured on this server".into(),
        ));
    }

    let nonce = Uuid::new_v4().to_string();
    let url = state
        .oauth
        .consent_url(&state.config.google_redirect_uri, &nonce);

    let mut cookie = Cookie::new(STATE_COOKIE, nonce);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(state.config.secure_cookies());
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::minutes(10));

    Ok((jar.add(cookie), Redirect::to(&url)))
}

#[derive(Deserialize)]
struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn callback(
    State(state): State<AppState>,
    jar: CookieJar,
    user: CurrentUser,
    Query(params): Query<Callback>,
) -> AppResult<impl IntoResponse> {
    let settings = format!("{}/mailboxes", state.config.app_url);

    if let Some(error) = params.error {
        tracing::warn!(%error, "google returned an oauth error");
        return Ok((jar, Redirect::to(&format!("{settings}?error=denied"))));
    }

    let expected = jar
        .get(STATE_COOKIE)
        .map(|cookie| cookie.value().to_string())
        .ok_or_else(|| AppError::BadRequest("the connect request has expired".into()))?;
    let returned = params
        .state
        .ok_or_else(|| AppError::BadRequest("missing state parameter".into()))?;
    if expected != returned {
        return Err(AppError::BadRequest("state parameter mismatch".into()));
    }

    let code = params
        .code
        .ok_or_else(|| AppError::BadRequest("missing authorization code".into()))?;

    let credentials = state
        .oauth
        .exchange_code(&code, &state.config.google_redirect_uri)
        .await
        .map_err(AppError::Internal)?;
    let address = state
        .oauth
        .mailbox_address(&credentials.access_token)
        .await
        .map_err(AppError::Internal)?;

    mailboxes::connect(
        &state.pool,
        &state.cipher,
        user.org_id,
        mailboxes::NewMailbox {
            provider: "gmail".into(),
            email: address,
            credentials,
        },
    )
    .await
    .map_err(AppError::Internal)?;

    let mut cleared = Cookie::new(STATE_COOKIE, "");
    cleared.set_path("/");
    cleared.set_max_age(time::Duration::ZERO);

    Ok((jar.add(cleared), Redirect::to(&format!("{settings}?connected=1"))))
}

async fn send_test(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    mailboxes::send_test_email(
        &state.pool,
        &state.cipher,
        state.oauth.as_ref(),
        state.mailer.as_ref(),
        user.org_id,
        id,
    )
    .await
    .map_err(AppError::Internal)?;

    Ok(Json(json!({ "ok": true })))
}

async fn disconnect(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    mailboxes::disconnect(&state.pool, user.org_id, id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "ok": true })))
}
