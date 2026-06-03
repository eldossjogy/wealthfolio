use async_trait::async_trait;
use log::debug;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use crate::errors::{Error as CoreError, Result as CoreResult};
use crate::portfolio::allocation::{AllocationServiceTrait, HoldingAllocationContribution};
use crate::portfolio::holdings::{HoldingType, HoldingsServiceTrait};

use super::drift_service::DriftServiceTrait;
use super::model::{
    CalculateRebalancePlanInput, RebalancePlan, RebalanceWarning, RebalanceWarningKind,
};
use super::optimizer::{
    AssetCandidate, CategoryState, DriftPriorityOptimizer, RebalanceInput, RebalanceOptimizer,
    RebalanceProfile,
};
use super::target_service::AllocationTargetServiceTrait;

// ── Service trait ─────────────────────────────────────────────────────────────

#[async_trait]
pub trait RebalanceServiceTrait: Send + Sync {
    async fn calculate_plan(&self, input: CalculateRebalancePlanInput)
        -> CoreResult<RebalancePlan>;
}

// ── Implementation ────────────────────────────────────────────────────────────

pub struct RebalanceService {
    allocation_target_service: Arc<dyn AllocationTargetServiceTrait>,
    drift_service: Arc<dyn DriftServiceTrait>,
    allocation_service: Arc<dyn AllocationServiceTrait>,
    holdings_service: Arc<dyn HoldingsServiceTrait>,
}

impl RebalanceService {
    pub fn new(
        allocation_target_service: Arc<dyn AllocationTargetServiceTrait>,
        drift_service: Arc<dyn DriftServiceTrait>,
        allocation_service: Arc<dyn AllocationServiceTrait>,
        holdings_service: Arc<dyn HoldingsServiceTrait>,
    ) -> Self {
        Self {
            allocation_target_service,
            drift_service,
            allocation_service,
            holdings_service,
        }
    }

    fn currency_fraction_digits(currency: &str) -> u32 {
        match currency.to_ascii_uppercase().as_str() {
            "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF"
            | "UGX" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
            "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
            "CLF" => 4,
            _ => 2,
        }
    }

    fn currency_rounding_tolerance(currency: &str) -> Decimal {
        Decimal::ONE / Decimal::from(10_i64.pow(Self::currency_fraction_digits(currency)))
    }

    /// Build `AssetCandidate` list from holdings + contributions.
    ///
    /// Rules (Afadil, #1036):
    /// - Holdings with no taxonomy assignments → `__UNKNOWN__` in contributions → warn
    ///   `UnclassifiedAsset`, skip.
    /// - Holdings with partial weights (<100%) → contributions already include an
    ///   `__UNKNOWN__` remainder row (from `AllocationService`). Use the known exposure
    ///   as-is and warn `PartialClassification`. Do NOT normalise.
    /// - Weights >100%: `AllocationService` normalises silently to 100%; we align with
    ///   that behaviour for consistency. (Open question for Afadil in PR-B description.)
    /// - Cash holdings: excluded from candidates.
    /// - Holdings with no usable price: skip (warn `MissingQuote` if whole_shares_only).
    fn build_candidates(
        contributions: &[HoldingAllocationContribution],
        price_by_holding: &HashMap<String, Decimal>,
        whole_shares_only: bool,
    ) -> (Vec<AssetCandidate>, Vec<RebalanceWarning>) {
        // Group contributions by holding_id.
        let mut by_holding: HashMap<&str, Vec<&HoldingAllocationContribution>> = HashMap::new();
        for c in contributions {
            by_holding.entry(c.holding_id.as_str()).or_default().push(c);
        }

        let mut candidates: Vec<AssetCandidate> = Vec::new();
        let mut warnings: Vec<RebalanceWarning> = Vec::new();

        // Iterate holdings in a stable order so emitted warnings are reproducible
        // run-to-run (HashMap iteration order is otherwise non-deterministic).
        let mut holding_ids: Vec<&str> = by_holding.keys().copied().collect();
        holding_ids.sort_unstable();

        for holding_id in holding_ids {
            let contribs = &by_holding[holding_id];
            // Skip cash holdings.
            if contribs.iter().all(|c| c.holding_type == HoldingType::Cash) {
                continue;
            }

            let repr = contribs[0];
            let symbol = &repr.symbol;

            // Check classification state.
            let all_unknown = contribs.iter().all(|c| c.category_id == "__UNKNOWN__");
            let has_unknown = contribs.iter().any(|c| c.category_id == "__UNKNOWN__");

            if all_unknown {
                warnings.push(RebalanceWarning {
                    kind: RebalanceWarningKind::UnclassifiedAsset,
                    category_id: "__UNKNOWN__".to_string(),
                    message: format!(
                        "{}: no taxonomy assignments for this taxonomy. Classify it to include in the drift planner.",
                        symbol
                    ),
                });
                continue;
            }

            if has_unknown {
                warnings.push(RebalanceWarning {
                    kind: RebalanceWarningKind::PartialClassification,
                    category_id: repr.category_id.clone(),
                    message: format!(
                        "{}: partial classification (<100%). Known exposure used; unclassified remainder ignored.",
                        symbol
                    ),
                });
            }

            // Derive price.
            let price = match price_by_holding.get(holding_id) {
                Some(&p) if p > Decimal::ZERO => p,
                _ => {
                    if whole_shares_only {
                        warnings.push(RebalanceWarning {
                            kind: RebalanceWarningKind::MissingQuote,
                            category_id: repr.category_id.clone(),
                            message: format!(
                                "{}: no valid price for whole-share mode. Skipped.",
                                symbol
                            ),
                        });
                    }
                    continue;
                }
            };

            // Build exposure per share: contribution.value / quantity, excluding __UNKNOWN__.
            let qty = repr.quantity;
            if qty <= Decimal::ZERO {
                continue;
            }
            let mut exposure_per_share: HashMap<String, Decimal> = HashMap::new();
            for c in contribs.iter() {
                if c.category_id == "__UNKNOWN__" {
                    continue;
                }
                *exposure_per_share.entry(c.category_id.clone()).or_default() += c.value / qty;
            }

            if exposure_per_share.is_empty() {
                continue;
            }

            candidates.push(AssetCandidate {
                holding_id: holding_id.to_string(),
                asset_id: repr.asset_id.clone(),
                symbol: symbol.clone(),
                name: Some(repr.name.clone()),
                price,
                exposure_per_share,
            });
        }

        (candidates, warnings)
    }
}

