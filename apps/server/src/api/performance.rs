use std::{collections::HashMap, sync::Arc};

use crate::{error::ApiResult, main_lib::AppState};
use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use wealthfolio_core::{
    accounts::{account_supports_purpose, AccountPurpose, AccountServiceTrait, TrackingMode},
    portfolio::{
        income::IncomeSummary,
        performance::{
            DataQualityStatus, PerformanceAttribution, PerformanceDataQuality, PerformancePeriod,
            PerformanceResult, PerformanceReturns, PerformanceRisk, PerformanceScopeDescriptor,
            ReturnMethod, SimplePerformanceMetrics,
        },
    },
    portfolios::AccountScope,
};

use super::shared::parse_date_optional;

#[derive(serde::Deserialize)]
struct AccountsSimplePerfBody {
    #[serde(rename = "accountIds")]
    account_ids: Option<Vec<String>>,
}

async fn calculate_accounts_simple_performance(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AccountsSimplePerfBody>,
) -> ApiResult<Json<Vec<SimplePerformanceMetrics>>> {
    let ids: Vec<String> = if let Some(ids) = body.account_ids {
        performance_account_ids(&state, &ids)?
    } else {
        state
            .account_service
            .get_active_non_archived_accounts()?
            .into_iter()
            .filter(|account| {
                account_supports_purpose(&account.account_type, AccountPurpose::Performance)
            })
            .map(|account| account.id)
            .collect()
    };
    if ids.is_empty() {
        return Ok(Json(Vec::new()));
    }
    let metrics = state
        .performance_service
        .calculate_accounts_simple_performance(&ids)?;
    Ok(Json(metrics))
}

#[derive(serde::Deserialize)]
struct PerfBody {
    #[serde(rename = "itemType")]
    item_type: String,
    #[serde(rename = "itemId")]
    item_id: String,
    #[serde(rename = "startDate")]
    start_date: Option<String>,
    #[serde(rename = "endDate")]
    end_date: Option<String>,
    #[serde(rename = "trackingMode")]
    tracking_mode: Option<String>,
    filter: Option<AccountScope>,
}

#[derive(serde::Deserialize)]
struct PerformanceSummaryScopeBody {
    #[serde(rename = "accountIds")]
    account_ids: Vec<String>,
}

#[derive(serde::Deserialize)]
struct PerformanceSummariesBody {
    scopes: Vec<PerformanceSummaryScopeBody>,
    #[serde(rename = "startDate")]
    start_date: Option<String>,
    #[serde(rename = "endDate")]
    end_date: Option<String>,
}

fn performance_summary_scope_key(account_ids: &[String]) -> String {
    let mut sorted = account_ids.to_vec();
    sorted.sort();
    sorted.dedup();
    format!("accounts:{}", sorted.join(","))
}

fn parse_tracking_mode(mode: Option<String>) -> Option<TrackingMode> {
    mode.and_then(|m| match m.as_str() {
        "HOLDINGS" => Some(TrackingMode::Holdings),
        "TRANSACTIONS" => Some(TrackingMode::Transactions),
        _ => None,
    })
}

fn account_ids_for_purpose(
    state: &AppState,
    account_ids: &[String],
    purpose: AccountPurpose,
) -> ApiResult<Vec<String>> {
    Ok(state
        .account_service
        .get_accounts_by_ids(account_ids)?
        .into_iter()
        .filter(|account| account_supports_purpose(&account.account_type, purpose))
        .map(|account| account.id)
        .collect())
}

fn empty_performance_metrics(
    id: &str,
    currency: String,
    start_date: Option<chrono::NaiveDate>,
    end_date: Option<chrono::NaiveDate>,
) -> PerformanceResult {
    PerformanceResult {
        scope: PerformanceScopeDescriptor {
            id: id.to_string(),
            currency,
        },
        period: PerformancePeriod {
            start_date,
            end_date,
        },
        mode: ReturnMethod::NotApplicable,
        returns: PerformanceReturns {
            twr: None,
            annualized_twr: None,
            irr: None,
            annualized_irr: None,
            value_return: None,
            annualized_value_return: None,
        },
        attribution: PerformanceAttribution::default(),
        risk: PerformanceRisk {
            volatility: None,
            max_drawdown: None,
            peak_date: None,
            trough_date: None,
            recovery_date: None,
            drawdown_duration_days: None,
        },
        data_quality: PerformanceDataQuality {
            status: DataQualityStatus::NoData,
            warnings: Vec::new(),
            not_applicable_reasons: vec![
                "Performance unavailable for this account type.".to_string()
            ],
        },
        series: Vec::new(),
        is_holdings_mode: false,
        is_mixed_tracking_mode: false,
    }
}

