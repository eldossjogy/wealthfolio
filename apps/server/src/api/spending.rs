use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;

use crate::{error::ApiResult, main_lib::AppState};
use wealthfolio_core::activities::Activity;
use wealthfolio_spending::activity_assignments::ActivityTaxonomyAssignment;
use wealthfolio_spending::analytics::{MonthlyReport, ReportRequest};
use wealthfolio_spending::budget::{BudgetAllocation, BudgetSnapshot, UpdateBudgetConfig};
use wealthfolio_spending::cash_activities::{
    CashActivityFilter, CashActivitySearchRequest, CashActivitySearchResponse,
};
use wealthfolio_spending::categorization_rules::{
    CategorizationRule, NewCategorizationRule, UpdateCategorizationRule,
};
use wealthfolio_spending::events::{Event, EventType, NewEvent, NewEventType, UpdateEvent};
use wealthfolio_spending::settings::{SpendingSettings, SpendingSettingsUpdate};

async fn get_spending_settings(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<SpendingSettings>> {
    let s = state.spending_settings_service.get().await?;
    Ok(Json(s))
}

async fn update_spending_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SpendingSettingsUpdate>,
) -> ApiResult<Json<SpendingSettings>> {
    let s = state.spending_settings_service.update(payload).await?;
    Ok(Json(s))
}

async fn list_cash_activities(
    State(state): State<Arc<AppState>>,
    Query(filter): Query<CashActivityFilter>,
) -> ApiResult<Json<Vec<Activity>>> {
    let activities = state.cash_activity_service.list(filter).await?;
    Ok(Json(activities))
}

