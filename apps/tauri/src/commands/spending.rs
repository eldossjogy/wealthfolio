use std::sync::Arc;

use crate::context::ServiceContext;
use log::debug;
use tauri::State;
use wealthfolio_core::activities::Activity;
use wealthfolio_spending::activity_assignments::{
    ActivityTaxonomyAssignment, BulkCategoryAssignment,
};
use wealthfolio_spending::analytics::{
    EventSpendingSummary, EventSummariesRequest, MonthlyReport, ReportRequest, SpendingSummary,
};
use wealthfolio_spending::budget::{
    BudgetSnapshot, NewBudgetGroup, NewBudgetRolloverSetting, NewBudgetTarget, UpdateBudgetGroup,
};
use wealthfolio_spending::cash_activities::{
    CashActivityFilter, CashActivitySearchRequest, CashActivitySearchResponse,
};
use wealthfolio_spending::categorization_rules::{
    CategorizationRule, ImportPresetResult, NewCategorizationRule, RemovePresetResult,
    RulePresetSummary, UpdateCategorizationRule,
};
use wealthfolio_spending::events::{
    Event, EventType, EventWithTypeName, NewEvent, NewEventType, UpdateEvent,
};
use wealthfolio_spending::insight::{SpendingInsight, SpendingInsightRequest};
use wealthfolio_spending::settings::{SpendingSettings, SpendingSettingsUpdate};

