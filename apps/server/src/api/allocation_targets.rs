use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use wealthfolio_core::{
    portfolio::allocation_targets::{
        DriftReport, NewTargetAllocationNode, NewTargetProfile, TargetAllocationNode, TargetProfile,
    },
    portfolios::AccountScope,
};

use crate::{error::ApiResult, main_lib::AppState};

// ── Profile CRUD ──────────────────────────────────────────────────────────────

async fn list_profiles(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<TargetProfile>>> {
    let profiles = state.target_profile_service.list_profiles()?;
    Ok(Json(profiles))
}

async fn get_profile(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Option<TargetProfile>>> {
    let profile = state.target_profile_service.get_profile(&id)?;
    Ok(Json(profile))
}

async fn create_profile(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<NewTargetProfile>,
) -> ApiResult<Json<TargetProfile>> {
    let created = state.target_profile_service.create_profile(payload).await?;
    Ok(Json(created))
}

async fn update_profile(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<NewTargetProfile>,
) -> ApiResult<Json<TargetProfile>> {
    let updated = state
        .target_profile_service
        .update_profile(&id, payload)
        .await?;
    Ok(Json(updated))
}

async fn activate_profile(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<TargetProfile>> {
    let profile = state.target_profile_service.activate_profile(&id).await?;
    Ok(Json(profile))
}

async fn archive_profile(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<TargetProfile>> {
    let profile = state.target_profile_service.archive_profile(&id).await?;
    Ok(Json(profile))
}

async fn delete_profile(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> ApiResult<StatusCode> {
    state.target_profile_service.delete_profile(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Nodes ─────────────────────────────────────────────────────────────────────

async fn list_nodes(
    Path(profile_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<TargetAllocationNode>>> {
    let nodes = state
        .target_profile_service
        .list_nodes_for_profile(&profile_id)?;
    Ok(Json(nodes))
}

async fn save_nodes(
    Path(profile_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(nodes): Json<Vec<NewTargetAllocationNode>>,
) -> ApiResult<Json<Vec<TargetAllocationNode>>> {
    let saved = state
        .target_profile_service
        .save_nodes(&profile_id, nodes)
        .await?;
    Ok(Json(saved))
}

// ── Drift ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DriftBody {
    filter: AccountScope,
}

async fn get_drift(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DriftBody>,
) -> ApiResult<Json<Option<DriftReport>>> {
    let base_currency = state.base_currency.read().unwrap().clone();
    let resolved = state
        .portfolio_service
        .resolve_account_scope(&body.filter, &base_currency)
        .map_err(crate::error::ApiError::from)?;

    let scope_type = resolved.scope_id.split(':').next().unwrap_or("all");
    let scope_id = resolved.scope_id.split(':').nth(1).map(|s| s.to_string());

    let report = state
        .drift_service
        .get_drift_report(
            scope_type,
            scope_id.as_deref(),
            &resolved.account_ids,
            &base_currency,
            &resolved.scope_id,
        )
        .await?;
    Ok(Json(report))
}

async fn get_drift_for_profile(
    Path(profile_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<DriftBody>,
) -> ApiResult<Json<DriftReport>> {
    let base_currency = state.base_currency.read().unwrap().clone();
    let resolved = state
        .portfolio_service
        .resolve_account_scope(&body.filter, &base_currency)
        .map_err(crate::error::ApiError::from)?;

    let report = state
        .drift_service
        .get_drift_report_for_profile(
            &profile_id,
            &resolved.account_ids,
            &base_currency,
            &resolved.scope_id,
        )
        .await?;
    Ok(Json(report))
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/allocation-targets/profiles",
            get(list_profiles).post(create_profile),
        )
        .route(
            "/allocation-targets/profiles/{id}",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
        .route(
            "/allocation-targets/profiles/{id}/activate",
            post(activate_profile),
        )
        .route(
            "/allocation-targets/profiles/{id}/archive",
            post(archive_profile),
        )
        .route(
            "/allocation-targets/profiles/{id}/nodes",
            get(list_nodes).post(save_nodes),
        )
        .route("/allocation-targets/drift", post(get_drift))
        .route(
            "/allocation-targets/profiles/{id}/drift",
            post(get_drift_for_profile),
        )
}
