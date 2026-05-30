use async_trait::async_trait;
use log::debug;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::{Error as CoreError, Result as CoreResult};
use crate::portfolio::allocation::AllocationServiceTrait;

use super::drift_service::DriftServiceTrait;
use super::model::{
    CalculateRebalancePlanInput, RebalanceDraft, RebalancePlan, RebalanceTo, RebalanceWarning,
    RebalanceWarningKind, SuggestedManualTrade, TargetProfile,
};
use super::target_service::TargetProfileServiceTrait;

// ── Repository trait ──────────────────────────────────────────────────────────

#[async_trait]
pub trait RebalanceDraftRepositoryTrait: Send + Sync {
    async fn save_draft(&self, draft: RebalanceDraft) -> CoreResult<RebalanceDraft>;
    fn list_drafts(&self, profile_id: &str) -> CoreResult<Vec<RebalanceDraft>>;
    async fn delete_draft(&self, id: &str) -> CoreResult<usize>;
}

// ── Service trait ─────────────────────────────────────────────────────────────

#[async_trait]
pub trait RebalanceServiceTrait: Send + Sync {
    async fn calculate_plan(&self, input: CalculateRebalancePlanInput)
        -> CoreResult<RebalancePlan>;

    async fn save_draft(
        &self,
        profile: &TargetProfile,
        input: &CalculateRebalancePlanInput,
        plan: &RebalancePlan,
    ) -> CoreResult<RebalanceDraft>;

    fn list_drafts(&self, profile_id: &str) -> CoreResult<Vec<RebalanceDraft>>;

    async fn delete_draft(&self, id: &str) -> CoreResult<()>;
}

// ── Implementation ────────────────────────────────────────────────────────────

pub struct RebalanceService {
    target_service: Arc<dyn TargetProfileServiceTrait>,
    drift_service: Arc<dyn DriftServiceTrait>,
    allocation_service: Arc<dyn AllocationServiceTrait>,
    draft_repo: Arc<dyn RebalanceDraftRepositoryTrait>,
}

impl RebalanceService {
    pub fn new(
        target_service: Arc<dyn TargetProfileServiceTrait>,
        drift_service: Arc<dyn DriftServiceTrait>,
        allocation_service: Arc<dyn AllocationServiceTrait>,
        draft_repo: Arc<dyn RebalanceDraftRepositoryTrait>,
    ) -> Self {
        Self {
            target_service,
            drift_service,
            allocation_service,
            draft_repo,
        }
    }

    fn now() -> String {
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
    }
}