#[tauri::command]
pub async fn get_spending_settings(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<SpendingSettings, String> {
    debug!("Fetching spending settings...");
    state
        .spending_settings_service()
        .get()
        .await
        .map_err(|e| format!("Failed to load spending settings: {}", e))
}

#[tauri::command]
pub async fn update_spending_settings(
    update: SpendingSettingsUpdate,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<SpendingSettings, String> {
    debug!("Updating spending settings...");
    state
        .spending_settings_service()
        .update(update)
        .await
        .map_err(|e| format!("Failed to update spending settings: {}", e))
}

#[tauri::command]
pub async fn list_cash_activities(
    filter: Option<CashActivityFilter>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<Activity>, String> {
    debug!("Listing cash activities...");
    state
        .cash_activity_service()
        .list(filter.unwrap_or_default())
        .await
        .map_err(|e| format!("Failed to list cash activities: {}", e))
}

#[tauri::command]
pub async fn search_cash_activities(
    request: Option<CashActivitySearchRequest>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<CashActivitySearchResponse, String> {
    debug!("Searching cash activities...");
    state
        .cash_activity_service()
        .search(request.unwrap_or_default())
        .await
        .map_err(|e| format!("Failed to search cash activities: {}", e))
}

#[tauri::command]
pub async fn set_activity_event(
    activity_id: String,
    event_id: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Activity, String> {
    state
        .cash_activity_service()
        .set_event(&activity_id, event_id)
        .await
        .map_err(|e| format!("Failed to set activity event: {}", e))
}

#[tauri::command]
pub async fn get_activity_assignments(
    activity_id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<ActivityTaxonomyAssignment>, String> {
    state
        .activity_taxonomy_assignment_service()
        .list_for_activity(&activity_id)
        .await
        .map_err(|e| format!("Failed to load activity assignments: {}", e))
}

#[tauri::command]
pub async fn assign_activity_category(
    activity_id: String,
    taxonomy_id: String,
    category_id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ActivityTaxonomyAssignment, String> {
    state
        .activity_taxonomy_assignment_service()
        .assign_single(&activity_id, &taxonomy_id, &category_id)
        .await
        .map_err(|e| format!("Failed to assign activity category: {}", e))
}

#[tauri::command]
pub async fn unassign_activity_category(
    activity_id: String,
    taxonomy_id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    state
        .activity_taxonomy_assignment_service()
        .unassign(&activity_id, &taxonomy_id)
        .await
        .map_err(|e| format!("Failed to clear activity category: {}", e))
}

/// Atomic batch assign — used by bulk-categorize on the transactions page and
/// by the AI proposal widget. Each item replaces any existing single-select
/// assignment for its (activity_id, taxonomy_id) pair.
#[tauri::command]
pub async fn bulk_assign_categories(
    items: Vec<BulkCategoryAssignment>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<ActivityTaxonomyAssignment>, String> {
    state
        .activity_taxonomy_assignment_service()
        .assign_many_single_select(&items)
        .await
        .map_err(|e| format!("Failed to bulk assign categories: {}", e))
}

#[tauri::command]
pub async fn list_categorization_rules(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<CategorizationRule>, String> {
    state
        .categorization_rules_service()
        .list()
        .await
        .map_err(|e| format!("Failed to list rules: {}", e))
}

#[tauri::command]
pub async fn create_categorization_rule(
    rule: NewCategorizationRule,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<CategorizationRule, String> {
    state
        .categorization_rules_service()
        .create(rule)
        .await
        .map_err(|e| format!("Failed to create rule: {}", e))
}

#[tauri::command]
pub async fn update_categorization_rule(
    id: String,
    patch: UpdateCategorizationRule,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<CategorizationRule, String> {
    state
        .categorization_rules_service()
        .update(&id, patch)
        .await
        .map_err(|e| format!("Failed to update rule: {}", e))
}

#[tauri::command]
pub async fn delete_categorization_rule(
    id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    state
        .categorization_rules_service()
        .delete(&id)
        .await
        .map_err(|e| format!("Failed to delete rule: {}", e))
}

#[tauri::command]
pub async fn rerun_categorization_rules(
    only_uncategorized: bool,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<usize, String> {
    let s = state
        .spending_settings_service()
        .get()
        .await
        .map_err(|e| format!("Failed to load spending settings: {}", e))?;
    state
        .categorization_rules_service()
        .rerun_all(&s.account_ids, only_uncategorized)
        .await
        .map_err(|e| format!("Failed to re-run rules: {}", e))
}

#[tauri::command]
pub async fn list_rule_presets(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<RulePresetSummary>, String> {
    state
        .categorization_rules_service()
        .list_presets()
        .await
        .map_err(|e| format!("Failed to list rule presets: {}", e))
}

#[tauri::command]
pub async fn import_rule_preset(
    preset_id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<ImportPresetResult, String> {
    // Build the categoryKey → (taxonomy_id, category_id) resolver from the
    // activity-scope taxonomies (spending_categories + income_sources).
    let taxonomies = state
        .taxonomy_service()
        .get_taxonomies_with_categories()
        .map_err(|e| format!("Failed to load taxonomies: {}", e))?;

    let mut resolver: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    for entry in taxonomies.iter().filter(|t| t.taxonomy.scope == "activity") {
        for cat in &entry.categories {
            resolver.insert(cat.key.clone(), (entry.taxonomy.id.clone(), cat.id.clone()));
        }
    }

    state
        .categorization_rules_service()
        .import_preset(&preset_id, &resolver)
        .await
        .map_err(|e| format!("Failed to import rule preset: {}", e))
}

#[tauri::command]
pub async fn remove_rule_preset(
    preset_id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<RemovePresetResult, String> {
    state
        .categorization_rules_service()
        .remove_preset(&preset_id)
        .await
        .map_err(|e| format!("Failed to remove rule preset: {}", e))
}

#[tauri::command]
pub async fn list_event_types(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<EventType>, String> {
    state
        .events_service()
        .list_types()
        .await
        .map_err(|e| format!("Failed to list event types: {}", e))
}

#[tauri::command]
pub async fn create_event_type(
    new_type: NewEventType,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<EventType, String> {
    state
        .events_service()
        .create_type(new_type)
        .await
        .map_err(|e| format!("Failed to create event type: {}", e))
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEventType {
    pub name: Option<String>,
    pub color: Option<Option<String>>,
}

#[tauri::command]
pub async fn update_event_type(
    id: String,
    patch: UpdateEventType,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<EventType, String> {
    state
        .events_service()
        .update_type(&id, patch.name, patch.color)
        .await
        .map_err(|e| format!("Failed to update event type: {}", e))
}

#[tauri::command]
pub async fn delete_event_type(
    id: String,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<(), String> {
    state
        .events_service()
        .delete_type(&id)
        .await
        .map_err(|e| format!("Failed to delete event type: {}", e))
}

#[tauri::command]
pub async fn list_events(state: State<'_, Arc<ServiceContext>>) -> Result<Vec<Event>, String> {
    state
        .events_service()
        .list_events()
        .await
        .map_err(|e| format!("Failed to list events: {}", e))
}

#[tauri::command]
pub async fn create_event(
    event: NewEvent,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Event, String> {
    state
        .events_service()
        .create_event(event)
        .await
        .map_err(|e| format!("Failed to create event: {}", e))
}

#[tauri::command]
pub async fn update_event(
    id: String,
    patch: UpdateEvent,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Event, String> {
    state
        .events_service()
        .update_event(&id, patch)
        .await
        .map_err(|e| format!("Failed to update event: {}", e))
}

#[tauri::command]
pub async fn delete_event(id: String, state: State<'_, Arc<ServiceContext>>) -> Result<(), String> {
    state
        .events_service()
        .delete_event(&id)
        .await
        .map_err(|e| format!("Failed to delete event: {}", e))
}

#[tauri::command]
pub async fn get_budget(
    period_key: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<BudgetSnapshot, String> {
    let base_currency = state.get_base_currency();
    state
        .budget_service()
        .get(period_key, &base_currency)
        .await
        .map_err(|e| format!("Failed to load budget: {}", e))
}

#[tauri::command]
pub async fn upsert_budget_target(
    target: NewBudgetTarget,
    period_key: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<BudgetSnapshot, String> {
    let base_currency = state.get_base_currency();
    state
        .budget_service()
        .upsert_target(target, period_key, &base_currency)
        .await
        .map_err(|e| format!("Failed to save budget target: {}", e))
}

#[tauri::command]
pub async fn delete_budget_target(
    id: String,
    period_key: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<BudgetSnapshot, String> {
    let base_currency = state.get_base_currency();
    state
        .budget_service()
        .delete_target(&id, period_key, &base_currency)
        .await
        .map_err(|e| format!("Failed to delete budget target: {}", e))
}

#[tauri::command]
pub async fn upsert_budget_rollover_setting(
    setting: NewBudgetRolloverSetting,
    period_key: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<BudgetSnapshot, String> {
    let base_currency = state.get_base_currency();
    state
        .budget_service()
        .upsert_rollover_setting(setting, period_key, &base_currency)
        .await
        .map_err(|e| format!("Failed to save budget rollover setting: {}", e))
}

#[tauri::command]
pub async fn delete_budget_rollover_setting(
    id: String,
    period_key: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<BudgetSnapshot, String> {
    let base_currency = state.get_base_currency();
    state
        .budget_service()
        .delete_rollover_setting(&id, period_key, &base_currency)
        .await
        .map_err(|e| format!("Failed to delete budget rollover setting: {}", e))
}

#[tauri::command]
pub async fn create_budget_group(
    group: NewBudgetGroup,
    period_key: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<BudgetSnapshot, String> {
    let base_currency = state.get_base_currency();
    state
        .budget_service()
        .create_group(group, period_key, &base_currency)
        .await
        .map_err(|e| format!("Failed to create budget group: {}", e))
}

#[tauri::command]
pub async fn update_budget_group(
    id: String,
    patch: UpdateBudgetGroup,
    period_key: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<BudgetSnapshot, String> {
    let base_currency = state.get_base_currency();
    state
        .budget_service()
        .update_group(&id, patch, period_key, &base_currency)
        .await
        .map_err(|e| format!("Failed to update budget group: {}", e))
}

#[tauri::command]
pub async fn delete_budget_group(
    id: String,
    reassign_to_group_id: String,
    period_key: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<BudgetSnapshot, String> {
    let base_currency = state.get_base_currency();
    state
        .budget_service()
        .delete_group(&id, &reassign_to_group_id, period_key, &base_currency)
        .await
        .map_err(|e| format!("Failed to delete budget group: {}", e))
}

#[tauri::command]
pub async fn assign_category_to_group(
    category_id: String,
    group_id: String,
    period_key: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<BudgetSnapshot, String> {
    let base_currency = state.get_base_currency();
    state
        .budget_service()
        .assign_category_to_group(category_id, group_id, period_key, &base_currency)
        .await
        .map_err(|e| format!("Failed to assign category to group: {}", e))
}

#[tauri::command]
pub async fn reset_budget_groups(
    period_key: Option<String>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<BudgetSnapshot, String> {
    let base_currency = state.get_base_currency();
    state
        .budget_service()
        .reset_groups(period_key, &base_currency)
        .await
        .map_err(|e| format!("Failed to reset budget groups: {}", e))
}

#[tauri::command]
pub async fn copy_budget_targets(
    source_period_key: String,
    target_period_key: String,
    overwrite: bool,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<BudgetSnapshot, String> {
    let base_currency = state.get_base_currency();
    state
        .budget_service()
        .copy_period_targets(
            &source_period_key,
            &target_period_key,
            overwrite,
            &base_currency,
        )
        .await
        .map_err(|e| format!("Failed to copy budget targets: {}", e))
}

#[tauri::command]
pub async fn get_spending_report(
    request: ReportRequest,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<MonthlyReport, String> {
    state
        .spending_analytics_service()
        .monthly_report(request)
        .await
        .map_err(|e| format!("Failed to compute spending report: {}", e))
}

#[tauri::command]
pub async fn get_spending_insight(
    request: SpendingInsightRequest,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<SpendingInsight, String> {
    let currency = state.get_base_currency();
    state
        .spending_insight_service()
        .compute(request, &currency)
        .await
        .map_err(|e| format!("Failed to compute spending insight: {}", e))
}

#[tauri::command]
pub async fn get_spending_summary(
    include_event_ids: Option<Vec<String>>,
    include_all_events: Option<bool>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<SpendingSummary>, String> {
    state
        .spending_analytics_service()
        .spending_summary(include_event_ids, include_all_events)
        .await
        .map_err(|e| format!("Failed to compute spending summary: {}", e))
}

#[tauri::command]
pub async fn get_events_with_names(
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<EventWithTypeName>, String> {
    state
        .events_service()
        .list_events_with_names()
        .await
        .map_err(|e| format!("Failed to list events with names: {}", e))
}

#[tauri::command]
pub async fn get_event_spending_summaries(
    request: Option<EventSummariesRequest>,
    state: State<'_, Arc<ServiceContext>>,
) -> Result<Vec<EventSpendingSummary>, String> {
    let mut req = request.unwrap_or(EventSummariesRequest {
        start_date: None,
        end_date: None,
        currency: None,
    });
    if req.currency.is_none() {
        req.currency = Some(state.get_base_currency());
    }
    state
        .spending_analytics_service()
        .event_spending_summaries(req)
        .await
        .map_err(|e| format!("Failed to compute event spending summaries: {}", e))
}
