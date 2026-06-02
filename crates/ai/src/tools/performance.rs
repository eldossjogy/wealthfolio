//! Performance tool - fetch portfolio performance metrics using PerformanceService.

use chrono::{Datelike, Local, NaiveDate};
use rig::{completion::ToolDefinition, tool::Tool};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;
use wealthfolio_core::accounts::{account_supports_purpose, AccountPurpose};

// ============================================================================
// Tool Arguments and Output
// ============================================================================

/// Arguments for the get_performance tool.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPerformanceArgs {
    /// Account ID. Omit for all accounts.
    #[serde(default)]
    pub account_id: Option<String>,

    /// Period for performance calculation: "1M", "3M", "6M", "YTD", "1Y", "ALL".
    #[serde(default = "default_period")]
    pub period: String,
}

fn default_period() -> String {
    "YTD".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetPerformanceOutput {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_end_date: Option<String>,
    pub currency: String,
    pub mode: String,
    pub returns: PerformanceReturnsOutput,
    pub attribution: PerformanceAttributionOutput,
    pub risk: PerformanceRiskOutput,
    pub data_quality: PerformanceDataQualityOutput,
    pub is_mixed_tracking_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceReturnsOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twr: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annualized_twr: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub irr: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annualized_irr: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_return: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annualized_value_return: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceAttributionOutput {
    pub contributions: f64,
    pub distributions: f64,
    pub income: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl_change: f64,
    pub fx_effect: f64,
    pub fees: f64,
    pub taxes: f64,
    pub residual: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceRiskOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volatility: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_drawdown: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trough_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawdown_duration_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceDataQualityOutput {
    pub status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_applicable_reasons: Vec<String>,
}

// ============================================================================
// Tool Implementation
// ============================================================================

/// Tool to get portfolio performance.
pub struct GetPerformanceTool<E: AiEnvironment> {
    env: Arc<E>,
    base_currency: String,
}

impl<E: AiEnvironment> GetPerformanceTool<E> {
    pub fn new(env: Arc<E>, base_currency: String) -> Self {
        Self { env, base_currency }
    }
}

impl<E: AiEnvironment> Clone for GetPerformanceTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
            base_currency: self.base_currency.clone(),
        }
    }
}

/// Convert a period string to a start date.
fn period_to_start_date(period: &str, end_date: NaiveDate) -> Option<NaiveDate> {
    match period.to_uppercase().as_str() {
        "1M" => Some(end_date - chrono::Duration::days(30)),
        "3M" => Some(end_date - chrono::Duration::days(90)),
        "6M" => Some(end_date - chrono::Duration::days(180)),
        "YTD" => NaiveDate::from_ymd_opt(end_date.year(), 1, 1),
        "1Y" => Some(end_date - chrono::Duration::days(365)),
        _ => None, // None means no start date filter
    }
}