#[async_trait]
impl RebalanceServiceTrait for RebalanceService {
    async fn calculate_plan(
        &self,
        input: CalculateRebalancePlanInput,
    ) -> CoreResult<RebalancePlan> {
        debug!(
            "Calculating rebalance plan for profile {} with {} available cash",
            input.profile_id, input.available_cash
        );

        // --- 1. Load profile -------------------------------------------------
        let profile = self
            .target_service
            .get_profile(&input.profile_id)?
            .ok_or_else(|| {
                CoreError::Database(crate::errors::DatabaseError::NotFound(format!(
                    "TargetProfile {} not found",
                    input.profile_id
                )))
            })?;

        // --- 2. Drift report (reuse M1 DriftService) -------------------------
        let drift = self
            .drift_service
            .get_drift_report_for_profile(
                &input.profile_id,
                &input.account_ids,
                &input.base_currency,
                &input.aggregated_account_id,
            )
            .await?;

        let total_value = drift.total_value;
        let max_drift_bps_before = drift.max_drift_bps;

        // Nothing to plan against.
        if total_value == Decimal::ZERO && input.available_cash == Decimal::ZERO {
            return Ok(RebalancePlan {
                profile_id: input.profile_id.clone(),
                available_cash: Decimal::ZERO,
                cash_used: Decimal::ZERO,
                cash_remaining: Decimal::ZERO,
                max_drift_bps_before: 0,
                max_drift_bps_after: 0,
                trades: vec![],
                warnings: vec![],
            });
        }

        // New total value after deploying all available cash (used for target calculations).
        let new_total_value = total_value + input.available_cash;
        let bps_scale = dec!(10000);

        // --- 3. Compute shortfalls per underweight sleeve --------------------
        // shortfall = dollars needed to reach desired level relative to new_total_value.
        // nearest_band: bring to just inside band edge (target_bps - drift_band_bps).
        // exact_target: bring to target_bps.
        let drift_band = Decimal::from(profile.drift_band_bps);

        struct SleeveShortfall {
            category_id: String,
            category_name: String,
            shortfall: Decimal,
        }

        let mut sleeves: Vec<SleeveShortfall> = Vec::new();
        for row in &drift.rows {
            let target_bps_dec = Decimal::from(row.target_bps);
            let desired_bps = match profile.rebalance_to {
                RebalanceTo::ExactTarget => target_bps_dec,
                RebalanceTo::NearestBand => (target_bps_dec - drift_band).max(Decimal::ZERO),
            };
            let desired_value = desired_bps / bps_scale * new_total_value;
            let shortfall = (desired_value - row.current_value).max(Decimal::ZERO);
            if shortfall > Decimal::ZERO {
                sleeves.push(SleeveShortfall {
                    category_id: row.category_id.clone(),
                    category_name: row.category_name.clone(),
                    shortfall,
                });
            }
        }

        // --- 4. Scale shortfalls to available cash ---------------------------
        let total_shortfall: Decimal = sleeves.iter().map(|s| s.shortfall).sum();
        let scale_factor =
            if total_shortfall == Decimal::ZERO || input.available_cash == Decimal::ZERO {
                Decimal::ZERO
            } else if total_shortfall <= input.available_cash {
                Decimal::ONE
            } else {
                input.available_cash / total_shortfall
            };

        // --- 5. Build trades per sleeve, proportional to holdings weights ----
        let mut trades: Vec<SuggestedManualTrade> = Vec::new();
        let mut warnings: Vec<RebalanceWarning> = Vec::new();
        let mut total_cash_used = Decimal::ZERO;

        // Track value deployed per sleeve for max_drift_bps_after estimation.
        let mut deployed_per_sleeve: std::collections::HashMap<String, Decimal> =
            std::collections::HashMap::new();

        for sleeve in &sleeves {
            // Cap budget at remaining cash to guard against Decimal precision drift.
            let remaining = input.available_cash - total_cash_used;
            if remaining <= Decimal::ZERO {
                break;
            }
            let budget = (sleeve.shortfall * scale_factor).min(remaining);
            if budget == Decimal::ZERO {
                continue;
            }

            // Load holdings for this sleeve.
            let allocation_holdings = self
                .allocation_service
                .get_holdings_by_allocation_for_accounts(
                    &input.account_ids,
                    &input.base_currency,
                    &profile.taxonomy_id,
                    &sleeve.category_id,
                    &input.aggregated_account_id,
                )
                .await?;

            let holdings = &allocation_holdings.holdings;

            // No holdings → sleeve-level suggestion.
            if holdings.is_empty() {
                warnings.push(RebalanceWarning {
                    kind: RebalanceWarningKind::NoBuyCandidate,
                    category_id: sleeve.category_id.clone(),
                    message: format!(
                        "No holdings found in {}. Buy ${:.2} of this sleeve manually.",
                        sleeve.category_name, budget
                    ),
                });
                trades.push(SuggestedManualTrade {
                    action: "buy".to_string(),
                    category_id: sleeve.category_id.clone(),
                    category_name: sleeve.category_name.clone(),
                    asset_id: None,
                    symbol: None,
                    name: None,
                    quantity: None,
                    estimated_price: None,
                    estimated_amount: budget,
                    reason: format!(
                        "Sleeve {} is underweight. Suggested amount to buy.",
                        sleeve.category_name
                    ),
                });
                *deployed_per_sleeve
                    .entry(sleeve.category_id.clone())
                    .or_default() += budget;
                total_cash_used += budget;
                continue;
            }

            // Compute total sleeve value for proportional weights.
            let sleeve_total_value: Decimal = holdings.iter().map(|h| h.market_value).sum();

            let mut sleeve_deployed = Decimal::ZERO;

            for holding in holdings {
                // Proportional budget for this holding.
                let weight = if sleeve_total_value > Decimal::ZERO {
                    holding.market_value / sleeve_total_value
                } else {
                    Decimal::ONE / Decimal::from(holdings.len() as i64)
                };
                let holding_budget = budget * weight;

                // Derive price per share.
                if holding.quantity <= Decimal::ZERO || holding.market_value <= Decimal::ZERO {
                    if profile.whole_shares_only {
                        warnings.push(RebalanceWarning {
                            kind: RebalanceWarningKind::MissingQuote,
                            category_id: sleeve.category_id.clone(),
                            message: format!(
                                "{}: no valid price/quantity for whole-share rounding. Skipped.",
                                holding.symbol
                            ),
                        });
                        continue;
                    }
                    // Fractional: emit as dollar amount without shares.
                    if holding_budget < profile.min_trade_amount {
                        continue;
                    }
                    trades.push(SuggestedManualTrade {
                        action: "buy".to_string(),
                        category_id: sleeve.category_id.clone(),
                        category_name: sleeve.category_name.clone(),
                        asset_id: Some(holding.id.clone()),
                        symbol: Some(holding.symbol.clone()),
                        name: holding.name.clone(),
                        quantity: None,
                        estimated_price: None,
                        estimated_amount: holding_budget,
                        reason: format!(
                            "{} is underweight in {}.",
                            holding.symbol, sleeve.category_name
                        ),
                    });
                    sleeve_deployed += holding_budget;
                    continue;
                }

                let price = holding.market_value / holding.quantity;

                let (shares, amount) = if profile.whole_shares_only {
                    let whole = (holding_budget / price).floor();
                    if whole == Decimal::ZERO {
                        warnings.push(RebalanceWarning {
                            kind: RebalanceWarningKind::WholeShareResidue,
                            category_id: sleeve.category_id.clone(),
                            message: format!(
                                "{}: ${:.2} budget insufficient for 1 whole share at ${:.2}. Skipped.",
                                holding.symbol, holding_budget, price
                            ),
                        });
                        continue;
                    }
                    let amt = whole * price;
                    (Some(whole), amt)
                } else {
                    // Fractional: full budget, compute shares for display.
                    (Some(holding_budget / price), holding_budget)
                };

                if amount < profile.min_trade_amount {
                    continue;
                }

                trades.push(SuggestedManualTrade {
                    action: "buy".to_string(),
                    category_id: sleeve.category_id.clone(),
                    category_name: sleeve.category_name.clone(),
                    asset_id: Some(holding.id.clone()),
                    symbol: Some(holding.symbol.clone()),
                    name: holding.name.clone(),
                    quantity: shares,
                    estimated_price: Some(price),
                    estimated_amount: amount,
                    reason: format!(
                        "{} is underweight in {}.",
                        holding.symbol, sleeve.category_name
                    ),
                });
                sleeve_deployed += amount;
            }

            // --- Intra-sleeve budget optimisation (whole_shares_only) -----------
            // After proportional round-lot allocation, rounding residue remains
            // undeployed. Re-apply the same proportional weights to the residue
            // iteratively until no further whole shares can be purchased. Using
            // proportional weights (instead of price-ascending order) preserves
            // sleeve composition — a cheap holding cannot absorb all the residue.
            if profile.whole_shares_only {
                let mut residue = budget - sleeve_deployed;

                'deploy_residue: loop {
                    let mut absorbed = false;
                    for holding in holdings.iter() {
                        if residue <= Decimal::ZERO {
                            break 'deploy_residue;
                        }
                        if holding.quantity <= Decimal::ZERO
                            || holding.market_value <= Decimal::ZERO
                        {
                            continue;
                        }
                        let price = holding.market_value / holding.quantity;
                        if price <= Decimal::ZERO {
                            continue;
                        }
                        let weight = if sleeve_total_value > Decimal::ZERO {
                            holding.market_value / sleeve_total_value
                        } else {
                            Decimal::ONE / Decimal::from(holdings.len() as i64)
                        };
                        let additional = (residue * weight / price).floor();
                        if additional < Decimal::ONE {
                            continue;
                        }
                        let amt = additional * price;

                        let existing = trades.iter_mut().rev().find(|t| {
                            t.asset_id.as_deref() == Some(holding.id.as_str())
                                && t.category_id == sleeve.category_id
                        });
                        if let Some(t) = existing {
                            t.quantity = t.quantity.map(|q| q + additional);
                            t.estimated_amount += amt;
                        } else if amt >= profile.min_trade_amount {
                            trades.push(SuggestedManualTrade {
                                action: "buy".to_string(),
                                category_id: sleeve.category_id.clone(),
                                category_name: sleeve.category_name.clone(),
                                asset_id: Some(holding.id.clone()),
                                symbol: Some(holding.symbol.clone()),
                                name: holding.name.clone(),
                                quantity: Some(additional),
                                estimated_price: Some(price),
                                estimated_amount: amt,
                                reason: format!(
                                    "{} is underweight in {}.",
                                    holding.symbol, sleeve.category_name
                                ),
                            });
                        }
                        sleeve_deployed += amt;
                        residue -= amt;
                        absorbed = true;
                    }
                    if !absorbed {
                        break;
                    }
                }
            }

            *deployed_per_sleeve
                .entry(sleeve.category_id.clone())
                .or_default() += sleeve_deployed;
            total_cash_used += sleeve_deployed;
        }

