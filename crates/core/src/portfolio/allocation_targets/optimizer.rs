use std::collections::HashMap;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use crate::errors::Result as CoreResult;

use super::model::{
    RebalanceGoal, RebalancePlan, RebalanceWarning, RebalanceWarningKind, SuggestedManualTrade,
};

// ── Input types ───────────────────────────────────────────────────────────────

pub struct RebalanceProfile {
    pub target_id: String,
    pub drift_band_bps: i32,
    pub rebalance_goal: RebalanceGoal,
    pub min_trade_amount: Decimal,
    pub whole_shares_only: bool,
}

pub struct CategoryState {
    pub category_id: String,
    pub category_name: String,
    pub target_bps: i32,
    pub current_value: Decimal,
    pub is_cash: bool,
    pub is_required: bool,
}

pub struct AssetCandidate {
    pub holding_id: String,
    pub asset_id: String,
    pub symbol: String,
    pub name: Option<String>,
    pub price: Decimal,
    /// Value added per share in base currency, keyed by category_id.
    /// __UNKNOWN__ excluded. May sum to less than price (partial classification).
    pub exposure_per_share: HashMap<String, Decimal>,
}

pub struct RebalanceInput {
    pub profile: RebalanceProfile,
    pub available_cash: Decimal,
    pub total_value: Decimal,
    pub categories: Vec<CategoryState>,
    pub candidates: Vec<AssetCandidate>,
    /// Pre-populated classification warnings (UnclassifiedAsset, PartialClassification).
    pub warnings: Vec<RebalanceWarning>,
}

// ── Trait ─────────────────────────────────────────────────────────────────────

pub trait RebalanceOptimizer: Send + Sync {
    fn plan(&self, input: RebalanceInput) -> CoreResult<RebalancePlan>;
}

// ── DriftPriorityOptimizer ────────────────────────────────────────────────────

/// Greedy exposure-aware planner: each iteration buys 1 share of the asset that
/// maximises total drift reduction per dollar across all taxonomy categories.
pub struct DriftPriorityOptimizer;

impl DriftPriorityOptimizer {
    /// Per-category drift the planner tries to minimise.
    ///
    /// `ExactTarget` measures distance to the exact target. `NearestBand` only counts
    /// the distance *outside* the [target ± band] tolerance, so once a category is
    /// inside its band it contributes zero and the greedy stops deploying into it.
    fn category_drift(
        bps: Decimal,
        target_bps: i32,
        goal: &RebalanceGoal,
        band: Decimal,
    ) -> Decimal {
        let dist = (bps - Decimal::from(target_bps)).abs();
        match goal {
            RebalanceGoal::ExactTarget => dist,
            RebalanceGoal::NearestBand => (dist - band).max(Decimal::ZERO),
        }
    }

    /// Σ drift for required non-cash categories (band-aware via `category_drift`).
    fn total_drift(
        values: &HashMap<String, Decimal>,
        categories: &[CategoryState],
        total_value: Decimal,
        goal: &RebalanceGoal,
        band: Decimal,
    ) -> Decimal {
        if total_value == Decimal::ZERO {
            return Decimal::ZERO;
        }
        let scale = dec!(10000);
        categories
            .iter()
            .filter(|c| c.is_required && !c.is_cash)
            .map(|c| {
                let v = values.get(&c.category_id).copied().unwrap_or_default();
                let bps = v / total_value * scale;
                Self::category_drift(bps, c.target_bps, goal, band)
            })
            .sum()
    }

    /// Same as `total_drift` but adds `exposure` delta without mutating state.
    fn total_drift_with_buy(
        values: &HashMap<String, Decimal>,
        categories: &[CategoryState],
        total_value: Decimal,
        exposure: &HashMap<String, Decimal>,
        goal: &RebalanceGoal,
        band: Decimal,
    ) -> Decimal {
        if total_value == Decimal::ZERO {
            return Decimal::ZERO;
        }
        let scale = dec!(10000);
        categories
            .iter()
            .filter(|c| c.is_required && !c.is_cash)
            .map(|c| {
                let base = values.get(&c.category_id).copied().unwrap_or_default();
                let delta = exposure.get(&c.category_id).copied().unwrap_or_default();
                let bps = (base + delta) / total_value * scale;
                Self::category_drift(bps, c.target_bps, goal, band)
            })
            .sum()
    }

    /// Max |current_bps[c] - target_bps[c]| for required categories (including cash).
    fn max_drift_bps(
        values: &HashMap<String, Decimal>,
        categories: &[CategoryState],
        total_value: Decimal,
    ) -> i32 {
        if total_value == Decimal::ZERO {
            return 0;
        }
        let scale = dec!(10000);
        categories
            .iter()
            .filter(|c| c.is_required)
            .map(|c| {
                let v = values.get(&c.category_id).copied().unwrap_or_default();
                let bps: i32 = (v / total_value * scale)
                    .round()
                    .to_string()
                    .parse()
                    .unwrap_or(0);
                (bps - c.target_bps).abs()
            })
            .max()
            .unwrap_or(0)
    }
}

