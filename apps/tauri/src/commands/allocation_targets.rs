use std::sync::Arc;

use serde::Deserialize;
use tauri::State;

use wealthfolio_core::portfolio::allocation_targets::{
    DriftReport, NewTargetAllocationNode, NewTargetProfile, TargetAllocationNode, TargetProfile,
};

use crate::context::ServiceContext;

use super::portfolio::AccountScopeInput;

// ── Profile CRUD ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_target_profiles(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<TargetProfile>, String> {
    state
        .target_profile_service()
        .list_profiles()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_target_profile(
    state: State<'_, Arc<ServiceContext>>,
    id: String,
) -> Result<Option<TargetProfile>, String> {
    state
        .target_profile_service()
        .get_profile(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_target_profile(
    state: State<'_, Arc<ServiceContext>>,
    input: NewTargetProfile,
) -> Result<TargetProfile, String> {
    state
        .target_profile_service()
        .create_profile(input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_target_profile(
    state: State<'_, Arc<ServiceContext>>,
    id: String,
    input: NewTargetProfile,
) -> Result<TargetProfile, String> {
    state
        .target_profile_service()
        .update_profile(&id, input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn activate_target_profile(
    state: State<'_, Arc<ServiceContext>>,
    id: String,
) -> Result<TargetProfile, String> {
    state
        .target_profile_service()
        .activate_profile(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn archive_target_profile(
    state: State<'_, Arc<ServiceContext>>,
    id: String,
) -> Result<TargetProfile, String> {
    state
        .target_profile_service()
        .archive_profile(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_target_profile(
    state: State<'_, Arc<ServiceContext>>,
    id: String,
) -> Result<(), String> {
    state
        .target_profile_service()
        .delete_profile(&id)
        .await
        .map_err(|e| e.to_string())
}

// ── Nodes ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_target_nodes(
    state: State<'_, Arc<ServiceContext>>,
    profile_id: String,
) -> Result<Vec<TargetAllocationNode>, String> {
    state
        .target_profile_service()
        .list_nodes_for_profile(&profile_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_target_nodes(
    state: State<'_, Arc<ServiceContext>>,
    profile_id: String,
    nodes: Vec<NewTargetAllocationNode>,
) -> Result<Vec<TargetAllocationNode>, String> {
    state
        .target_profile_service()
        .save_nodes(&profile_id, nodes)
        .await
        .map_err(|e| e.to_string())
}

// ── Drift ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftInput {
    pub filter: AccountScopeInput,
}

#[tauri::command]
pub async fn get_target_drift(
    state: State<'_, Arc<ServiceContext>>,
    input: DriftInput,
) -> Result<Option<DriftReport>, String> {
    let filter = input.filter.into_account_filter()?;
    let base_currency = state.get_base_currency();

    let resolved = wealthfolio_core::portfolios::PortfolioServiceTrait::resolve_account_scope(
        state.portfolio_service.as_ref(),
        &filter,
        &base_currency,
    )
    .map_err(|e| e.to_string())?;

    let scope_type = resolved.scope_id.split(':').next().unwrap_or("all");
    let scope_id = resolved.scope_id.split(':').nth(1).map(|s| s.to_string());

    state
        .drift_service()
        .get_drift_report(
            scope_type,
            scope_id.as_deref(),
            &resolved.account_ids,
            &base_currency,
            &resolved.scope_id,
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_target_drift_for_profile(
    state: State<'_, Arc<ServiceContext>>,
    profile_id: String,
    filter: AccountScopeInput,
) -> Result<DriftReport, String> {
    let filter = filter.into_account_filter()?;
    let base_currency = state.get_base_currency();

    let resolved = wealthfolio_core::portfolios::PortfolioServiceTrait::resolve_account_scope(
        state.portfolio_service.as_ref(),
        &filter,
        &base_currency,
    )
    .map_err(|e| e.to_string())?;

    state
        .drift_service()
        .get_drift_report_for_profile(
            &profile_id,
            &resolved.account_ids,
            &base_currency,
            &resolved.scope_id,
        )
        .await
        .map_err(|e| e.to_string())
}
