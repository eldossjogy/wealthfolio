use std::sync::Arc;

use tauri::State;

use wealthfolio_core::{
    portfolio::allocation_targets::{
        AllocationTarget, AllocationTargetWeight, CalculateRebalancePlanInput, DriftReport,
        NewAllocationTarget, NewAllocationTargetWeight, RebalanceDraft, RebalancePlan,
        SaveAllocationTargetResult, ScopeType,
    },
    portfolios::AccountScope,
};

use crate::context::ServiceContext;

use super::portfolio::{holdings_account_ids, AccountScopeInput};

fn scope_id_for_target(target: &AllocationTarget) -> Result<String, String> {
    target
        .scope_id
        .clone()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            format!(
                "Allocation target {} is missing scope_id for scoped drift",
                target.id
            )
        })
}

fn account_scope_for_target(target: &AllocationTarget) -> Result<AccountScope, String> {
    match &target.scope_type {
        ScopeType::All => Ok(AccountScope::All),
        ScopeType::Account => Ok(AccountScope::Account {
            account_id: scope_id_for_target(target)?,
        }),
        ScopeType::Portfolio => Ok(AccountScope::Portfolio {
            portfolio_id: scope_id_for_target(target)?,
        }),
    }
}

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
    let _ = filter;
    let base_currency = state.get_base_currency();
    let target = state
        .allocation_target_service()
        .get_target(&target_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("AllocationTarget {} not found", target_id))?;
    let filter = account_scope_for_target(&target)?;

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

// ── Rebalance ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn calculate_rebalance_plan(
    state: State<'_, Arc<ServiceContext>>,
    input: CalculateRebalancePlanInput,
) -> Result<RebalancePlan, String> {
    state
        .rebalance_service()
        .calculate_plan(input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_rebalance_draft(
    state: State<'_, Arc<ServiceContext>>,
    target_id: String,
    input: CalculateRebalancePlanInput,
    plan: RebalancePlan,
) -> Result<RebalanceDraft, String> {
    let svc = state.rebalance_service();
    let target = state
        .allocation_target_service()
        .get_target(&target_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Target {target_id} not found"))?;
    svc.save_draft(&target, &input, &plan)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_rebalance_drafts(
    state: State<'_, Arc<ServiceContext>>,
    target_id: String,
) -> Result<Vec<RebalanceDraft>, String> {
    state
        .rebalance_service()
        .list_drafts(&target_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_rebalance_draft(
    state: State<'_, Arc<ServiceContext>>,
    id: String,
) -> Result<(), String> {
    state
        .rebalance_service()
        .delete_draft(&id)
        .await
        .map_err(|e| e.to_string())
}