#[async_trait]
impl RebalanceServiceTrait for RebalanceService {
    async fn calculate_plan(
        &self,
        input: CalculateRebalancePlanInput,
    ) -> CoreResult<RebalancePlan> {
        debug!("Calculating rebalance plan for target {}", input.target_id);

        if input.available_cash < Decimal::ZERO {
            return Err(CoreError::Validation(
                crate::errors::ValidationError::InvalidInput(
                    "available_cash must be non-negative".to_string(),
                ),
            ));
        }

        // Fetch holdings once — used for both cash check and price extraction.
        let all_holdings = self
            .holdings_service
            .get_holdings_for_accounts(
                &input.account_ids,
                &input.base_currency,
                &input.aggregated_account_id,
            )
            .await?;

        // Authoritative tracked cash in scope.
        let cash_in_scope: Decimal = all_holdings
            .iter()
            .filter(|h| h.holding_type == HoldingType::Cash)
            .map(|h| h.market_value.base)
            .sum();

        let available_cash = if input.available_cash > cash_in_scope {
            let overage = input.available_cash - cash_in_scope;
            if overage <= Self::currency_rounding_tolerance(&input.base_currency) {
                cash_in_scope
            } else {
                return Err(CoreError::Validation(
                    crate::errors::ValidationError::InvalidInput(
                        "cash to deploy exceeds tracked cash in scope".to_string(),
                    ),
                ));
            }
        } else {
            input.available_cash
        };

        // Load profile.
        let profile = self
            .allocation_target_service
            .get_target(&input.target_id)?
            .ok_or_else(|| {
                CoreError::Database(crate::errors::DatabaseError::NotFound(format!(
                    "AllocationTarget {} not found",
                    input.target_id
                )))
            })?;

        // Drift report — provides total_value and per-category target/current data.
        let drift = self
            .drift_service
            .get_drift_report_for_target(
                &input.target_id,
                &input.account_ids,
                &input.base_currency,
                &input.aggregated_account_id,
            )
            .await?;

        let total_value = drift.total_value;

        if total_value == Decimal::ZERO && available_cash == Decimal::ZERO {
            return Ok(RebalancePlan {
                target_id: input.target_id,
                available_cash: Decimal::ZERO,
                cash_used: Decimal::ZERO,
                cash_remaining: Decimal::ZERO,
                max_drift_bps_before: 0,
                max_drift_bps_after: 0,
                trades: vec![],
                warnings: vec![],
                after_bps_by_category: std::collections::HashMap::new(),
            });
        }

        // Holding contributions for exposure vectors.
        let taxonomy_contributions = self
            .allocation_service
            .get_holding_contributions_for_taxonomy_for_accounts(
                &input.account_ids,
                &input.base_currency,
                &profile.taxonomy_id,
                &input.aggregated_account_id,
            )
            .await?;

        // Price map: holding_id → price per share in base currency.
        let price_by_holding: HashMap<String, Decimal> = all_holdings
            .iter()
            .filter(|h| h.holding_type != HoldingType::Cash)
            .filter_map(|h| {
                let price = h.price.filter(|&p| p > Decimal::ZERO).or_else(|| {
                    if h.quantity > Decimal::ZERO {
                        Some(h.market_value.base / h.quantity)
                    } else {
                        None
                    }
                })?;
                Some((h.id.clone(), price))
            })
            .collect();

        let (candidates, classification_warnings) = Self::build_candidates(
            &taxonomy_contributions.contributions,
            &price_by_holding,
            profile.whole_shares_only,
        );

        // Map drift rows to CategoryState for the optimizer.
        let categories: Vec<CategoryState> = drift
            .rows
            .iter()
            .map(|row| CategoryState {
                category_id: row.category_id.clone(),
                category_name: row.category_name.clone(),
                target_bps: row.target_bps,
                current_value: row.current_value,
                is_cash: row.is_cash,
                is_required: row.is_required,
            })
            .collect();

        let optimizer_input = RebalanceInput {
            profile: RebalanceProfile {
                target_id: input.target_id.clone(),
                drift_band_bps: profile.drift_band_bps,
                rebalance_goal: profile.rebalance_goal.clone(),
                min_trade_amount: Decimal::from_str(&profile.min_trade_amount)
                    .unwrap_or(Decimal::ZERO),
                whole_shares_only: profile.whole_shares_only,
            },
            available_cash,
            total_value,
            categories,
            candidates,
            warnings: classification_warnings,
        };

        DriftPriorityOptimizer.plan(optimizer_input)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portfolio::allocation::{
        AllocationHoldings, HoldingAllocationContribution, PortfolioAllocations,
        TaxonomyHoldingContributions,
    };
    use crate::portfolio::allocation_targets::{
        AllocationTarget, AllocationTargetWeight, DriftReport, DriftRow, DriftStatus,
        NewAllocationTarget, NewAllocationTargetWeight, RebalanceGoal, ScopeType, TriggerType,
    };
    use crate::portfolio::holdings::{Holding, HoldingType, Instrument, MonetaryValue};
    use rust_decimal_macros::dec;

    // ── Test helpers ─────────────────────────────────────────────────────────

    fn make_profile(rebalance_goal: RebalanceGoal, whole_shares_only: bool) -> AllocationTarget {
        AllocationTarget {
            id: "profile-1".to_string(),
            name: "Test".to_string(),
            scope_type: ScopeType::All,
            scope_id: None,
            taxonomy_id: "asset_classes".to_string(),
            trigger_type: TriggerType::Threshold,
            drift_band_bps: 500,
            rebalance_goal,
            min_trade_amount: "0".to_string(),
            whole_shares_only,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            archived_at: None,
        }
    }

    fn make_drift_row(
        category_id: &str,
        current_bps: i32,
        target_bps: i32,
        total_value: Decimal,
    ) -> DriftRow {
        let drift_bps = current_bps - target_bps;
        let current_value = Decimal::from(current_bps) / dec!(10000) * total_value;
        let target_value = Decimal::from(target_bps) / dec!(10000) * total_value;
        let value_delta = current_value - target_value;
        let status = if drift_bps.abs() <= 500 {
            DriftStatus::InBand
        } else if drift_bps < 0 {
            DriftStatus::Underweight
        } else {
            DriftStatus::Overweight
        };
        DriftRow {
            category_id: category_id.to_string(),
            category_name: category_id.to_string(),
            color: "#aaa".to_string(),
            current_bps,
            target_bps,
            drift_bps,
            current_value,
            target_value,
            value_delta,
            status,
            is_required: true,
            is_zero_current: current_bps == 0,
            is_cash: false,
        }
    }

    fn make_holding(id: &str, symbol: &str, quantity: Decimal, market_value: Decimal) -> Holding {
        let price = if quantity > Decimal::ZERO {
            Some(market_value / quantity)
        } else {
            None
        };
        Holding {
            id: id.to_string(),
            account_id: "acc-1".to_string(),
            holding_type: HoldingType::Security,
            instrument: Some(Instrument {
                id: id.to_string(),
                symbol: symbol.to_string(),
                name: Some(symbol.to_string()),
                currency: "USD".to_string(),
                notes: None,
                pricing_mode: "auto".to_string(),
                preferred_provider: None,
                exchange_mic: None,
                classifications: None,
            }),
            asset_kind: None,
            quantity,
            open_date: None,
            lots: None,
            contract_multiplier: Decimal::ONE,
            local_currency: "USD".to_string(),
            base_currency: "USD".to_string(),
            fx_rate: None,
            market_value: MonetaryValue {
                local: market_value,
                base: market_value,
            },
            cost_basis: None,
            price,
            purchase_price: None,
            unrealized_gain: None,
            unrealized_gain_pct: None,
            realized_gain: None,
            realized_gain_pct: None,
            total_gain: None,
            total_gain_pct: None,
            day_change: None,
            day_change_pct: None,
            prev_close_value: None,
            weight: Decimal::ZERO,
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            metadata: None,
            source_account_ids: vec![],
        }
    }

    fn make_cash_holding(amount: Decimal, currency: &str) -> Holding {
        Holding {
            id: "cash".to_string(),
            account_id: "acc-1".to_string(),
            holding_type: HoldingType::Cash,
            instrument: None,
            asset_kind: None,
            quantity: amount,
            open_date: None,
            lots: None,
            contract_multiplier: Decimal::ONE,
            local_currency: currency.to_string(),
            base_currency: currency.to_string(),
            fx_rate: None,
            market_value: MonetaryValue {
                local: amount,
                base: amount,
            },
            cost_basis: None,
            price: None,
            purchase_price: None,
            unrealized_gain: None,
            unrealized_gain_pct: None,
            realized_gain: None,
            realized_gain_pct: None,
            total_gain: None,
            total_gain_pct: None,
            day_change: None,
            day_change_pct: None,
            prev_close_value: None,
            weight: Decimal::ZERO,
            as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            metadata: None,
            source_account_ids: vec![],
        }
    }

    /// Build a single-category `HoldingAllocationContribution` (100% classified).
    fn make_contribution(
        holding: &Holding,
        category_id: &str,
        value: Decimal,
    ) -> HoldingAllocationContribution {
        HoldingAllocationContribution {
            id: format!("{}:{}", holding.id, category_id),
            holding_id: holding.id.clone(),
            asset_id: holding
                .instrument
                .as_ref()
                .map(|i| i.id.clone())
                .unwrap_or_default(),
            account_id: holding.account_id.clone(),
            source_account_ids: vec![],
            symbol: holding
                .instrument
                .as_ref()
                .map(|i| i.symbol.clone())
                .unwrap_or_default(),
            name: holding
                .instrument
                .as_ref()
                .and_then(|i| i.name.clone())
                .unwrap_or_default(),
            holding_type: holding.holding_type.clone(),
            quantity: holding.quantity,
            category_id: category_id.to_string(),
            category_name: category_id.to_string(),
            category_color: "#aaa".to_string(),
            value,
        }
    }

    fn make_contributions(
        contribs: Vec<HoldingAllocationContribution>,
    ) -> TaxonomyHoldingContributions {
        let total: Decimal = contribs.iter().map(|c| c.value).sum();
        TaxonomyHoldingContributions {
            taxonomy_id: "asset_classes".to_string(),
            taxonomy_name: "Asset Classes".to_string(),
            total_value: total,
            currency: "USD".to_string(),
            contributions: contribs,
        }
    }

    fn make_report(rows: Vec<DriftRow>, total_value: Decimal) -> DriftReport {
        let max = rows
            .iter()
            .map(|r| r.drift_bps.unsigned_abs())
            .max()
            .unwrap_or(0);
        let out = rows
            .iter()
            .filter(|r| r.drift_bps.unsigned_abs() > 500)
            .count();
        DriftReport {
            target_id: "profile-1".to_string(),
            scope_type: ScopeType::All,
            scope_id: None,
            total_value,
            base_currency: "USD".to_string(),
            max_drift_bps: max as i32,
            out_of_band_count: out,
            rows,
            holdings: None,
        }
    }

    // ── Mocks ─────────────────────────────────────────────────────────────────

    struct MockTargetService {
        profile: AllocationTarget,
    }

    #[async_trait]
    impl AllocationTargetServiceTrait for MockTargetService {
        fn get_target(&self, _: &str) -> CoreResult<Option<AllocationTarget>> {
            Ok(Some(self.profile.clone()))
        }
        fn list_targets(&self) -> CoreResult<Vec<AllocationTarget>> {
            Ok(vec![])
        }
        fn list_weights_for_target(&self, _: &str) -> CoreResult<Vec<AllocationTargetWeight>> {
            Ok(vec![])
        }
        async fn create_target(&self, _: NewAllocationTarget) -> CoreResult<AllocationTarget> {
            unimplemented!()
        }
        async fn update_target(
            &self,
            _: &str,
            _: NewAllocationTarget,
        ) -> CoreResult<AllocationTarget> {
            unimplemented!()
        }
        async fn archive_target(&self, _: &str) -> CoreResult<AllocationTarget> {
            unimplemented!()
        }
        async fn delete_target(&self, _: &str) -> CoreResult<()> {
            unimplemented!()
        }
        async fn save_weights(
            &self,
            _: &str,
            _: Vec<NewAllocationTargetWeight>,
        ) -> CoreResult<Vec<AllocationTargetWeight>> {
            unimplemented!()
        }
        async fn save_target_with_weights(
            &self,
            _: Option<String>,
            _: NewAllocationTarget,
            _: Vec<NewAllocationTargetWeight>,
        ) -> CoreResult<crate::portfolio::allocation_targets::SaveAllocationTargetResult> {
            unimplemented!()
        }
    }

    struct MockDriftService {
        report: DriftReport,
    }

    #[async_trait]
    impl DriftServiceTrait for MockDriftService {
        async fn get_drift_report_for_target(
            &self,
            _: &str,
            _: &[String],
            _: &str,
            _: &str,
        ) -> CoreResult<DriftReport> {
            Ok(self.report.clone())
        }
        async fn get_drift_report_with_holdings_for_target(
            &self,
            _: &str,
            _: &[String],
            _: &str,
            _: &str,
        ) -> CoreResult<DriftReport> {
            Ok(self.report.clone())
        }
    }

    struct MockAllocationService {
        contributions: TaxonomyHoldingContributions,
    }

    #[async_trait]
    impl AllocationServiceTrait for MockAllocationService {
        async fn get_portfolio_allocations(
            &self,
            _: &str,
            _: &str,
        ) -> CoreResult<PortfolioAllocations> {
            unimplemented!()
        }
        async fn get_portfolio_allocations_for_accounts(
            &self,
            _: &[String],
            _: &str,
            _: &str,
        ) -> CoreResult<PortfolioAllocations> {
            unimplemented!()
        }
        async fn get_holdings_by_allocation(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> CoreResult<AllocationHoldings> {
            unimplemented!()
        }
        async fn get_holdings_by_allocation_for_accounts(
            &self,
            _: &[String],
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> CoreResult<AllocationHoldings> {
            unimplemented!()
        }
        async fn get_holding_contributions_for_taxonomy_for_accounts(
            &self,
            _: &[String],
            _: &str,
            _: &str,
            _: &str,
        ) -> CoreResult<TaxonomyHoldingContributions> {
            Ok(self.contributions.clone())
        }
    }

    struct MockHoldingsService {
        holdings: Vec<Holding>,
    }

    #[async_trait]
    impl crate::portfolio::holdings::HoldingsServiceTrait for MockHoldingsService {
        async fn get_holdings(&self, _: &str, _: &str) -> CoreResult<Vec<Holding>> {
            unimplemented!()
        }
        async fn get_holdings_for_accounts(
            &self,
            _: &[String],
            _: &str,
            _: &str,
        ) -> CoreResult<Vec<Holding>> {
            Ok(self.holdings.clone())
        }
        async fn get_holding(&self, _: &str, _: &str, _: &str) -> CoreResult<Option<Holding>> {
            unimplemented!()
        }
        async fn holdings_from_snapshot(
            &self,
            _: &crate::portfolio::snapshot::AccountStateSnapshot,
            _: &str,
        ) -> CoreResult<Vec<Holding>> {
            unimplemented!()
        }
    }

    // ── Service constructors ──────────────────────────────────────────────────

    fn make_service(
        profile: AllocationTarget,
        report: DriftReport,
        contributions: TaxonomyHoldingContributions,
        holdings: Vec<Holding>,
    ) -> RebalanceService {
        RebalanceService::new(
            Arc::new(MockTargetService { profile }),
            Arc::new(MockDriftService { report }),
            Arc::new(MockAllocationService { contributions }),
            Arc::new(MockHoldingsService { holdings }),
        )
    }

    fn make_input(available_cash: Decimal) -> CalculateRebalancePlanInput {
        CalculateRebalancePlanInput {
            target_id: "profile-1".to_string(),
            available_cash,
            account_ids: vec!["acc-1".to_string()],
            base_currency: "USD".to_string(),
            aggregated_account_id: "agg".to_string(),
        }
    }

    // ── Cash enforcement tests (unchanged behaviour from PR-A) ────────────────

    #[tokio::test]
    async fn negative_cash_returns_validation_error() {
        let total = dec!(10000);
        let h = make_holding("h1", "VTI", dec!(10), dec!(6000));
        let c = make_contribution(&h, "equity", dec!(6000));
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(vec![make_drift_row("equity", 6000, 7000, total)], total),
            make_contributions(vec![c]),
            vec![make_cash_holding(dec!(1_000_000), "USD"), h],
        );
        let err = svc
            .calculate_plan(make_input(dec!(-100)))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("non-negative"), "{err}");
    }

    #[tokio::test]
    async fn deploy_exceeding_tracked_cash_is_rejected() {
        let total = dec!(10000);
        let h = make_holding("h1", "VTI", dec!(10), dec!(9500));
        let c = make_contribution(&h, "equity", dec!(9500));
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(vec![make_drift_row("equity", 9500, 7000, total)], total),
            make_contributions(vec![c]),
            vec![make_cash_holding(dec!(500), "USD"), h],
        );
        let err = svc
            .calculate_plan(make_input(dec!(1000)))
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("exceeds tracked cash in scope"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn rounded_cash_within_cent_is_capped_to_tracked_cash() {
        let total = dec!(1000);
        let h = make_holding("h1", "VTI", dec!(10), dec!(500));
        let c = make_contribution(&h, "equity", dec!(500));
        let rows = vec![
            make_drift_row("equity", 5000, 6000, total),
            DriftRow {
                is_cash: true,
                ..make_drift_row("cash", 5000, 4000, total)
            },
        ];
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(rows, total),
            make_contributions(vec![c]),
            vec![make_cash_holding(dec!(100.005), "USD"), h],
        );
        let plan = svc.calculate_plan(make_input(dec!(100.01))).await.unwrap();
        assert_eq!(plan.available_cash, dec!(100.005));
    }

    #[tokio::test]
    async fn rounded_zero_decimal_cash_is_capped_to_tracked_cash() {
        let total = dec!(1000);
        let h = make_holding("h1", "VTI", dec!(10), dec!(500));
        let c = make_contribution(&h, "equity", dec!(500));
        let rows = vec![
            make_drift_row("equity", 5000, 6000, total),
            DriftRow {
                is_cash: true,
                ..make_drift_row("cash", 5000, 4000, total)
            },
        ];
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(rows, total),
            make_contributions(vec![c]),
            vec![make_cash_holding(dec!(100.5), "USD"), h],
        );
        let input = CalculateRebalancePlanInput {
            base_currency: "JPY".to_string(),
            ..make_input(dec!(101))
        };
        let plan = svc.calculate_plan(input).await.unwrap();
        assert_eq!(plan.available_cash, dec!(100.5));
    }