impl<E: AiEnvironment + 'static> Tool for GetPerformanceTool<E> {
    const NAME: &'static str = "get_performance";

    type Error = AiError;
    type Args = GetPerformanceArgs;
    type Output = GetPerformanceOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Get portfolio performance metrics including TWR, IRR, value return, attribution, volatility, and max drawdown. Omit accountId for aggregate performance across all accounts.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "accountId": {
                        "type": "string",
                        "description": "Account ID to get performance for. Omit for all accounts."
                    },
                    "period": {
                        "type": "string",
                        "description": "Time period for performance calculation",
                        "enum": ["1M", "3M", "6M", "YTD", "1Y", "ALL"],
                        "default": "YTD"
                    }
                },
                "required": []
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let account_id = args.account_id.as_deref().filter(|id| !id.is_empty());
        let period = args.period.to_uppercase();

        // Calculate date range
        let end_date = Local::now().date_naive();
        let start_date = period_to_start_date(&period, end_date);

        let metrics = if let Some(account_id) = account_id {
            let account = self
                .env
                .account_service()
                .get_account(account_id)
                .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;
            if !account.is_active
                || account.is_archived
                || !account_supports_purpose(&account.account_type, AccountPurpose::Performance)
            {
                return Ok(GetPerformanceOutput {
                    id: account_id.to_string(),
                    period_start_date: start_date.map(|d| d.to_string()),
                    period_end_date: Some(end_date.to_string()),
                    currency: account.currency,
                    mode: "notApplicable".to_string(),
                    data_quality: PerformanceDataQualityOutput {
                        status: "noData".to_string(),
                        warnings: Vec::new(),
                        not_applicable_reasons: vec![
                            "Performance unavailable for this account type.".to_string(),
                        ],
                    },
                    ..Default::default()
                });
            }
            self.env
                .performance_service()
                .calculate_performance_history(
                    "account",
                    account_id,
                    start_date,
                    Some(end_date),
                    Some(account.tracking_mode),
                )
                .await
                .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?
        } else {
            let accounts = self
                .env
                .account_service()
                .get_active_non_archived_accounts()
                .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;
            let mut account_tracking_modes = std::collections::HashMap::new();
            let account_ids: Vec<String> = accounts
                .into_iter()
                .filter(|account| {
                    account_supports_purpose(&account.account_type, AccountPurpose::Performance)
                })
                .map(|account| {
                    account_tracking_modes.insert(account.id.clone(), account.tracking_mode);
                    account.id
                })
                .collect();
            self.env
                .performance_service()
                .calculate_performance_history_for_accounts(
                    "all",
                    &account_ids,
                    &self.base_currency,
                    &account_tracking_modes,
                    start_date,
                    Some(end_date),
                )
                .await
                .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?
        };

        Ok(GetPerformanceOutput {
            id: metrics.scope.id,
            period_start_date: metrics.period.start_date.map(|d| d.to_string()),
            period_end_date: metrics.period.end_date.map(|d| d.to_string()),
            currency: if metrics.scope.currency.is_empty() {
                self.base_currency.clone()
            } else {
                metrics.scope.currency
            },
            mode: serde_json::to_value(metrics.mode)
                .ok()
                .and_then(|value| value.as_str().map(ToString::to_string))
                .unwrap_or_else(|| "notApplicable".to_string()),
            returns: PerformanceReturnsOutput {
                twr: metrics.returns.twr.and_then(|v| v.to_f64()),
                annualized_twr: metrics.returns.annualized_twr.and_then(|v| v.to_f64()),
                irr: metrics.returns.irr.and_then(|v| v.to_f64()),
                annualized_irr: metrics.returns.annualized_irr.and_then(|v| v.to_f64()),
                value_return: metrics.returns.value_return.and_then(|v| v.to_f64()),
                annualized_value_return: metrics
                    .returns
                    .annualized_value_return
                    .and_then(|v| v.to_f64()),
            },
            attribution: PerformanceAttributionOutput {
                contributions: metrics.attribution.contributions.to_f64().unwrap_or(0.0),
                distributions: metrics.attribution.distributions.to_f64().unwrap_or(0.0),
                income: metrics.attribution.income.to_f64().unwrap_or(0.0),
                realized_pnl: metrics.attribution.realized_pnl.to_f64().unwrap_or(0.0),
                unrealized_pnl_change: metrics
                    .attribution
                    .unrealized_pnl_change
                    .to_f64()
                    .unwrap_or(0.0),
                fx_effect: metrics.attribution.fx_effect.to_f64().unwrap_or(0.0),
                fees: metrics.attribution.fees.to_f64().unwrap_or(0.0),
                taxes: metrics.attribution.taxes.to_f64().unwrap_or(0.0),
                residual: metrics.attribution.residual.to_f64().unwrap_or(0.0),
            },
            risk: PerformanceRiskOutput {
                volatility: metrics.risk.volatility.and_then(|v| v.to_f64()),
                max_drawdown: metrics.risk.max_drawdown.and_then(|v| v.to_f64()),
                peak_date: metrics.risk.peak_date.map(|d| d.to_string()),
                trough_date: metrics.risk.trough_date.map(|d| d.to_string()),
                recovery_date: metrics.risk.recovery_date.map(|d| d.to_string()),
                drawdown_duration_days: metrics.risk.drawdown_duration_days,
            },
            data_quality: PerformanceDataQualityOutput {
                status: serde_json::to_value(metrics.data_quality.status)
                    .ok()
                    .and_then(|value| value.as_str().map(ToString::to_string))
                    .unwrap_or_else(|| "partial".to_string()),
                warnings: metrics.data_quality.warnings,
                not_applicable_reasons: metrics.data_quality.not_applicable_reasons,
            },
            is_mixed_tracking_mode: metrics.is_mixed_tracking_mode,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::test_env::{MockAccountService, MockEnvironment};
    use chrono::Utc;
    use wealthfolio_core::accounts::{Account, TrackingMode};

    #[tokio::test]
    async fn test_get_performance_tool() {
        let env = Arc::new(MockEnvironment::new());
        let tool = GetPerformanceTool::new(env, "USD".to_string());

        let result = tool
            .call(GetPerformanceArgs {
                account_id: None,
                period: "YTD".to_string(),
            })
            .await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.currency, "USD");
    }

    #[tokio::test]
    async fn test_get_performance_with_account_id() {
        let mut env = MockEnvironment::new();
        env.account_service = Arc::new(MockAccountService {
            accounts: vec![test_account("acc-123", "SECURITIES")],
        });
        let env = Arc::new(env);
        let tool = GetPerformanceTool::new(env, "USD".to_string());

        let result = tool
            .call(GetPerformanceArgs {
                account_id: Some("acc-123".to_string()),
                period: "1M".to_string(),
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_performance_returns_empty_metrics_for_credit_card() {
        let mut env = MockEnvironment::new();
        env.account_service = Arc::new(MockAccountService {
            accounts: vec![test_account("card-1", "CREDIT_CARD")],
        });
        let env = Arc::new(env);
        let tool = GetPerformanceTool::new(env, "USD".to_string());

        let output = tool
            .call(GetPerformanceArgs {
                account_id: Some("card-1".to_string()),
                period: "1M".to_string(),
            })
            .await
            .expect("credit cards should return an empty performance response");

        assert_eq!(output.id, "card-1");
        assert_eq!(output.currency, "USD");
        assert_eq!(output.returns.twr, None);
        assert_eq!(output.returns.irr, None);
    }

    #[tokio::test]
    async fn test_period_conversion() {
        let today = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();

        // Test YTD
        let ytd_start = period_to_start_date("YTD", today);
        assert_eq!(ytd_start, NaiveDate::from_ymd_opt(2024, 1, 1));

        // Test 1M (30 days back)
        let one_month_start = period_to_start_date("1M", today);
        assert_eq!(one_month_start, NaiveDate::from_ymd_opt(2024, 5, 16));

        // Test 1Y (365 days back)
        let one_year_start = period_to_start_date("1Y", today);
        assert_eq!(one_year_start, NaiveDate::from_ymd_opt(2023, 6, 16));

        // Test ALL - returns None (no start date filter)
        let all_start = period_to_start_date("ALL", today);
        assert_eq!(all_start, None);
    }

    fn test_account(id: &str, account_type: &str) -> Account {
        let now = Utc::now().naive_utc();
        Account {
            id: id.to_string(),
            name: id.to_string(),
            account_type: account_type.to_string(),
            group: None,
            currency: "USD".to_string(),
            is_default: false,
            is_active: true,
            created_at: now,
            updated_at: now,
            platform_id: None,
            account_number: None,
            meta: None,
            provider: None,
            provider_account_id: None,
            is_archived: false,
            tracking_mode: TrackingMode::Transactions,
        }
    }
}