fn account_tracking_modes(
    state: &AppState,
    account_ids: &[String],
) -> Result<HashMap<String, TrackingMode>, crate::error::ApiError> {
    Ok(state
        .account_service
        .get_accounts_by_ids(account_ids)?
        .into_iter()
        .map(|account| (account.id, account.tracking_mode))
        .collect())
}

fn performance_account_ids(
    state: &AppState,
    account_ids: &[String],
) -> Result<Vec<String>, crate::error::ApiError> {
    Ok(state
        .account_service
        .get_accounts_by_ids(account_ids)?
        .into_iter()
        .filter(|account| {
            account.is_active
                && !account.is_archived
                && account_supports_purpose(&account.account_type, AccountPurpose::Performance)
        })
        .map(|account| account.id)
        .collect())
}

async fn calculate_performance_history(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PerfBody>,
) -> ApiResult<Json<PerformanceResult>> {
    let start = parse_date_optional(body.start_date, "startDate")?;
    let end = parse_date_optional(body.end_date, "endDate")?;
    let tracking_mode = parse_tracking_mode(body.tracking_mode);
    let metrics = if let (true, Some(filter)) = (body.item_type == "account", body.filter.as_ref())
    {
        let base = state.base_currency.read().unwrap().clone();
        let resolved = state
            .portfolio_service
            .resolve_account_scope(filter, &base)
            .map_err(crate::error::ApiError::from)?;
        let account_ids = performance_account_ids(&state, &resolved.account_ids)?;
        if account_ids.is_empty() {
            let mut result = empty_performance_metrics(
                &resolved.scope_id,
                resolved.base_currency.clone(),
                start,
                end,
            );
            if !resolved.account_ids.is_empty() {
                result.data_quality.warnings.push(
                    "Requested accounts were excluded because they are inactive, archived, or not eligible for performance."
                        .to_string(),
                );
            }
            return Ok(Json(result));
        }
        let tracking_modes = account_tracking_modes(&state, &account_ids)?;
        let mut result = state
            .performance_service
            .calculate_performance_history_for_accounts(
                &resolved.scope_id,
                &account_ids,
                &resolved.base_currency,
                &tracking_modes,
                start,
                end,
            )
            .await?;
        if account_ids.len() != resolved.account_ids.len() {
            result.data_quality.warnings.push(
                "Some requested accounts were excluded because they are inactive, archived, or not eligible for performance."
                    .to_string(),
            );
            result.data_quality.status = DataQualityStatus::Partial;
        }
        result
    } else {
        let authoritative_tracking_mode = if body.item_type == "account" {
            let account = state.account_service.get_account(&body.item_id)?;
            if !account.is_active
                || account.is_archived
                || !account_supports_purpose(&account.account_type, AccountPurpose::Performance)
            {
                return Ok(Json(empty_performance_metrics(
                    &body.item_id,
                    account.currency,
                    start,
                    end,
                )));
            }
            Some(account.tracking_mode)
        } else {
            tracking_mode
        };
        state
            .performance_service
            .calculate_performance_history(
                &body.item_type,
                &body.item_id,
                start,
                end,
                authoritative_tracking_mode,
            )
            .await?
    };
    Ok(Json(metrics))
}

async fn calculate_performance_summary(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PerfBody>,
) -> ApiResult<Json<PerformanceResult>> {
    let start = parse_date_optional(body.start_date, "startDate")?;
    let end = parse_date_optional(body.end_date, "endDate")?;
    let tracking_mode = parse_tracking_mode(body.tracking_mode);
    let metrics = if let (true, Some(filter)) = (body.item_type == "account", body.filter.as_ref())
    {
        let base = state.base_currency.read().unwrap().clone();
        let resolved = state
            .portfolio_service
            .resolve_account_scope(filter, &base)
            .map_err(crate::error::ApiError::from)?;
        let account_ids = performance_account_ids(&state, &resolved.account_ids)?;
        if account_ids.is_empty() {
            let mut result = empty_performance_metrics(
                &resolved.scope_id,
                resolved.base_currency.clone(),
                start,
                end,
            );
            if !resolved.account_ids.is_empty() {
                result.data_quality.warnings.push(
                    "Requested accounts were excluded because they are inactive, archived, or not eligible for performance."
                        .to_string(),
                );
            }
            return Ok(Json(result));
        }
        let tracking_modes = account_tracking_modes(&state, &account_ids)?;
        let mut result = state
            .performance_service
            .calculate_performance_summary_for_accounts(
                &resolved.scope_id,
                &account_ids,
                &resolved.base_currency,
                &tracking_modes,
                start,
                end,
            )
            .await?;
        if account_ids.len() != resolved.account_ids.len() {
            result.data_quality.warnings.push(
                "Some requested accounts were excluded because they are inactive, archived, or not eligible for performance."
                    .to_string(),
            );
            result.data_quality.status = DataQualityStatus::Partial;
        }
        result
    } else {
        let authoritative_tracking_mode = if body.item_type == "account" {
            let account = state.account_service.get_account(&body.item_id)?;
            if !account.is_active
                || account.is_archived
                || !account_supports_purpose(&account.account_type, AccountPurpose::Performance)
            {
                return Ok(Json(empty_performance_metrics(
                    &body.item_id,
                    account.currency,
                    start,
                    end,
                )));
            }
            Some(account.tracking_mode)
        } else {
            tracking_mode
        };
        state
            .performance_service
            .calculate_performance_summary(
                &body.item_type,
                &body.item_id,
                start,
                end,
                authoritative_tracking_mode,
            )
            .await?
    };
    Ok(Json(metrics))
}

