use async_trait::async_trait;
use log::debug;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::str::FromStr;
use std::sync::Arc;

use crate::errors::{Error as CoreError, Result as CoreResult};
use crate::portfolio::allocation::AllocationServiceTrait;
use crate::portfolio::holdings::{HoldingSummary, HoldingType, HoldingsServiceTrait};

use super::drift_service::DriftServiceTrait;
use super::model::{
    CalculateRebalancePlanInput, RebalanceGoal, RebalancePlan, RebalanceWarning,
    RebalanceWarningKind, SuggestedManualTrade,
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

        // M2 deploys tracked cash only. Compute the cash in scope from holdings
        // (authoritative — does not trust a client-supplied figure) and reject
        // attempts to deploy more than is actually held.
        let cash_in_scope: Decimal = self
            .holdings_service
            .get_holdings_for_accounts(
                &input.account_ids,
                &input.base_currency,
                &input.aggregated_account_id,
            )
            .await?
            .iter()
            .filter(|holding| holding.holding_type == HoldingType::Cash)
            .map(|holding| holding.market_value.base)
            .sum();

        if input.available_cash > cash_in_scope {
            return Err(CoreError::Validation(
                crate::errors::ValidationError::InvalidInput(
                    "cash to deploy exceeds tracked cash in scope".to_string(),
                ),
            ));
        }

        // --- 1. Load profile -------------------------------------------------
        let profile = self
            .allocation_target_service
            .get_target(&input.target_id)?
            .ok_or_else(|| {
                CoreError::Database(crate::errors::DatabaseError::NotFound(format!(
                    "AllocationTarget {} not found",
                    input.target_id
                )))
            })?;

        // --- 2. Drift report (reuse M1 DriftService) -------------------------
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
        let max_drift_bps_before = drift.max_drift_bps;
        let min_trade_amount =
            Decimal::from_str(&profile.min_trade_amount).unwrap_or(Decimal::ZERO);

        // Nothing to plan against.
        if total_value == Decimal::ZERO && input.available_cash == Decimal::ZERO {
            return Ok(RebalancePlan {
                target_id: input.target_id.clone(),
                available_cash: Decimal::ZERO,
                cash_used: Decimal::ZERO,
                cash_remaining: Decimal::ZERO,
                max_drift_bps_before: 0,
                max_drift_bps_after: 0,
                trades: vec![],
                warnings: vec![],
            });
        }

        // Total portfolio value stays constant — tracked cash is already inside the portfolio.
        // Deploying cash moves value from CASH holdings into buy trades, it does not add new value.
        let new_total_value = total_value;
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
            let desired_bps = match profile.rebalance_goal {
                RebalanceGoal::ExactTarget => target_bps_dec,
                RebalanceGoal::NearestBand => (target_bps_dec - drift_band).max(Decimal::ZERO),
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
                        "No holdings found in {}. Allocate {:.2} to this sleeve manually.",
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
            // Track holdings skipped in Phase 1 due to budget < price (whole-share mode).
            // Emit a warning post-Phase-2 only if they are still un-funded.
            let mut sleeve_skipped: Vec<(String, Decimal, Decimal)> = Vec::new();

            for holding in holdings {
                // Proportional budget for this holding.
                let weight = if sleeve_total_value > Decimal::ZERO {
                    holding.market_value / sleeve_total_value
                } else {
                    Decimal::ONE / Decimal::from(holdings.len() as i64)
                };
                let holding_budget = budget * weight;

                // Derive price per share.
                // Prefer unit_price (actual quote from provider) over market_value/quantity,
                // which gives a wrong proportional price when a holding spans multiple
                // taxonomy categories (e.g. a multi-sector ETF in GICS).
                let price = if let Some(p) = holding.unit_price.filter(|p| *p > Decimal::ZERO) {
                    p
                } else if holding.quantity > Decimal::ZERO && holding.market_value > Decimal::ZERO {
                    holding.market_value / holding.quantity
                } else {
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
                    if holding_budget < min_trade_amount {
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
                };

                let (shares, amount) = if profile.whole_shares_only {
                    let whole = (holding_budget / price).floor();
                    if whole == Decimal::ZERO {
                        // Skip emission for now — Phase 2 may rescue this holding from
                        // sleeve residue. Warning is emitted post-Phase-2 if still skipped.
                        sleeve_skipped.push((holding.symbol.clone(), holding_budget, price));
                        continue;
                    }
                    let amt = whole * price;
                    (Some(whole), amt)
                } else {
                    // Fractional: full budget, compute shares for display.
                    (Some(holding_budget / price), holding_budget)
                };

                if amount < min_trade_amount {
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

            // --- Phase 2: Intra-sleeve residue absorption (whole_shares_only) ---
            // After Phase 1 round-lot allocation, rounding residue remains. We
            // redistribute it by buying ONE share at a time, cheapest-first.
            //
            // Rationale: a proportional `residue * weight / price` formula fails
            // for small residues — each holding's slice is below its share price
            // and the loop absorbs nothing. Buying one share per pass distributes
            // the residue fairly without letting the cheapest holding absorb it
            // all in a single shot.
            //
            // Side effect (intentional): a holding skipped in Phase 1 (budget <
            // price) can be rescued here if the residue >= price. Holdings whose
            // price exceeds the entire sleeve residue stay starved — see the
            // post-loop warning below.
            if profile.whole_shares_only {
                let mut residue = budget - sleeve_deployed;

                // Build (holding, price) pairs filtered to those with a valid price,
                // sorted by price ascending.
                let mut priced: Vec<(&HoldingSummary, Decimal)> = holdings
                    .iter()
                    .filter_map(|h| {
                        let p = h.unit_price.filter(|p| *p > Decimal::ZERO).or_else(|| {
                            if h.quantity > Decimal::ZERO && h.market_value > Decimal::ZERO {
                                Some(h.market_value / h.quantity)
                            } else {
                                None
                            }
                        })?;
                        Some((h, p))
                    })
                    .collect();
                priced.sort_by_key(|a| a.1);

                'deploy_residue: loop {
                    let mut absorbed = false;
                    for (holding, price) in &priced {
                        if residue <= Decimal::ZERO {
                            break 'deploy_residue;
                        }
                        if residue < *price {
                            continue;
                        }
                        // Buy exactly 1 share per pass.
                        let existing = trades.iter_mut().rev().find(|t| {
                            t.asset_id.as_deref() == Some(holding.id.as_str())
                                && t.category_id == sleeve.category_id
                        });
                        if let Some(t) = existing {
                            t.quantity = t.quantity.map(|q| q + Decimal::ONE);
                            t.estimated_amount += *price;
                        } else if *price >= min_trade_amount {
                            // New trade (Phase 1 had skipped this holding).
                            trades.push(SuggestedManualTrade {
                                action: "buy".to_string(),
                                category_id: sleeve.category_id.clone(),
                                category_name: sleeve.category_name.clone(),
                                asset_id: Some(holding.id.clone()),
                                symbol: Some(holding.symbol.clone()),
                                name: holding.name.clone(),
                                quantity: Some(Decimal::ONE),
                                estimated_price: Some(*price),
                                estimated_amount: *price,
                                reason: format!(
                                    "{} is underweight in {}.",
                                    holding.symbol, sleeve.category_name
                                ),
                            });
                        } else {
                            // Sub-min-trade new trade — skip and treat as undeployed.
                            continue;
                        }
                        sleeve_deployed += *price;
                        residue -= *price;
                        absorbed = true;
                    }
                    if !absorbed {
                        break;
                    }
                }

                // Emit a warning for any Phase-1-skipped holding that Phase 2 did
                // not rescue. Suggest the top-up amount needed to fund 1 share.
                for (symbol, original_budget, price) in &sleeve_skipped {
                    let funded = trades.iter().any(|t| {
                        t.symbol.as_deref() == Some(symbol.as_str())
                            && t.category_id == sleeve.category_id
                    });
                    if !funded {
                        let topup = *price - *original_budget;
                        warnings.push(RebalanceWarning {
                            kind: RebalanceWarningKind::WholeShareResidue,
                            category_id: sleeve.category_id.clone(),
                            message: format!(
                                "{}: budget {:.2} is short of 1 share at {:.2}. Add ~{:.2} more cash to fund 1 share, or {} will drift below target over time.",
                                symbol, original_budget, price, topup, symbol
                            ),
                        });
                    }
                }
            }

            *deployed_per_sleeve
                .entry(sleeve.category_id.clone())
                .or_default() += sleeve_deployed;
            total_cash_used += sleeve_deployed;
        }

        // --- 6. Estimate max_drift_bps_after ---------------------------------
        // Total value is constant (tracked cash moves within portfolio, not added from outside).
        // Only consider required rows — matches DriftService.max_drift_bps semantics.
        let max_drift_bps_after = if total_value == Decimal::ZERO {
            0
        } else {
            drift
                .rows
                .iter()
                .filter(|row| row.is_required)
                .map(|row| {
                    // Deploying cash moves value out of the cash sleeve into buy
                    // sleeves. Total value is unchanged, so the cash row must shed
                    // total_cash_used for after-drift to stay consistent.
                    let new_value = if row.is_cash {
                        (row.current_value - total_cash_used).max(Decimal::ZERO)
                    } else {
                        let deployed = deployed_per_sleeve
                            .get(&row.category_id)
                            .copied()
                            .unwrap_or(Decimal::ZERO);
                        row.current_value + deployed
                    };
                    let new_bps = (new_value / total_value * bps_scale)
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

        Ok(RebalancePlan {
            target_id: input.target_id.clone(),
            available_cash: input.available_cash,
            cash_used: total_cash_used,
            cash_remaining,
            max_drift_bps_before,
            max_drift_bps_after,
            trades,
            warnings,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portfolio::allocation::{AllocationHoldings, PortfolioAllocations};
    use crate::portfolio::allocation_targets::{
        AllocationTarget, AllocationTargetWeight, DriftReport, DriftRow, DriftStatus,
        NewAllocationTarget, NewAllocationTargetWeight, RebalanceGoal, ScopeType, TriggerType,
    };
    use crate::portfolio::holdings::{Holding, HoldingSummary, HoldingType, MonetaryValue};
    use rust_decimal_macros::dec;

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

    fn make_holding(
        id: &str,
        symbol: &str,
        quantity: Decimal,
        market_value: Decimal,
    ) -> HoldingSummary {
        let unit_price = if quantity > Decimal::ZERO {
            Some(market_value / quantity)
        } else {
            None
        };
        HoldingSummary {
            id: id.to_string(),
            symbol: symbol.to_string(),
            name: Some(symbol.to_string()),
            holding_type: HoldingType::Security,
            quantity,
            market_value,
            currency: "USD".to_string(),
            weight_in_category: Decimal::ZERO,
            unit_price,
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
        async fn get_holding_contributions_for_taxonomy_for_accounts(
            &self,
            _: &[String],
            _: &str,
            _: &str,
            _: &str,
        ) -> CoreResult<crate::portfolio::allocation::TaxonomyHoldingContributions> {
            unimplemented!()
        }
    }

    struct MockHoldingsService {
        cash_in_scope: Decimal,
    }

    #[async_trait]
    impl crate::portfolio::holdings::HoldingsServiceTrait for MockHoldingsService {
        async fn get_holdings(&self, _: &str, _: &str) -> CoreResult<Vec<Holding>> {
            unimplemented!()
        }
        async fn get_holdings_for_accounts(
            &self,
            _: &[String],
            base_currency: &str,
            _: &str,
        ) -> CoreResult<Vec<Holding>> {
            // Single cash holding carrying the configured in-scope cash.
            Ok(vec![Holding {
                id: "cash".to_string(),
                account_id: "acc-1".to_string(),
                holding_type: HoldingType::Cash,
                instrument: None,
                asset_kind: None,
                quantity: self.cash_in_scope,
                open_date: None,
                lots: None,
                contract_multiplier: Decimal::ONE,
                local_currency: base_currency.to_string(),
                base_currency: base_currency.to_string(),
                fx_rate: None,
                market_value: MonetaryValue {
                    local: self.cash_in_scope,
                    base: self.cash_in_scope,
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
            }])
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

    fn make_service(
        profile: AllocationTarget,
        report: DriftReport,
        holdings: std::collections::HashMap<String, Vec<HoldingSummary>>,
    ) -> RebalanceService {
        // Large default so existing tests can deploy freely; the rejection test
        // uses make_service_with_cash to constrain it.
        make_service_with_cash(profile, report, holdings, dec!(1_000_000))
    }

    fn make_service_with_cash(
        profile: AllocationTarget,
        report: DriftReport,
        holdings: std::collections::HashMap<String, Vec<HoldingSummary>>,
        cash_in_scope: Decimal,
    ) -> RebalanceService {
        RebalanceService::new(
            Arc::new(MockTargetService { profile }),
            Arc::new(MockDriftService { report }),
            Arc::new(MockAllocationService {
                holdings_by_category: holdings,
            }),
            Arc::new(MockHoldingsService { cash_in_scope }),
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
            make_profile(RebalanceGoal::ExactTarget, false),
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
            make_profile(RebalanceGoal::ExactTarget, false),
            report,
            holdings,
        );
        let plan = svc.calculate_plan(make_input(dec!(0))).await.unwrap();

        assert!(plan.trades.is_empty());
        assert_eq!(plan.cash_used, Decimal::ZERO);
    }

    #[tokio::test]
    async fn negative_cash_returns_validation_error() {
        let total = dec!(10000);
        let rows = vec![make_drift_row("equity", 6000, 7000, total)];
        let report = make_report(rows, total);
        let mut holdings = std::collections::HashMap::new();
        holdings.insert(
            "equity".to_string(),
            vec![make_holding("h1", "VTI", dec!(10), dec!(6000))],
        );
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            report,
            holdings,
        );
        let err = svc
            .calculate_plan(make_input(dec!(-100)))
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("non-negative"),
            "negative cash must be rejected: {err}"
        );
    }

    #[tokio::test]
    async fn deploy_exceeding_tracked_cash_is_rejected() {
        // Only $500 of cash is tracked in scope, but the caller asks to deploy $1000.
        // Backend must reject regardless of what the UI allowed.
        let total = dec!(10000);
        let rows = vec![make_drift_row("equity", 6000, 7000, total)];
        let report = make_report(rows, total);
        let mut holdings = std::collections::HashMap::new();
        holdings.insert(
            "equity".to_string(),
            vec![make_holding("h1", "VTI", dec!(10), dec!(6000))],
        );
        let svc = make_service_with_cash(
            make_profile(RebalanceGoal::ExactTarget, false),
            report,
            holdings,
            dec!(500),
        );
        let err = svc
            .calculate_plan(make_input(dec!(1000)))
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("exceeds tracked cash in scope"),
            "deploy over tracked cash must be rejected: {err}"
        );
    }

    #[tokio::test]
    async fn total_value_stays_constant_when_cash_deployed() {
        // Portfolio: equity $6000 (60%), cash $4000 (40%). Total = $10000.
        // Target: equity 70%, cash 30%.
        // Deploy $2000 of cash → buys equity.
        // Total value must stay $10000 (cash moves within portfolio, not added).
        let total = dec!(10000);
        let rows = vec![
            make_drift_row("equity", 6000, 7000, total),
            DriftRow {
                is_cash: true,
                ..make_drift_row("cash", 4000, 3000, total)
            },
        ];
        let report = make_report(rows, total);
        let mut holdings = std::collections::HashMap::new();
        holdings.insert(
            "equity".to_string(),
            vec![make_holding("h1", "VTI", dec!(10), dec!(6000))],
        );
        let svc = make_service(
            make_profile(RebalanceGoal::ExactTarget, false),
            report,
            holdings,
        );
        let plan = svc.calculate_plan(make_input(dec!(2000))).await.unwrap();

        // cash_used + cash_remaining must equal available_cash (no value created)
        assert_eq!(plan.cash_used + plan.cash_remaining, dec!(2000));
        // Equity shortfall is $1000 (60% → 70%); deploying it pulls cash 40% → 30%.
        // The cash sleeve must shed cash_used so both sleeves hit target → after-drift 0.
        // Without subtracting cash_used the cash row would stay at 40% (drift +1000bps).
        assert_eq!(plan.cash_used, dec!(1000));
        assert_eq!(
            plan.max_drift_bps_after, 0,
            "cash sleeve must drop by cash_used so both sleeves reach target"
        );
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
            make_profile(RebalanceGoal::ExactTarget, false),
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
            make_profile(RebalanceGoal::ExactTarget, false),
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
            make_profile(RebalanceGoal::ExactTarget, true),
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
            make_profile(RebalanceGoal::ExactTarget, false),
            report_exact,
            h.clone(),
        );
        let svc_band = make_service(
            make_profile(RebalanceGoal::NearestBand, false),
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
    async fn min_trade_amount_filters_small_trades() {
        // Equity 60% current, 80% target → $2000 shortfall.
        // Two holdings: VTI $900 (90%), IVV $100 (10%).
        // Cash = $200 → VTI gets $180, IVV gets $20.
        // min_trade_amount = $100 → IVV trade dropped, VTI kept.
        let total = dec!(10000);
        let rows = vec![make_drift_row("equity", 6000, 8000, total)];
        let report = make_report(rows, total);
        let mut holdings = std::collections::HashMap::new();
        holdings.insert(
            "equity".to_string(),
            vec![
                make_holding("h1", "VTI", dec!(9), dec!(900)),
                make_holding("h2", "IVV", dec!(1), dec!(100)),
            ],
        );
        let profile = AllocationTarget {
            min_trade_amount: "100".to_string(),
            ..make_profile(RebalanceGoal::ExactTarget, false)
        };

        let svc = make_service(profile, report, holdings);
        let plan = svc.calculate_plan(make_input(dec!(200))).await.unwrap();

        // VTI gets $180 → above $100 → kept.
        assert!(
            plan.trades
                .iter()
                .any(|t| t.symbol.as_deref() == Some("VTI")),
            "VTI trade should survive min_trade filter"
        );
        // IVV gets $20 → below $100 → dropped.
        assert!(
            !plan
                .trades
                .iter()
                .any(|t| t.symbol.as_deref() == Some("IVV")),
            "IVV trade should be filtered by min_trade_amount"
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

        let svc = make_service(make_profile(RebalanceGoal::ExactTarget, false), report, h);
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

    #[tokio::test]
    async fn phase2_distributes_residue_one_share_at_a_time() {
        // whole_shares_only ON. Equity sleeve underweight.
        // Three holdings in sleeve, total market value = $420:
        //   A: price $10, mv $100 (weight 23.81%)
        //   B: price $30, mv $120 (weight 28.57%)
        //   C: price $50, mv $200 (weight 47.62%)
        // Cash $100 → sleeve budget $100 (scale_factor < 1).
        //   Phase 1: A gets $23.81 → 2 shares × $10 = $20. B gets $28.57 → 0 (< $30 price), skipped.
        //            C gets $47.62 → 0 (< $50 price), skipped. Deployed $20, residue $80.
        // Phase 2 ASC (A 10, B 30, C 50), 1-share-at-a-time:
        //   Pass 1: A +1 ($10), residue $70. B +1 ($30), residue $40. C: $40<$50 skip.
        //   Pass 2: A +1 ($10), residue $30. B +1 ($30), residue $0.
        // Final: A=4 shares, B=2 shares (rescued), C still starved.
        //
        // Verifies BOTH:
        //  (a) Phase 2 absorbs residue (vs proportional Phase 2 which would absorb 0).
        //  (b) Phase 2 does not give all residue to cheapest (vs original bug where A
        //      would get floor(80/10)=8 shares and B would still be starved).
        let total = dec!(10000);
        let rows = vec![make_drift_row("equity", 4000, 8000, total)];
        let report = make_report(rows, total);

        let mut h = std::collections::HashMap::new();
        h.insert(
            "equity".to_string(),
            vec![
                make_holding("a", "A", dec!(10), dec!(100)),
                make_holding("b", "B", dec!(4), dec!(120)),
                make_holding("c", "C", dec!(4), dec!(200)),
            ],
        );

        let svc = make_service(make_profile(RebalanceGoal::ExactTarget, true), report, h);
        let plan = svc.calculate_plan(make_input(dec!(100))).await.unwrap();

        let a = plan
            .trades
            .iter()
            .find(|t| t.symbol.as_deref() == Some("A"))
            .expect("A trade present");
        let b = plan
            .trades
            .iter()
            .find(|t| t.symbol.as_deref() == Some("B"))
            .expect("B trade present (rescued from skipped by Phase 2)");
        let c = plan
            .trades
            .iter()
            .find(|t| t.symbol.as_deref() == Some("C"));

        assert_eq!(
            a.quantity,
            Some(dec!(4)),
            "A absorbs only what it needs (1 from Phase 1 + 3 from Phase 2), not all residue"
        );
        assert_eq!(
            b.quantity,
            Some(dec!(2)),
            "B rescued from Phase 1 skip — Phase 2 buys 2 shares from residue"
        );
        assert!(
            c.is_none(),
            "C remains starved (price $50 never fits in residue once A and B consume their share)"
        );
        assert_eq!(
            plan.cash_used,
            dec!(100),
            "Full $100 deployed across A and B"
        );
    }

    #[tokio::test]
    async fn phase2_emits_topup_warning_for_unrescued_starved_holding() {
        // Single expensive holding X (price $240, only holding in sleeve).
        // Cash $200 → sleeve budget $200 < $240 price.
        // Phase 1: X skipped. Phase 2: residue $200 < $240 → cannot rescue.
        // Warning emitted with topup = $240 - $200 = $40.
        let total = dec!(10000);
        let rows = vec![make_drift_row("equity", 5000, 8000, total)];
        let report = make_report(rows, total);

        let mut h = std::collections::HashMap::new();
        h.insert(
            "equity".to_string(),
            vec![make_holding("x", "EXPENSIVE", dec!(5), dec!(1200))],
        );

        let svc = make_service(make_profile(RebalanceGoal::ExactTarget, true), report, h);
        let plan = svc.calculate_plan(make_input(dec!(200))).await.unwrap();

        // No trade for EXPENSIVE.
        assert!(
            !plan
                .trades
                .iter()
                .any(|t| t.symbol.as_deref() == Some("EXPENSIVE")),
            "EXPENSIVE should be skipped (price $240 > sleeve budget $200)"
        );

        // Warning emitted with topup info.
        let warn = plan
            .warnings
            .iter()
            .find(|w| w.kind == RebalanceWarningKind::WholeShareResidue)
            .expect("WholeShareResidue warning emitted for starved holding");
        assert!(
            warn.message.contains("EXPENSIVE"),
            "warning names the symbol: {}",
            warn.message
        );
        assert!(
            warn.message.contains("40"),
            "warning includes topup amount ($40 to fund 1 share): {}",
            warn.message
        );
    }
}