impl RebalanceOptimizer for DriftPriorityOptimizer {
    fn plan(&self, input: RebalanceInput) -> CoreResult<RebalancePlan> {
        let RebalanceInput {
            profile,
            available_cash,
            total_value,
            categories,
            mut candidates,
            mut warnings,
        } = input;

        if total_value == Decimal::ZERO && available_cash == Decimal::ZERO {
            return Ok(RebalancePlan {
                target_id: profile.target_id,
                available_cash: Decimal::ZERO,
                cash_used: Decimal::ZERO,
                cash_remaining: Decimal::ZERO,
                max_drift_bps_before: 0,
                max_drift_bps_after: 0,
                trades: vec![],
                warnings,
                after_bps_by_category: HashMap::new(),
            });
        }

        let scale = dec!(10000);
        let drift_band = Decimal::from(profile.drift_band_bps);

        let mut values: HashMap<String, Decimal> = categories
            .iter()
            .map(|c| (c.category_id.clone(), c.current_value))
            .collect();

        let max_drift_before = Self::max_drift_bps(&values, &categories, total_value);

        // Emit NoBuyCandidate for required underweight categories with no candidate coverage.
        // Track them so sleeve-level dollar trades can be added after the greedy.
        let mut no_candidate_categories: Vec<&CategoryState> = Vec::new();
        for cat in categories.iter().filter(|c| c.is_required && !c.is_cash) {
            let desired_bps = match profile.rebalance_goal {
                RebalanceGoal::ExactTarget => Decimal::from(cat.target_bps),
                RebalanceGoal::NearestBand => {
                    (Decimal::from(cat.target_bps) - drift_band).max(Decimal::ZERO)
                }
            };
            let desired_value = desired_bps / scale * total_value;
            if cat.current_value >= desired_value {
                continue;
            }
            let covered = candidates
                .iter()
                .any(|c| c.exposure_per_share.contains_key(&cat.category_id));
            if !covered {
                let shortfall = (desired_value - cat.current_value).max(Decimal::ZERO);
                warnings.push(RebalanceWarning {
                    kind: RebalanceWarningKind::NoBuyCandidate,
                    category_id: cat.category_id.clone(),
                    message: format!(
                        "No classifiable holdings in {}. Allocate {:.2} to this category manually.",
                        cat.category_name, shortfall,
                    ),
                });
                no_candidate_categories.push(cat);
            }
        }

        // Sort by price ASC for tie-breaking on equal scores, then by (symbol, asset_id)
        // so equal-price candidates have a stable, reproducible order across runs.
        candidates.sort_by(|a, b| {
            a.price
                .cmp(&b.price)
                .then_with(|| a.symbol.cmp(&b.symbol))
                .then_with(|| a.asset_id.cmp(&b.asset_id))
        });

        let mut shares_bought: Vec<Decimal> = vec![Decimal::ZERO; candidates.len()];
        let mut cash = available_cash;

        // Greedy: each iteration buys 1 share of the candidate with the highest
        // (drift_before - drift_after) / price score.
        loop {
            if cash <= Decimal::ZERO {
                break;
            }
            let drift_before = Self::total_drift(
                &values,
                &categories,
                total_value,
                &profile.rebalance_goal,
                drift_band,
            );

            let mut best_score = Decimal::ZERO;
            let mut best_idx: Option<usize> = None;

            for (idx, candidate) in candidates.iter().enumerate() {
                if cash < candidate.price {
                    continue;
                }
                let drift_after = Self::total_drift_with_buy(
                    &values,
                    &categories,
                    total_value,
                    &candidate.exposure_per_share,
                    &profile.rebalance_goal,
                    drift_band,
                );
                let improvement = drift_before - drift_after;
                if improvement <= Decimal::ZERO {
                    continue;
                }
                let score = improvement / candidate.price;
                // candidates sorted price ASC — first found wins ties (cheaper asset preferred)
                if score > best_score {
                    best_score = score;
                    best_idx = Some(idx);
                }
            }

            let Some(idx) = best_idx else {
                break;
            };

            let candidate = &candidates[idx];
            for (cat_id, expo) in &candidate.exposure_per_share {
                *values.entry(cat_id.clone()).or_default() += expo;
            }
            cash -= candidate.price;
            shares_bought[idx] += Decimal::ONE;
        }

        // Build trades from accumulated shares; apply min_trade_amount filter.
        let mut trades: Vec<SuggestedManualTrade> = Vec::new();

        for (idx, &shares) in shares_bought.iter().enumerate() {
            if shares == Decimal::ZERO {
                continue;
            }
            let candidate = &candidates[idx];
            let estimated_amount = shares * candidate.price;

            if profile.min_trade_amount > Decimal::ZERO
                && estimated_amount < profile.min_trade_amount
            {
                continue;
            }

            // Primary category = category with the largest per-share exposure.
            let (primary_cat_id, primary_cat_name) = candidate
                .exposure_per_share
                .iter()
                .max_by(|(_, a), (_, b)| a.cmp(b))
                .map(|(cat_id, _)| {
                    let name = categories
                        .iter()
                        .find(|c| &c.category_id == cat_id)
                        .map(|c| c.category_name.clone())
                        .unwrap_or_else(|| cat_id.clone());
                    (cat_id.clone(), name)
                })
                .unwrap_or_else(|| ("unknown".to_string(), "Unknown".to_string()));

            trades.push(SuggestedManualTrade {
                action: "buy".to_string(),
                category_id: primary_cat_id,
                category_name: primary_cat_name,
                asset_id: Some(candidate.asset_id.clone()),
                symbol: Some(candidate.symbol.clone()),
                name: candidate.name.clone(),
                quantity: Some(shares),
                estimated_price: Some(candidate.price),
                estimated_amount,
                reason: format!("{} improves portfolio drift.", candidate.symbol),
            });
        }

        // Sleeve-level dollar trades for uncovered underweight categories.
        // Draw from the cash left after the greedy whole-share buys, decrementing as we
        // go, so multiple uncovered categories can never collectively overspend.
        let mut manual_cash = cash;
        for cat in &no_candidate_categories {
            if manual_cash <= Decimal::ZERO {
                break;
            }
            let desired_bps = match profile.rebalance_goal {
                RebalanceGoal::ExactTarget => Decimal::from(cat.target_bps),
                RebalanceGoal::NearestBand => {
                    (Decimal::from(cat.target_bps) - drift_band).max(Decimal::ZERO)
                }
            };
            let shortfall =
                ((desired_bps / scale * total_value) - cat.current_value).max(Decimal::ZERO);
            let amount = shortfall.min(manual_cash);
            if amount > Decimal::ZERO {
                manual_cash -= amount;
                trades.push(SuggestedManualTrade {
                    action: "buy".to_string(),
                    category_id: cat.category_id.clone(),
                    category_name: cat.category_name.clone(),
                    asset_id: None,
                    symbol: None,
                    name: None,
                    quantity: None,
                    estimated_price: None,
                    estimated_amount: amount,
                    reason: format!(
                        "Category {} is underweight. Allocate manually.",
                        cat.category_name
                    ),
                });
            }
        }

        // cash_used = sum of recommended trade amounts (post min_trade filter).
        // Keeps cash_used consistent with what the user will actually execute.
        let cash_used: Decimal = trades.iter().map(|t| t.estimated_amount).sum();
        let cash_remaining = available_cash - cash_used;

        // After-drift: recompute from initial state + recommended trades only.
        let mut after_values: HashMap<String, Decimal> = categories
            .iter()
            .map(|c| (c.category_id.clone(), c.current_value))
            .collect();
        for trade in &trades {
            if let Some(asset_id) = &trade.asset_id {
                if let Some(candidate) = candidates.iter().find(|c| &c.asset_id == asset_id) {
                    let shares = trade.quantity.unwrap_or(Decimal::ZERO);
                    for (cat_id, expo) in &candidate.exposure_per_share {
                        *after_values.entry(cat_id.clone()).or_default() += expo * shares;
                    }
                }
            } else {
                // Manual sleeve trade (no ticker): the deployed cash lands directly in
                // the target category. Credit it so after-drift reflects the move.
                *after_values.entry(trade.category_id.clone()).or_default() +=
                    trade.estimated_amount;
            }
        }
        for cat in categories.iter().filter(|c| c.is_cash) {
            let entry = after_values.entry(cat.category_id.clone()).or_default();
            *entry = (*entry - cash_used).max(Decimal::ZERO);
        }
        let max_drift_after = Self::max_drift_bps(&after_values, &categories, total_value);

        let after_bps_by_category: HashMap<String, i32> = if total_value > Decimal::ZERO {
            after_values
                .iter()
                .map(|(cat_id, val)| {
                    let bps: i32 = (*val / total_value * scale)
                        .round()
                        .to_string()
                        .parse()
                        .unwrap_or(0);
                    (cat_id.clone(), bps)
                })
                .collect()
        } else {
            HashMap::new()
        };

        Ok(RebalancePlan {
            target_id: profile.target_id,
            available_cash,
            cash_used,
            cash_remaining,
            max_drift_bps_before: max_drift_before,
            max_drift_bps_after: max_drift_after,
            trades,
            warnings,
            after_bps_by_category,
        })
    }
}