async fn get_performance_summaries(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PerformanceSummariesBody>,
) -> ApiResult<Json<HashMap<String, PerformanceResult>>> {
    let start = parse_date_optional(body.start_date, "startDate")?;
    let end = parse_date_optional(body.end_date, "endDate")?;
    let base = state.base_currency.read().unwrap().clone();
    let mut results = HashMap::new();

    for scope in body.scopes {
        let key = performance_summary_scope_key(&scope.account_ids);
        let account_ids = performance_account_ids(&state, &scope.account_ids)?;

        if account_ids.is_empty() {
            let mut result = empty_performance_metrics(&key, base.clone(), start, end);
            if !scope.account_ids.is_empty() {
                result.data_quality.warnings.push(
                    "Requested accounts were excluded because they are inactive, archived, or not eligible for performance."
                        .to_string(),
                );
            }
            results.insert(key.clone(), result);
            continue;
        }

        let mut result = state
            .performance_service
            .calculate_performance_summary_for_accounts(
                &key,
                &account_ids,
                &base,
                &account_tracking_modes(&state, &account_ids)?,
                start,
                end,
            )
            .await?;

        if account_ids.len() != scope.account_ids.len() {
            result.data_quality.warnings.push(
                "Some requested accounts were excluded because they are inactive, archived, or not eligible for performance."
                    .to_string(),
            );
            result.data_quality.status = DataQualityStatus::Partial;
        }

        results.insert(key, result);
    }

    Ok(Json(results))
}

#[derive(serde::Deserialize)]
struct IncomeSummaryAccountQuery {
    #[serde(rename = "accountId")]
    account_id: Option<String>,
}

/// GET /income/summary?accountId=... — single-account or all-accounts scope
async fn get_income_summary_for_account(
    State(state): State<Arc<AppState>>,
    Query(q): Query<IncomeSummaryAccountQuery>,
) -> ApiResult<Json<Vec<IncomeSummary>>> {
    let account_ids: Vec<String> = if let Some(id) = q.account_id {
        account_ids_for_purpose(&state, &[id], AccountPurpose::Income)?
    } else {
        state
            .account_service
            .get_active_accounts()?
            .into_iter()
            .filter(|account| {
                account_supports_purpose(&account.account_type, AccountPurpose::Income)
            })
            .map(|account| account.id)
            .collect()
    };
    if account_ids.is_empty() {
        return Ok(Json(Vec::new()));
    }
    let items = state
        .income_service
        .get_income_summary(Some(&account_ids))?;
    Ok(Json(items))
}

#[derive(serde::Deserialize)]
struct IncomeSummaryBody {
    filter: Option<wealthfolio_core::portfolios::AccountScope>,
}

/// POST /income/summary/query — typed scope query (all, portfolio, multi-account)
async fn get_income_summary(
    State(state): State<Arc<AppState>>,
    Json(body): Json<IncomeSummaryBody>,
) -> ApiResult<Json<Vec<IncomeSummary>>> {
    let account_ids: Vec<String> = match &body.filter {
        None => state
            .account_service
            .get_active_accounts()?
            .into_iter()
            .filter(|account| {
                account_supports_purpose(&account.account_type, AccountPurpose::Income)
            })
            .map(|account| account.id)
            .collect(),
        Some(filter) => {
            let base = state.base_currency.read().unwrap().clone();
            let resolved = state
                .portfolio_service
                .resolve_account_scope(filter, &base)
                .map_err(crate::error::ApiError::from)?;
            account_ids_for_purpose(&state, &resolved.account_ids, AccountPurpose::Income)?
        }
    };
    if account_ids.is_empty() {
        return Ok(Json(Vec::new()));
    }
    let items = state
        .income_service
        .get_income_summary(Some(&account_ids))?;
    Ok(Json(items))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/performance/accounts/simple",
            post(calculate_accounts_simple_performance),
        )
        .route("/performance/history", post(calculate_performance_history))
        .route("/performance/summary", post(calculate_performance_summary))
        .route("/performance/summaries", post(get_performance_summaries))
        .route("/income/summary", get(get_income_summary_for_account))
        .route("/income/summary/query", post(get_income_summary))
}
