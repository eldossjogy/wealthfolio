use std::sync::Arc;

use tauri::State;

use wealthfolio_core::portfolio::allocation_targets::{
    AllocationTarget, AllocationTargetWeight, DriftReport, NewAllocationTarget,
    NewAllocationTargetWeight, SaveAllocationTargetResult,
};

use crate::context::ServiceContext;

use super::portfolio::{holdings_account_ids, AccountScopeInput};

// ── Target CRUD ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_allocation_targets(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<AllocationTarget>, String> {
    state
        .allocation_target_service()
        .list_targets()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_allocation_target(
    state: State<'_, Arc<ServiceContext>>,
    id: String,
) -> Result<Option<AllocationTarget>, String> {
    state
        .allocation_target_service()
        .get_target(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_allocation_target(
    state: State<'_, Arc<ServiceContext>>,
    input: NewAllocationTarget,
) -> Result<AllocationTarget, String> {
    state
        .allocation_target_service()
        .create_target(input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_allocation_target(
    state: State<'_, Arc<ServiceContext>>,
    id: String,
    input: NewAllocationTarget,
) -> Result<AllocationTarget, String> {
    state
        .allocation_target_service()
        .update_target(&id, input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn archive_allocation_target(
    state: State<'_, Arc<ServiceContext>>,
    id: String,
) -> Result<AllocationTarget, String> {
    state
        .allocation_target_service()
        .archive_target(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_allocation_target(
    state: State<'_, Arc<ServiceContext>>,
    id: String,
) -> Result<(), String> {
    state
        .allocation_target_service()
        .delete_target(&id)
        .await
        .map_err(|e| e.to_string())
}

// ── Weights ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_allocation_target_weights(
    state: State<'_, Arc<ServiceContext>>,
    target_id: String,
) -> Result<Vec<AllocationTargetWeight>, String> {
    state
        .allocation_target_service()
        .list_weights_for_target(&target_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_allocation_target_weights(
    state: State<'_, Arc<ServiceContext>>,
    target_id: String,
    weights: Vec<NewAllocationTargetWeight>,
) -> Result<Vec<AllocationTargetWeight>, String> {
    state
        .allocation_target_service()
        .save_weights(&target_id, weights)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_allocation_target_with_weights(
    state: State<'_, Arc<ServiceContext>>,
    id: Option<String>,
    input: NewAllocationTarget,
    weights: Vec<NewAllocationTargetWeight>,
) -> Result<SaveAllocationTargetResult, String> {
    state
        .allocation_target_service()
        .save_target_with_weights(id, input, weights)
        .await
        .map_err(|e| e.to_string())
}

// ── Drift ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_allocation_target_drift(
    state: State<'_, Arc<ServiceContext>>,
    target_id: String,
    filter: AccountScopeInput,
    include_holdings: Option<bool>,
) -> Result<DriftReport, String> {
    let filter = filter.into_account_filter()?;
    let base_currency = state.get_base_currency();

    let resolved = wealthfolio_core::portfolios::PortfolioServiceTrait::resolve_account_scope(
        state.portfolio_service.as_ref(),
        &filter,
        &base_currency,
    )
    .map_err(|e| e.to_string())?;

    let account_ids = holdings_account_ids(&state, &resolved.account_ids)?;

    if include_holdings.unwrap_or(false) {
        state
            .drift_service()
            .get_drift_report_with_holdings_for_target(
                &target_id,
                &account_ids,
                &base_currency,
                &resolved.scope_id,
            )
            .await
            .map_err(|e| e.to_string())
    } else {
        state
            .drift_service()
            .get_drift_report_for_target(
                &target_id,
                &account_ids,
                &base_currency,
                &resolved.scope_id,
            )
            .await
            .map_err(|e| e.to_string())
    }
}