async fn search_cash_activities(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CashActivitySearchRequest>,
) -> ApiResult<Json<CashActivitySearchResponse>> {
    let response = state.cash_activity_service.search(request).await?;
    Ok(Json(response))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetEventBody {
    event_id: Option<String>,
}

async fn set_activity_event(
    State(state): State<Arc<AppState>>,
    Path(activity_id): Path<String>,
    Json(body): Json<SetEventBody>,
) -> ApiResult<Json<Activity>> {
    let activity = state
        .cash_activity_service
        .set_event(&activity_id, body.event_id)
        .await?;
    Ok(Json(activity))
}

async fn get_activity_assignments(
    State(state): State<Arc<AppState>>,
    Path(activity_id): Path<String>,
) -> ApiResult<Json<Vec<ActivityTaxonomyAssignment>>> {
    let rows = state
        .activity_taxonomy_assignment_service
        .list_for_activity(&activity_id)
        .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssignBody {
    taxonomy_id: String,
    category_id: String,
}

async fn assign_activity_category(
    State(state): State<Arc<AppState>>,
    Path(activity_id): Path<String>,
    Json(body): Json<AssignBody>,
) -> ApiResult<Json<ActivityTaxonomyAssignment>> {
    let row = state
        .activity_taxonomy_assignment_service
        .assign_single(&activity_id, &body.taxonomy_id, &body.category_id)
        .await?;
    Ok(Json(row))
}

async fn unassign_activity_category(
    State(state): State<Arc<AppState>>,
    Path((activity_id, taxonomy_id)): Path<(String, String)>,
) -> ApiResult<()> {
    state
        .activity_taxonomy_assignment_service
        .unassign(&activity_id, &taxonomy_id)
        .await?;
    Ok(())
}

async fn list_categorization_rules(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<CategorizationRule>>> {
    Ok(Json(state.categorization_rules_service.list().await?))
}

async fn create_categorization_rule(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<NewCategorizationRule>,
) -> ApiResult<Json<CategorizationRule>> {
    Ok(Json(
        state.categorization_rules_service.create(payload).await?,
    ))
}

async fn update_categorization_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateCategorizationRule>,
) -> ApiResult<Json<CategorizationRule>> {
    Ok(Json(
        state
            .categorization_rules_service
            .update(&id, payload)
            .await?,
    ))
}

async fn delete_categorization_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<()> {
    state.categorization_rules_service.delete(&id).await?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RerunRulesBody {
    #[serde(default)]
    only_uncategorized: bool,
}

async fn rerun_categorization_rules(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RerunRulesBody>,
) -> ApiResult<Json<usize>> {
    let s = state.spending_settings_service.get().await?;
    Ok(Json(
        state
            .categorization_rules_service
            .rerun_all(&s.account_ids, body.only_uncategorized)
            .await?,
    ))
}

async fn list_rule_presets(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<wealthfolio_spending::categorization_rules::RulePresetSummary>>> {
    Ok(Json(
        state.categorization_rules_service.list_presets().await?,
    ))
}

async fn import_rule_preset(
    State(state): State<Arc<AppState>>,
    Path(preset_id): Path<String>,
) -> ApiResult<Json<wealthfolio_spending::categorization_rules::ImportPresetResult>> {
    let taxonomies = state.taxonomy_service.get_taxonomies_with_categories()?;
    let mut resolver: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    for entry in taxonomies.iter().filter(|t| t.taxonomy.scope == "activity") {
        for cat in &entry.categories {
            resolver.insert(cat.key.clone(), (entry.taxonomy.id.clone(), cat.id.clone()));
        }
    }
    Ok(Json(
        state
            .categorization_rules_service
            .import_preset(&preset_id, &resolver)
            .await?,
    ))
}

async fn list_event_types(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<EventType>>> {
    Ok(Json(state.events_service.list_types().await?))
}

async fn create_event_type(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<NewEventType>,
) -> ApiResult<Json<EventType>> {
    Ok(Json(state.events_service.create_type(payload).await?))
}

async fn delete_event_type(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<()> {
    state.events_service.delete_type(&id).await?;
    Ok(())
}

async fn list_events(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<Event>>> {
    Ok(Json(state.events_service.list_events().await?))
}

async fn create_event(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<NewEvent>,
) -> ApiResult<Json<Event>> {
    Ok(Json(state.events_service.create_event(payload).await?))
}

async fn update_event(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateEvent>,
) -> ApiResult<Json<Event>> {
    Ok(Json(state.events_service.update_event(&id, payload).await?))
}

async fn delete_event(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult<()> {
    state.events_service.delete_event(&id).await?;
    Ok(())
}

async fn get_budget(State(state): State<Arc<AppState>>) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(state.budget_service.get(&base).await?))
}

async fn update_budget_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateBudgetConfig>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    state.budget_service.update_config(payload, &base).await?;
    Ok(Json(state.budget_service.get(&base).await?))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpsertAllocationBody {
    taxonomy_id: String,
    category_id: String,
    amount: String,
}

async fn upsert_budget_allocation(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpsertAllocationBody>,
) -> ApiResult<Json<BudgetAllocation>> {
    let base = state.base_currency.read().unwrap().clone();
    let row = state
        .budget_service
        .upsert_allocation(body.taxonomy_id, body.category_id, body.amount, &base)
        .await?;
    Ok(Json(row))
}

async fn delete_budget_allocation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<()> {
    state.budget_service.delete_allocation(&id).await?;
    Ok(())
}

async fn get_spending_report(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ReportRequest>,
) -> ApiResult<Json<MonthlyReport>> {
    Ok(Json(
        state
            .spending_analytics_service
            .monthly_report(payload)
            .await?,
    ))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/spending/settings", get(get_spending_settings))
        .route("/v1/spending/settings", put(update_spending_settings))
        .route("/v1/spending/cash-activities", get(list_cash_activities))
        .route(
            "/v1/spending/cash-activities/search",
            post(search_cash_activities),
        )
        .route(
            "/v1/spending/cash-activities/:activity_id/event",
            put(set_activity_event),
        )
        .route(
            "/v1/spending/activities/:activity_id/assignments",
            get(get_activity_assignments).put(assign_activity_category),
        )
        .route(
            "/v1/spending/activities/:activity_id/assignments/:taxonomy_id",
            delete(unassign_activity_category),
        )
        .route(
            "/v1/spending/rules",
            get(list_categorization_rules).post(create_categorization_rule),
        )
        .route(
            "/v1/spending/rules/:id",
            put(update_categorization_rule).delete(delete_categorization_rule),
        )
        .route("/v1/spending/rules/rerun", post(rerun_categorization_rules))
        .route("/v1/spending/rule-presets", get(list_rule_presets))
        .route(
            "/v1/spending/rule-presets/:preset_id/import",
            post(import_rule_preset),
        )
        .route(
            "/v1/spending/event-types",
            get(list_event_types).post(create_event_type),
        )
        .route("/v1/spending/event-types/:id", delete(delete_event_type))
        .route("/v1/spending/events", get(list_events).post(create_event))
        .route(
            "/v1/spending/events/:id",
            put(update_event).delete(delete_event),
        )
        .route(
            "/v1/spending/budget",
            get(get_budget).put(update_budget_config),
        )
        .route(
            "/v1/spending/budget/allocations",
            post(upsert_budget_allocation),
        )
        .route(
            "/v1/spending/budget/allocations/:id",
            delete(delete_budget_allocation),
        )
        .route("/v1/spending/report", post(get_spending_report))
}