    // ── Greedy planner tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn zero_cash_produces_no_trades() {
        let total = dec!(10000);
        let h = make_holding("h1", "VTI", dec!(10), dec!(6000));
        let c = make_contribution(&h, "equity", dec!(6000));
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(vec![make_drift_row("equity", 6000, 7000, total)], total),
            make_contributions(vec![c]),
            vec![make_cash_holding(dec!(0), "USD"), h],
        );
        let plan = svc.calculate_plan(make_input(dec!(0))).await.unwrap();
        assert!(plan.trades.is_empty());
        assert_eq!(plan.cash_used, Decimal::ZERO);
    }

    #[tokio::test]
    async fn cash_flow_only_no_sells_generated() {
        // Equity 60% (target 70%), Bond 40% (target 30%). Bond overweight.
        let total = dec!(10000);
        let h_vti = make_holding("h1", "VTI", dec!(10), dec!(6000));
        let h_bnd = make_holding("h2", "BND", dec!(40), dec!(4000));
        let c_vti = make_contribution(&h_vti, "equity", dec!(6000));
        let c_bnd = make_contribution(&h_bnd, "bond", dec!(4000));
        let rows = vec![
            make_drift_row("equity", 6000, 7000, total),
            make_drift_row("bond", 4000, 3000, total),
        ];
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(rows, total),
            make_contributions(vec![c_vti, c_bnd]),
            vec![make_cash_holding(dec!(2000), "USD"), h_vti, h_bnd],
        );
        let plan = svc.calculate_plan(make_input(dec!(2000))).await.unwrap();

        assert!(
            plan.trades.iter().all(|t| t.action == "buy"),
            "cash-flow-only must not sell"
        );
        assert!(plan.cash_used <= dec!(2000));
        assert!(
            plan.trades.iter().any(|t| t.category_id == "equity"),
            "should suggest buying equity"
        );
    }

    #[tokio::test]
    async fn greedy_buys_most_drift_reducing_asset() {
        // Equity 50% (target 70%), Bond 50% (target 30%). Equity far underweight.
        // VTI (equity): price $100. BND (bond): price $50.
        // Greedy should buy VTI (improves equity drift) not BND (overweight, buying worsens).
        let total = dec!(10000);
        let h_vti = make_holding("h1", "VTI", dec!(50), dec!(5000));
        let h_bnd = make_holding("h2", "BND", dec!(100), dec!(5000));
        let c_vti = make_contribution(&h_vti, "equity", dec!(5000));
        let c_bnd = make_contribution(&h_bnd, "bond", dec!(5000));
        let rows = vec![
            make_drift_row("equity", 5000, 7000, total),
            make_drift_row("bond", 5000, 3000, total),
        ];
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(rows, total),
            make_contributions(vec![c_vti, c_bnd]),
            vec![make_cash_holding(dec!(5000), "USD"), h_vti, h_bnd],
        );
        let plan = svc.calculate_plan(make_input(dec!(200))).await.unwrap();

        assert!(
            plan.trades
                .iter()
                .all(|t| t.symbol.as_deref() != Some("BND")),
            "BND is overweight — buying it increases bond drift"
        );
        assert!(
            plan.trades
                .iter()
                .any(|t| t.symbol.as_deref() == Some("VTI")),
            "VTI should be selected to reduce equity drift"
        );
    }

    #[tokio::test]
    async fn multi_category_etf_reduces_multiple_sleeve_drifts() {
        // VT is a global ETF: 60% US equity, 40% international equity.
        // Portfolio: US 40% (target 60%), Intl 40% (target 40%), Cash 20%.
        // Buying VT should simultaneously improve US and Intl drift.
        let total = dec!(10000);
        let price = dec!(100); // $100/share
        let qty = dec!(40);
        let h_vt = make_holding("vt", "VT", qty, price * qty);

        // Split contribution: 60% US, 40% Intl of holding value ($4000 total)
        let c_us = make_contribution(&h_vt, "us_equity", dec!(2400)); // 60%
        let c_intl = make_contribution(&h_vt, "intl_equity", dec!(1600)); // 40%

        let rows = vec![
            make_drift_row("us_equity", 4000, 6000, total),
            make_drift_row("intl_equity", 4000, 4000, total),
            DriftRow {
                is_cash: true,
                ..make_drift_row("cash", 2000, 0, total)
            },
        ];
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(rows, total),
            make_contributions(vec![c_us, c_intl]),
            vec![make_cash_holding(dec!(2000), "USD"), h_vt],
        );
        let plan = svc.calculate_plan(make_input(dec!(200))).await.unwrap();

        // VT should be selected (only candidate that reduces US drift)
        let vt_trade = plan
            .trades
            .iter()
            .find(|t| t.symbol.as_deref() == Some("VT"));
        assert!(vt_trade.is_some(), "VT should be selected");
        assert!(plan.cash_used > Decimal::ZERO);
    }

    #[tokio::test]
    async fn underweight_cash_is_not_a_buy_candidate() {
        // Cash underweight but cannot be a buy candidate.
        let total = dec!(10000);
        let h_vti = make_holding("h1", "VTI", dec!(80), dec!(8000));
        // No contribution for cash sleeve — it has no HoldingAllocationContribution.
        let c_vti = make_contribution(&h_vti, "equity", dec!(8000));
        let rows = vec![
            make_drift_row("equity", 8000, 7000, total),
            DriftRow {
                is_cash: true,
                ..make_drift_row("cash", 2000, 3000, total)
            },
        ];
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(rows, total),
            make_contributions(vec![c_vti]),
            vec![make_cash_holding(dec!(2000), "USD"), h_vti],
        );
        let plan = svc.calculate_plan(make_input(dec!(1000))).await.unwrap();

        // Equity is overweight → buying VTI would worsen equity drift → greedy finds no improvement.
        assert!(plan.cash_used == Decimal::ZERO || plan.trades.iter().all(|t| t.action == "buy"));
    }

    #[tokio::test]
    async fn no_holdings_emits_warning_and_sleeve_level_trade() {
        let total = dec!(10000);
        // No holdings, no contributions for bonds sleeve.
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(vec![make_drift_row("bonds", 2000, 4000, total)], total),
            make_contributions(vec![]),
            vec![make_cash_holding(dec!(1000), "USD")],
        );
        let plan = svc.calculate_plan(make_input(dec!(1000))).await.unwrap();

        assert!(
            plan.warnings
                .iter()
                .any(|w| w.kind == RebalanceWarningKind::NoBuyCandidate),
            "NoBuyCandidate warning expected"
        );
        let trade = plan.trades.iter().find(|t| t.category_id == "bonds");
        assert!(trade.is_some(), "sleeve-level trade expected");
        assert!(
            trade.unwrap().symbol.is_none(),
            "sleeve trade has no ticker"
        );
    }

    #[tokio::test]
    async fn equal_price_candidates_break_ties_by_symbol() {
        // Two equity candidates, identical price ($100) and exposure. Cash funds exactly
        // one share. The stable (price, symbol, asset_id) sort must always pick the
        // lexicographically smaller symbol (AAA), regardless of HashMap iteration order.
        let total = dec!(10000);
        let h_z = make_holding("z1", "ZZZ", dec!(1), dec!(100));
        let h_a = make_holding("a1", "AAA", dec!(1), dec!(100));
        let c_z = make_contribution(&h_z, "equity", dec!(100));
        let c_a = make_contribution(&h_a, "equity", dec!(100));
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(vec![make_drift_row("equity", 5000, 7000, total)], total),
            make_contributions(vec![c_z, c_a]),
            vec![make_cash_holding(dec!(100), "USD"), h_z, h_a],
        );
        let plan = svc.calculate_plan(make_input(dec!(100))).await.unwrap();

        let trade = plan
            .trades
            .iter()
            .find(|t| t.asset_id.is_some())
            .expect("one share bought");
        assert_eq!(
            trade.symbol.as_deref(),
            Some("AAA"),
            "equal-price tie must resolve to the smaller symbol"
        );
    }

    #[tokio::test]
    async fn manual_sleeve_trade_is_reflected_in_after_bps() {
        // Bonds underweight (0%, target 10%) with no buy candidate. A $1000 manual
        // sleeve trade fills it; after_bps_by_category must credit the target category
        // (1000 bps) rather than leaving it at 0.
        let total = dec!(10000);
        let rows = vec![
            make_drift_row("bonds", 0, 1000, total),
            make_drift_row("equity", 9000, 9000, total),
            DriftRow {
                is_cash: true,
                ..make_drift_row("cash", 1000, 0, total)
            },
        ];
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(rows, total),
            make_contributions(vec![]),
            vec![make_cash_holding(dec!(1000), "USD")],
        );
        let plan = svc.calculate_plan(make_input(dec!(1000))).await.unwrap();

        assert_eq!(
            plan.cash_used,
            dec!(1000),
            "manual trade should deploy $1000"
        );
        assert_eq!(
            plan.after_bps_by_category.get("bonds").copied(),
            Some(1000),
            "bonds after-drift must reflect the manual sleeve trade"
        );
    }

    #[tokio::test]
    async fn multiple_uncovered_categories_never_overspend_cash() {
        // Two required underweight categories with no buy candidates. Each shortfall
        // ($2000) exceeds available cash ($1000). Manual sleeve trades must share the
        // cash, never letting cash_used exceed available / cash_remaining go negative.
        let total = dec!(10000);
        let rows = vec![
            make_drift_row("bonds", 2000, 4000, total),
            make_drift_row("reit", 2000, 4000, total),
            make_drift_row("equity", 6000, 2000, total),
        ];
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(rows, total),
            make_contributions(vec![]),
            vec![make_cash_holding(dec!(1000), "USD")],
        );
        let plan = svc.calculate_plan(make_input(dec!(1000))).await.unwrap();

        assert!(
            plan.cash_used <= dec!(1000),
            "cash_used must not exceed available cash, got {}",
            plan.cash_used
        );
        assert!(
            plan.cash_remaining >= Decimal::ZERO,
            "cash_remaining must not go negative, got {}",
            plan.cash_remaining
        );
        assert_eq!(plan.cash_used + plan.cash_remaining, dec!(1000));
    }

    #[tokio::test]
    async fn unclassified_asset_emits_warning_and_is_skipped() {
        let total = dec!(10000);
        let h = make_holding("h1", "XYZ", dec!(10), dec!(5000));
        // All contribution in __UNKNOWN__ = no taxonomy assignments.
        let c_unknown = make_contribution(&h, "__UNKNOWN__", dec!(5000));
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(vec![make_drift_row("equity", 5000, 7000, total)], total),
            make_contributions(vec![c_unknown]),
            vec![make_cash_holding(dec!(1000), "USD"), h],
        );
        let plan = svc.calculate_plan(make_input(dec!(500))).await.unwrap();

        assert!(
            plan.warnings
                .iter()
                .any(|w| w.kind == RebalanceWarningKind::UnclassifiedAsset),
            "UnclassifiedAsset warning expected"
        );
        assert!(
            !plan
                .trades
                .iter()
                .any(|t| t.symbol.as_deref() == Some("XYZ")),
            "XYZ must not be a buy candidate"
        );
    }

    #[tokio::test]
    async fn partial_classification_warns_and_uses_known_exposure() {
        // ABC: 70% equity, 30% __UNKNOWN__. Should warn PartialClassification
        // and still be a candidate using only the 70% equity exposure.
        let total = dec!(10000);
        let qty = dec!(10);
        let mv = dec!(7000);
        let h = make_holding("h1", "ABC", qty, mv);
        let c_equity = make_contribution(&h, "equity", dec!(4900)); // 70%
        let c_unknown = make_contribution(&h, "__UNKNOWN__", dec!(2100)); // 30%

        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(vec![make_drift_row("equity", 3000, 7000, total)], total),
            make_contributions(vec![c_equity, c_unknown]),
            vec![make_cash_holding(dec!(5000), "USD"), h],
        );
        let plan = svc.calculate_plan(make_input(dec!(700))).await.unwrap();

        assert!(
            plan.warnings
                .iter()
                .any(|w| w.kind == RebalanceWarningKind::PartialClassification),
            "PartialClassification warning expected"
        );
        assert!(
            plan.trades
                .iter()
                .any(|t| t.symbol.as_deref() == Some("ABC")),
            "ABC should still be a buy candidate"
        );
    }

    #[tokio::test]
    async fn whole_shares_only_buys_integer_shares() {
        let total = dec!(10000);
        // VTI: 10 shares @ $600 = $6000. Price = $600/share.
        let h = make_holding("h1", "VTI", dec!(10), dec!(6000));
        let c = make_contribution(&h, "equity", dec!(6000));
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, true),
            make_report(vec![make_drift_row("equity", 6000, 7000, total)], total),
            make_contributions(vec![c]),
            vec![make_cash_holding(dec!(5000), "USD"), h],
        );
        let plan = svc.calculate_plan(make_input(dec!(1500))).await.unwrap();

        let vti = plan
            .trades
            .iter()
            .find(|t| t.symbol.as_deref() == Some("VTI"))
            .expect("VTI trade");
        let qty = vti.quantity.unwrap();
        assert_eq!(
            qty.fract(),
            Decimal::ZERO,
            "whole shares only: must be integer, got {qty}"
        );
    }

    #[tokio::test]
    async fn nearest_band_deploys_less_than_exact_target() {
        let total = dec!(10000);
        let h = make_holding("h1", "VTI", dec!(10), dec!(6000));
        let c1 = make_contribution(&h, "equity", dec!(6000));
        let c2 = make_contribution(&h, "equity", dec!(6000));
        let rows = vec![make_drift_row("equity", 6000, 7000, total)];

        let svc_exact = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(rows.clone(), total),
            make_contributions(vec![c1]),
            vec![make_cash_holding(dec!(5000), "USD"), h.clone()],
        );
        let svc_band = make_service(
            make_profile(RebalanceGoal::NearestBand, false),
            make_report(rows, total),
            make_contributions(vec![c2]),
            vec![make_cash_holding(dec!(5000), "USD"), h],
        );

        let plan_exact = svc_exact
            .calculate_plan(make_input(dec!(5000)))
            .await
            .unwrap();
        let plan_band = svc_band
            .calculate_plan(make_input(dec!(5000)))
            .await
            .unwrap();

        assert!(
            plan_band.cash_used <= plan_exact.cash_used,
            "nearest_band deploys less cash than exact_target: {} vs {}",
            plan_band.cash_used,
            plan_exact.cash_used
        );
    }

    #[tokio::test]
    async fn nearest_band_stops_at_band_edge() {
        // Equity 60% (target 70%, band 5%). NearestBand should stop once equity reaches
        // the 65% band edge ($500 deployed), not optimize to the exact 70% target ($1000).
        let total = dec!(10000);
        let h = make_holding("h1", "VTI", dec!(60), dec!(6000)); // $100/share
        let rows = vec![make_drift_row("equity", 6000, 7000, total)];

        let svc_band = make_service(
            make_profile(RebalanceGoal::NearestBand, false),
            make_report(rows.clone(), total),
            make_contributions(vec![make_contribution(&h, "equity", dec!(6000))]),
            vec![make_cash_holding(dec!(1000), "USD"), h.clone()],
        );
        let svc_exact = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(rows, total),
            make_contributions(vec![make_contribution(&h, "equity", dec!(6000))]),
            vec![make_cash_holding(dec!(1000), "USD"), h],
        );

        let plan_band = svc_band
            .calculate_plan(make_input(dec!(1000)))
            .await
            .unwrap();
        let plan_exact = svc_exact
            .calculate_plan(make_input(dec!(1000)))
            .await
            .unwrap();

        assert_eq!(
            plan_band.cash_used,
            dec!(500),
            "nearest_band stops at the band edge ($500), got {}",
            plan_band.cash_used
        );
        assert_eq!(
            plan_exact.cash_used,
            dec!(1000),
            "exact_target deploys to the exact target ($1000), got {}",
            plan_exact.cash_used
        );
    }

    #[tokio::test]
    async fn min_trade_amount_filters_small_trades() {
        // VTI at $80/share. min_trade $100.
        // Cash $80 → greedy buys 1 share ($80). Post-filter: $80 < $100 → dropped.
        // Result: no trades. cash_used = $0, cash_remaining = $80.
        let total = dec!(10000);
        let h = make_holding("h1", "VTI", dec!(75), dec!(6000)); // $80/share
        let c = make_contribution(&h, "equity", dec!(6000));
        let profile = AllocationTarget {
            min_trade_amount: "100".to_string(),
            ..make_profile(RebalanceGoal::ExactTarget, false)
        };
        let svc = make_service(
            profile,
            make_report(vec![make_drift_row("equity", 6000, 7000, total)], total),
            make_contributions(vec![c]),
            vec![make_cash_holding(dec!(80), "USD"), h],
        );
        let plan = svc.calculate_plan(make_input(dec!(80))).await.unwrap();
        assert!(
            plan.trades.iter().all(|t| t.estimated_amount >= dec!(100)),
            "all trades must meet min_trade threshold"
        );

        // With $160 cash: 2 shares × $80 = $160 ≥ $100 → kept.
        let h2 = make_holding("h2", "VTI", dec!(75), dec!(6000));
        let c2 = make_contribution(&h2, "equity", dec!(6000));
        let profile2 = AllocationTarget {
            min_trade_amount: "100".to_string(),
            ..make_profile(RebalanceGoal::ExactTarget, false)
        };
        let svc2 = make_service(
            profile2,
            make_report(vec![make_drift_row("equity", 6000, 7000, total)], total),
            make_contributions(vec![c2]),
            vec![make_cash_holding(dec!(160), "USD"), h2],
        );
        let plan2 = svc2.calculate_plan(make_input(dec!(160))).await.unwrap();
        assert!(
            plan2
                .trades
                .iter()
                .any(|t| t.symbol.as_deref() == Some("VTI")),
            "VTI should survive min_trade when total >= threshold"
        );
    }

    #[tokio::test]
    async fn total_value_stays_constant_when_cash_deployed() {
        // Portfolio: equity $6000 (60%), cash $4000 (40%). Total = $10000.
        // Target: equity 70%, cash 30%.
        // Greedy buys equity, max_drift_after should improve.
        let total = dec!(10000);
        let h = make_holding("h1", "VTI", dec!(10), dec!(6000));
        let c = make_contribution(&h, "equity", dec!(6000));
        let rows = vec![
            make_drift_row("equity", 6000, 7000, total),
            DriftRow {
                is_cash: true,
                ..make_drift_row("cash", 4000, 3000, total)
            },
        ];
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            make_report(rows, total),
            make_contributions(vec![c]),
            vec![make_cash_holding(dec!(4000), "USD"), h],
        );
        let plan = svc.calculate_plan(make_input(dec!(2000))).await.unwrap();

        assert_eq!(
            plan.cash_used + plan.cash_remaining,
            dec!(2000),
            "cash_used + remaining must equal available"
        );
        assert!(
            plan.max_drift_bps_after <= plan.max_drift_bps_before,
            "drift must not increase after buying"
        );
    }
}
