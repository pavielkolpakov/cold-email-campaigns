use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::leads::{self, ColumnMapping};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/lead-lists", get(lists).post(create_list))
        .route("/lead-lists/{id}/leads", get(list_leads))
        .route("/lead-lists/{id}/import", post(import))
}

async fn lists(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    let lists = leads::lists(&state.pool, user.org_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "lists": lists })))
}

#[derive(Deserialize)]
struct CreateList {
    name: String,
}

async fn create_list(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateList>,
) -> AppResult<Json<Value>> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("a list name is required".into()));
    }
    let list = leads::create_list(&state.pool, user.org_id, &body.name)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "list": list })))
}

async fn list_leads(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let leads = leads::list(&state.pool, user.org_id, id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "leads": leads })))
}

#[derive(Deserialize)]
struct Import {
    /// The raw CSV text. The browser reads the file and sends its contents.
    csv: String,
    mapping: ColumnMapping,
}

async fn import(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<Import>,
) -> AppResult<Json<Value>> {
    let report = leads::import_csv(
        &state.pool,
        user.org_id,
        id,
        body.csv.as_bytes(),
        &body.mapping,
    )
    .await
    .map_err(|err| AppError::BadRequest(err.to_string()))?;

    Ok(Json(json!({ "report": report })))
}
