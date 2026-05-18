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
use wealthfolio_spending::budget::{
    BudgetSnapshot, NewBudgetGroup, NewBudgetRolloverSetting, NewBudgetTarget, UpdateBudgetGroup,
};
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

async fn bulk_assign_categories(
    State(state): State<Arc<AppState>>,
    Json(items): Json<Vec<wealthfolio_spending::activity_assignments::BulkCategoryAssignment>>,
) -> ApiResult<Json<Vec<ActivityTaxonomyAssignment>>> {
    let result = state
        .activity_taxonomy_assignment_service
        .assign_many_single_select(&items)
        .await?;
    Ok(Json(result))
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

async fn remove_rule_preset(
    State(state): State<Arc<AppState>>,
    Path(preset_id): Path<String>,
) -> ApiResult<Json<wealthfolio_spending::categorization_rules::RemovePresetResult>> {
    Ok(Json(
        state
            .categorization_rules_service
            .remove_preset(&preset_id)
            .await?,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BudgetQuery {
    period_key: Option<String>,
}

async fn get_budget(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BudgetQuery>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(
        state.budget_service.get(query.period_key, &base).await?,
    ))
}

async fn upsert_budget_target(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BudgetQuery>,
    Json(payload): Json<NewBudgetTarget>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(
        state
            .budget_service
            .upsert_target(payload, query.period_key, &base)
            .await?,
    ))
}

async fn delete_budget_target(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BudgetQuery>,
    Path(id): Path<String>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(
        state
            .budget_service
            .delete_target(&id, query.period_key, &base)
            .await?,
    ))
}

async fn upsert_budget_rollover_setting(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BudgetQuery>,
    Json(payload): Json<NewBudgetRolloverSetting>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(
        state
            .budget_service
            .upsert_rollover_setting(payload, query.period_key, &base)
            .await?,
    ))
}

async fn delete_budget_rollover_setting(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BudgetQuery>,
    Path(id): Path<String>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(
        state
            .budget_service
            .delete_rollover_setting(&id, query.period_key, &base)
            .await?,
    ))
}

async fn create_budget_group(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BudgetQuery>,
    Json(payload): Json<NewBudgetGroup>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(
        state
            .budget_service
            .create_group(payload, query.period_key, &base)
            .await?,
    ))
}

async fn update_budget_group(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BudgetQuery>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateBudgetGroup>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(
        state
            .budget_service
            .update_group(&id, payload, query.period_key, &base)
            .await?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteBudgetGroupBody {
    reassign_to_group_id: String,
}

async fn delete_budget_group(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BudgetQuery>,
    Path(id): Path<String>,
    Json(payload): Json<DeleteBudgetGroupBody>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(
        state
            .budget_service
            .delete_group(&id, &payload.reassign_to_group_id, query.period_key, &base)
            .await?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssignCategoryToGroupBody {
    category_id: String,
    group_id: String,
}

async fn assign_category_to_group(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BudgetQuery>,
    Json(payload): Json<AssignCategoryToGroupBody>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(
        state
            .budget_service
            .assign_category_to_group(
                payload.category_id,
                payload.group_id,
                query.period_key,
                &base,
            )
            .await?,
    ))
}

async fn reset_budget_groups(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BudgetQuery>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(
        state
            .budget_service
            .reset_groups(query.period_key, &base)
            .await?,
    ))
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopyBudgetTargetsBody {
    source_period_key: String,
    target_period_key: String,
    #[serde(default)]
    overwrite: bool,
}

async fn copy_budget_targets(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CopyBudgetTargetsBody>,
) -> ApiResult<Json<BudgetSnapshot>> {
    let base = state.base_currency.read().unwrap().clone();
    Ok(Json(
        state
            .budget_service
            .copy_period_targets(
                &payload.source_period_key,
                &payload.target_period_key,
                payload.overwrite,
                &base,
            )
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
            "/v1/spending/assignments/bulk",
            post(bulk_assign_categories),
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
            "/v1/spending/rule-presets/:preset_id",
            delete(remove_rule_preset),
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
        .route("/v1/spending/budget", get(get_budget))
        .route("/v1/spending/budget/targets", post(upsert_budget_target))
        .route(
            "/v1/spending/budget/targets/:id",
            delete(delete_budget_target),
        )
        .route(
            "/v1/spending/budget/rollovers",
            post(upsert_budget_rollover_setting),
        )
        .route(
            "/v1/spending/budget/rollovers/:id",
            delete(delete_budget_rollover_setting),
        )
        .route("/v1/spending/budget/groups", post(create_budget_group))
        .route(
            "/v1/spending/budget/groups/reset",
            post(reset_budget_groups),
        )
        .route(
            "/v1/spending/budget/groups/:id",
            put(update_budget_group).delete(delete_budget_group),
        )
        .route(
            "/v1/spending/budget/group-assignments",
            post(assign_category_to_group),
        )
        .route("/v1/spending/budget/copy", post(copy_budget_targets))
        .route("/v1/spending/report", post(get_spending_report))
}
