use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::campaigns::{self, NewCampaign};
use crate::error::{AppError, AppResult};
use crate::sequences::{self, NewStep};
use crate::state::AppState;
use crate::suppressions;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sequences", get(list_sequences).post(create_sequence))
        .route("/sequences/{id}/steps", get(sequence_steps))
        .route("/campaigns", get(list_campaigns).post(create_campaign))
        .route("/campaigns/{id}", get(campaign_detail))
        .route("/campaigns/{id}/launch", post(launch))
        .route("/campaigns/{id}/pause", post(pause))
        .route("/campaigns/{id}/resume", post(resume))
        .route("/suppressions", get(list_suppressions))
        .route("/dashboard", get(dashboard))
        // Public: a recipient clicking from their inbox has no session.
        .route("/unsubscribe/{token}", post(unsubscribe))
}

#[derive(Deserialize)]
struct CreateSequence {
    name: String,
    steps: Vec<NewStep>,
}

async fn create_sequence(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateSequence>,
) -> AppResult<Json<Value>> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("a sequence name is required".into()));
    }
    if body.steps.is_empty() {
        return Err(AppError::BadRequest("a sequence needs at least one step".into()));
    }
    if body.steps[0].subject.trim().is_empty() {
        return Err(AppError::BadRequest(
            "the first step needs a subject; later steps reply in its thread".into(),
        ));
    }

    let sequence = sequences::create(&state.pool, user.org_id, &body.name, body.steps)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "sequence": sequence })))
}

async fn list_sequences(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    let sequences = sequences::list(&state.pool, user.org_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "sequences": sequences })))
}

async fn sequence_steps(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let steps = sequences::steps(&state.pool, user.org_id, id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "steps": steps })))
}

async fn create_campaign(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<NewCampaign>,
) -> AppResult<Json<Value>> {
    let campaign = campaigns::create(&state.pool, user.org_id, body)
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    Ok(Json(json!({ "campaign": campaign })))
}

async fn list_campaigns(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    let campaigns = campaigns::list(&state.pool, user.org_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "campaigns": campaigns })))
}

async fn campaign_detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let campaign = campaigns::get(&state.pool, user.org_id, id)
        .await
        .map_err(|_| AppError::BadRequest("campaign not found".into()))?;
    let stats = campaigns::stats(&state.pool, user.org_id, id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "campaign": campaign, "stats": stats })))
}

async fn launch(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let enrolled = campaigns::launch(&state.pool, user.org_id, id)
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    Ok(Json(json!({ "enrolled": enrolled })))
}

async fn pause(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    campaigns::pause(&state.pool, user.org_id, id)
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

async fn resume(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    campaigns::resume(&state.pool, user.org_id, id)
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

async fn list_suppressions(
    State(state): State<AppState>,
    user: CurrentUser,
) -> AppResult<Json<Value>> {
    let emails = suppressions::list(&state.pool, user.org_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "suppressions": emails })))
}

async fn unsubscribe(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Json<Value>> {
    suppressions::unsubscribe(&state.pool, &token)
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

async fn dashboard(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    let summary = crate::dashboard::summary(&state.pool, user.org_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "summary": summary })))
}