        // --- 6. Estimate max_drift_bps_after ---------------------------------
        let total_value_after = total_value + total_cash_used;

        let max_drift_bps_after = if total_value_after == Decimal::ZERO {
            0
        } else {
            drift
                .rows
                .iter()
                .map(|row| {
                    let deployed = deployed_per_sleeve
                        .get(&row.category_id)
                        .copied()
                        .unwrap_or(Decimal::ZERO);
                    let new_value = row.current_value + deployed;
                    let new_bps = (new_value / total_value_after * bps_scale)
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0);
                    (new_bps - row.target_bps).abs()
                })
                .max()
                .unwrap_or(0)
        };

        let cash_remaining = input.available_cash - total_cash_used;

        debug!(
            "Plan: {} trades, ${} used, ${} remaining, max drift {} → {} bps",
            trades.len(),
            total_cash_used,
            cash_remaining,
            max_drift_bps_before,
            max_drift_bps_after,
        );

        Ok(RebalancePlan {
            profile_id: input.profile_id.clone(),
            available_cash: input.available_cash,
            cash_used: total_cash_used,
            cash_remaining,
            max_drift_bps_before,
            max_drift_bps_after,
            trades,
            warnings,
        })
    }

    async fn save_draft(
        &self,
        profile: &TargetProfile,
        input: &CalculateRebalancePlanInput,
        plan: &RebalancePlan,
    ) -> CoreResult<RebalanceDraft> {
        let db_err = |msg: String| CoreError::Database(crate::errors::DatabaseError::Internal(msg));
        let now = Self::now();
        let draft = RebalanceDraft {
            id: Uuid::new_v4().to_string(),
            profile_id: profile.id.clone(),
            profile_snapshot_json: serde_json::to_string(profile)
                .map_err(|e| db_err(format!("failed to serialize profile snapshot: {e}")))?,
            input_json: serde_json::to_string(input)
                .map_err(|e| db_err(format!("failed to serialize plan input: {e}")))?,
            result_json: serde_json::to_string(plan)
                .map_err(|e| db_err(format!("failed to serialize plan result: {e}")))?,
            created_at: now.clone(),
            updated_at: now,
        };
        self.draft_repo.save_draft(draft).await
    }

    fn list_drafts(&self, profile_id: &str) -> CoreResult<Vec<RebalanceDraft>> {
        self.draft_repo.list_drafts(profile_id)
    }

    async fn delete_draft(&self, id: &str) -> CoreResult<()> {
        self.draft_repo.delete_draft(id).await?;
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portfolio::allocation::{AllocationHoldings, PortfolioAllocations};
    use crate::portfolio::allocation_targets::{
        DriftReport, DriftRow, DriftStatus, NewTargetAllocationNode, NewTargetProfile,
        ProfileStatus, RebalanceTo, ScopeType, TargetAllocationNode, TargetProfile, TriggerType,
    };
    use crate::portfolio::holdings::{HoldingSummary, HoldingType};
    use rust_decimal_macros::dec;

    fn make_profile(rebalance_to: RebalanceTo, whole_shares_only: bool) -> TargetProfile {
        TargetProfile {
            id: "profile-1".to_string(),
            name: "Test".to_string(),
            status: ProfileStatus::Active,
            scope_type: ScopeType::All,
            scope_id: None,
            taxonomy_id: "asset_classes".to_string(),
            trigger_type: TriggerType::Threshold,
            drift_band_bps: 500,
            rebalance_to,
            min_trade_amount: Decimal::ZERO,
            whole_shares_only,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
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
        }
    }

    fn make_holding(
        id: &str,
        symbol: &str,
        quantity: Decimal,
        market_value: Decimal,
    ) -> HoldingSummary {
        HoldingSummary {
            id: id.to_string(),
            symbol: symbol.to_string(),
            name: Some(symbol.to_string()),
            holding_type: HoldingType::Security,
            quantity,
            market_value,
            currency: "USD".to_string(),
            weight_in_category: Decimal::ZERO,
        }
    }

    // ── Mocks ─────────────────────────────────────────────────────────────────

    struct MockTargetService {
        profile: TargetProfile,
    }

    #[async_trait]
    impl TargetProfileServiceTrait for MockTargetService {
        fn get_profile(&self, _: &str) -> CoreResult<Option<TargetProfile>> {
            Ok(Some(self.profile.clone()))
        }
        fn list_profiles(&self) -> CoreResult<Vec<TargetProfile>> {
            Ok(vec![])
        }
        fn get_active_profile_for_scope(
            &self,
            _: &str,
            _: Option<&str>,
        ) -> CoreResult<Option<TargetProfile>> {
            Ok(None)
        }
        fn list_nodes_for_profile(&self, _: &str) -> CoreResult<Vec<TargetAllocationNode>> {
            Ok(vec![])
        }
        async fn create_profile(&self, _: NewTargetProfile) -> CoreResult<TargetProfile> {
            unimplemented!()
        }
        async fn update_profile(&self, _: &str, _: NewTargetProfile) -> CoreResult<TargetProfile> {
            unimplemented!()
        }
        async fn activate_profile(&self, _: &str) -> CoreResult<TargetProfile> {
            unimplemented!()
        }
        async fn archive_profile(&self, _: &str) -> CoreResult<TargetProfile> {
            unimplemented!()
        }
        async fn delete_profile(&self, _: &str) -> CoreResult<()> {
            unimplemented!()
        }
        async fn save_nodes(
            &self,
            _: &str,
            _: Vec<NewTargetAllocationNode>,
        ) -> CoreResult<Vec<TargetAllocationNode>> {
            unimplemented!()
        }
    }

    struct MockDriftService {
        report: DriftReport,
    }

    #[async_trait]
    impl DriftServiceTrait for MockDriftService {
        async fn get_drift_report(
            &self,
            _: &str,
            _: Option<&str>,
            _: &[String],
            _: &str,
            _: &str,
        ) -> CoreResult<Option<DriftReport>> {
            Ok(Some(self.report.clone()))
        }
        async fn get_drift_report_for_profile(
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
        holdings_by_category: std::collections::HashMap<String, Vec<HoldingSummary>>,
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
            category_id: &str,
            _: &str,
        ) -> CoreResult<AllocationHoldings> {
            let holdings = self
                .holdings_by_category
                .get(category_id)
                .cloned()
                .unwrap_or_default();
            let total = holdings.iter().map(|h| h.market_value).sum();
            Ok(AllocationHoldings {
                taxonomy_id: "asset_classes".to_string(),
                taxonomy_name: "Asset Classes".to_string(),
                category_id: category_id.to_string(),
                category_name: category_id.to_string(),
                color: "#aaa".to_string(),
                holdings,
                total_value: total,
                currency: "USD".to_string(),
            })
        }
    }

    struct MockDraftRepo;

    #[async_trait]
    impl RebalanceDraftRepositoryTrait for MockDraftRepo {
        async fn save_draft(&self, d: RebalanceDraft) -> CoreResult<RebalanceDraft> {
            Ok(d)
        }
        fn list_drafts(&self, _: &str) -> CoreResult<Vec<RebalanceDraft>> {
            Ok(vec![])
        }
        async fn delete_draft(&self, _: &str) -> CoreResult<usize> {
            Ok(1)
        }
    }

    fn make_service(
        profile: TargetProfile,
        report: DriftReport,
        holdings: std::collections::HashMap<String, Vec<HoldingSummary>>,
    ) -> RebalanceService {
        RebalanceService::new(
            Arc::new(MockTargetService { profile }),
            Arc::new(MockDriftService { report }),
            Arc::new(MockAllocationService {
                holdings_by_category: holdings,
            }),
            Arc::new(MockDraftRepo),
        )
    }

    fn make_input(available_cash: Decimal) -> CalculateRebalancePlanInput {
        CalculateRebalancePlanInput {
            profile_id: "profile-1".to_string(),
            available_cash,
            account_ids: vec!["acc-1".to_string()],
            base_currency: "USD".to_string(),
            aggregated_account_id: "agg".to_string(),
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
            profile_id: "profile-1".to_string(),
            scope_type: ScopeType::All,
            scope_id: None,
            total_value,
            base_currency: "USD".to_string(),
            max_drift_bps: max as i32,
            out_of_band_count: out,
            rows,
        }
    }

    #[tokio::test]
    async fn cash_flow_only_no_sells_generated() {
        // Equity 60% (target 70%), Bond 40% (target 30%).
        // Bond is overweight, Equity is underweight.
        let total = dec!(10000);
        let rows = vec![
            make_drift_row("equity", 6000, 7000, total),
            make_drift_row("bond", 4000, 3000, total),
        ];
        let report = make_report(rows, total);

        let mut holdings = std::collections::HashMap::new();
        holdings.insert(
            "equity".to_string(),
            vec![make_holding("h1", "VTI", dec!(10), dec!(6000))],
        );

        let svc = make_service(
            make_profile(RebalanceTo::ExactTarget, false),
            report,
            holdings,
        );
        let plan = svc.calculate_plan(make_input(dec!(2000))).await.unwrap();

        assert!(
            plan.trades.iter().all(|t| t.action == "buy"),
            "cash-flow-only must not sell"
        );
        assert!(plan.cash_used <= dec!(2000));
        let equity_trade = plan.trades.iter().find(|t| t.category_id == "equity");
        assert!(equity_trade.is_some(), "should suggest buying equity");
    }

    #[tokio::test]
    async fn zero_cash_produces_no_trades() {
        let total = dec!(10000);
        let rows = vec![make_drift_row("equity", 6000, 7000, total)];
        let report = make_report(rows, total);
        let mut holdings = std::collections::HashMap::new();
        holdings.insert(
            "equity".to_string(),
            vec![make_holding("h1", "VTI", dec!(10), dec!(6000))],
        );

        let svc = make_service(
            make_profile(RebalanceTo::ExactTarget, false),
            report,
            holdings,
        );
        let plan = svc.calculate_plan(make_input(dec!(0))).await.unwrap();

        assert!(plan.trades.is_empty());
        assert_eq!(plan.cash_used, Decimal::ZERO);
    }

    #[tokio::test]
    async fn insufficient_cash_scales_down() {
        // Need $1000 for equity, but only $500 available.
        let total = dec!(10000);
        let rows = vec![
            make_drift_row("equity", 6000, 7000, total),
            make_drift_row("bond", 4000, 3000, total),
        ];
        let report = make_report(rows, total);
        let mut holdings = std::collections::HashMap::new();
        holdings.insert(
            "equity".to_string(),
            vec![make_holding("h1", "VTI", dec!(10), dec!(6000))],
        );

        let svc = make_service(
            make_profile(RebalanceTo::ExactTarget, false),
            report,
            holdings,
        );
        let plan = svc.calculate_plan(make_input(dec!(500))).await.unwrap();

        assert!(plan.cash_used <= dec!(500));
    }

    #[tokio::test]
    async fn no_holdings_emits_warning_and_sleeve_level_trade() {
        let total = dec!(10000);
        let rows = vec![make_drift_row("bonds", 2000, 4000, total)];
        let report = make_report(rows, total);
        let holdings = std::collections::HashMap::new(); // no holdings in bonds

        let svc = make_service(
            make_profile(RebalanceTo::ExactTarget, false),
            report,
            holdings,
        );
        let plan = svc.calculate_plan(make_input(dec!(1000))).await.unwrap();

        assert!(plan
            .warnings
            .iter()
            .any(|w| w.kind == RebalanceWarningKind::NoBuyCandidate));
        let trade = plan.trades.iter().find(|t| t.category_id == "bonds");
        assert!(trade.is_some());
        assert!(
            trade.unwrap().symbol.is_none(),
            "sleeve-level trade has no ticker"
        );
    }

    #[tokio::test]
    async fn whole_shares_only_rounds_down() {
        let total = dec!(10000);
        let rows = vec![make_drift_row("equity", 6000, 7000, total)];
        let report = make_report(rows, total);
        let mut holdings = std::collections::HashMap::new();
        // Price = 1000/10 = $100. Budget = $1000 → 10 shares exactly.
        holdings.insert(
            "equity".to_string(),
            vec![make_holding("h1", "VTI", dec!(10), dec!(1000))],
        );

        let svc = make_service(
            make_profile(RebalanceTo::ExactTarget, true),
            report,
            holdings,
        );
        let plan = svc.calculate_plan(make_input(dec!(1000))).await.unwrap();

        let trade = plan
            .trades
            .iter()
            .find(|t| t.symbol.as_deref() == Some("VTI"))
            .unwrap();
        let qty = trade.quantity.unwrap();
        assert_eq!(
            qty.fract(),
            Decimal::ZERO,
            "whole shares only: must be integer"
        );
    }

    #[tokio::test]
    async fn nearest_band_shortfall_less_than_exact_target() {
        let total = dec!(10000);
        let target_bps = 7000;
        let current_bps = 6000; // drift = -1000 bps, band = 500, out of band
        let rows = vec![make_drift_row("equity", current_bps, target_bps, total)];

        let report_exact = make_report(rows.clone(), total);
        let report_band = make_report(rows, total);

        let mut h = std::collections::HashMap::new();
        h.insert(
            "equity".to_string(),
            vec![make_holding("h1", "VTI", dec!(10), dec!(6000))],
        );

        let svc_exact = make_service(
            make_profile(RebalanceTo::ExactTarget, false),
            report_exact,
            h.clone(),
        );
        let svc_band = make_service(
            make_profile(RebalanceTo::NearestBand, false),
            report_band,
            h,
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
            "nearest_band deploys less cash than exact_target"
        );
    }

    #[tokio::test]
    async fn proportional_buy_across_multiple_holdings() {
        let total = dec!(10000);
        let rows = vec![make_drift_row("equity", 5000, 8000, total)];
        let report = make_report(rows, total);

        let mut h = std::collections::HashMap::new();
        // Two holdings with 3:1 market value ratio.
        h.insert(
            "equity".to_string(),
            vec![
                make_holding("h1", "VTI", dec!(30), dec!(3000)),
                make_holding("h2", "VXUS", dec!(10), dec!(1000)),
            ],
        );

        let svc = make_service(make_profile(RebalanceTo::ExactTarget, false), report, h);
        let plan = svc.calculate_plan(make_input(dec!(2000))).await.unwrap();

        let vti = plan
            .trades
            .iter()
            .find(|t| t.symbol.as_deref() == Some("VTI"))
            .unwrap();
        let vxus = plan
            .trades
            .iter()
            .find(|t| t.symbol.as_deref() == Some("VXUS"))
            .unwrap();

        // VTI gets 75%, VXUS gets 25% of the equity budget.
        let ratio = vti.estimated_amount / vxus.estimated_amount;
        let expected = dec!(3); // 3000/1000
        assert!(
            (ratio - expected).abs() < dec!(0.01),
            "proportional weights: VTI should get 3x VXUS, got ratio {ratio}"
        );
    }
}
