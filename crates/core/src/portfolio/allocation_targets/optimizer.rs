use std::collections::HashMap;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use crate::errors::Result as CoreResult;

use super::model::{
    RebalanceGoal, RebalancePlan, RebalanceWarning, RebalanceWarningKind, ScenarioMode,
    SuggestedManualTrade,
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

pub struct SellCandidate {
    pub holding_id: String,
    pub asset_id: String,
    pub symbol: String,
    pub name: Option<String>,
    pub price: Decimal,
    pub quantity_owned: Decimal,
    /// Same semantics as AssetCandidate: value removed per share sold.
    pub exposure_per_share: HashMap<String, Decimal>,
}

pub struct RebalanceInput {
    pub profile: RebalanceProfile,
    pub scenario_mode: ScenarioMode,
    pub available_cash: Decimal,
    pub total_value: Decimal,
    pub categories: Vec<CategoryState>,
    pub candidates: Vec<AssetCandidate>,
    pub sell_candidates: Vec<SellCandidate>,
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
    fn desired_bps_for_goal(target_bps: i32, goal: &RebalanceGoal, band: Decimal) -> Decimal {
        match goal {
            RebalanceGoal::ExactTarget => Decimal::from(target_bps),
            RebalanceGoal::NearestBand => (Decimal::from(target_bps) - band).max(Decimal::ZERO),
        }
    }

    fn cap_fractional_shares_to_next_bend(
        candidate: &AssetCandidate,
        cash: Decimal,
        values: &HashMap<String, Decimal>,
        categories: &[CategoryState],
        total_value: Decimal,
        goal: &RebalanceGoal,
        band: Decimal,
    ) -> Decimal {
        if candidate.price <= Decimal::ZERO || cash <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        let scale = dec!(10000);
        let mut shares = cash / candidate.price;

        for cat in categories.iter().filter(|c| c.is_required && !c.is_cash) {
            let Some(expo) = candidate.exposure_per_share.get(&cat.category_id) else {
                continue;
            };
            if *expo <= Decimal::ZERO {
                continue;
            }

            let desired_bps = Self::desired_bps_for_goal(cat.target_bps, goal, band);
            let desired_value = desired_bps / scale * total_value;
            let base = values.get(&cat.category_id).copied().unwrap_or_default();
            if base < desired_value {
                let cap = (desired_value - base) / expo;
                if cap < shares {
                    shares = cap;
                }
            }
        }

        shares.max(Decimal::ZERO)
    }

    fn exposure_delta(
        exposure_per_share: &HashMap<String, Decimal>,
        shares: Decimal,
    ) -> HashMap<String, Decimal> {
        exposure_per_share
            .iter()
            .map(|(cat_id, e)| (cat_id.clone(), e * shares))
            .collect()
    }

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

    /// Sell greedy: each iteration sells 1 share of the asset with the highest
    /// drift-improvement/dollar score. Returns (updated values, proceeds, sell trades).
    fn run_sell_phase(
        values: &HashMap<String, Decimal>,
        total_value: Decimal,
        categories: &[CategoryState],
        sell_candidates: &[SellCandidate],
        goal: &RebalanceGoal,
        drift_band: Decimal,
        whole_shares_only: bool,
    ) -> (HashMap<String, Decimal>, Decimal, Vec<SuggestedManualTrade>) {
        if total_value == Decimal::ZERO || sell_candidates.is_empty() {
            return (values.clone(), Decimal::ZERO, vec![]);
        }

        let scale = dec!(10000);
        let mut values = values.clone();
        let mut qty_remaining: Vec<Decimal> =
            sell_candidates.iter().map(|c| c.quantity_owned).collect();
        let mut shares_sold: Vec<Decimal> = vec![Decimal::ZERO; sell_candidates.len()];
        let mut proceeds = Decimal::ZERO;

        loop {
            let drift_before =
                Self::total_drift(&values, categories, total_value, goal, drift_band);
            if drift_before == Decimal::ZERO {
                break;
            }

            let mut best_score = Decimal::ZERO;
            let mut best_idx: Option<usize> = None;
            let mut best_sell_shares = Decimal::ZERO;

            for (idx, candidate) in sell_candidates.iter().enumerate() {
                if qty_remaining[idx] <= Decimal::ZERO {
                    continue;
                }
                if candidate.price <= Decimal::ZERO {
                    continue;
                }

                let sell_qty = if whole_shares_only {
                    if qty_remaining[idx] < Decimal::ONE {
                        continue;
                    }
                    Decimal::ONE
                } else {
                    // Cap fractional sell at the band edge for each exposed category
                    let mut max_shares = qty_remaining[idx];
                    for cat in categories.iter().filter(|c| c.is_required && !c.is_cash) {
                        let Some(expo) = candidate.exposure_per_share.get(&cat.category_id) else {
                            continue;
                        };
                        if *expo <= Decimal::ZERO {
                            continue;
                        }
                        let current_v = values.get(&cat.category_id).copied().unwrap_or_default();
                        let current_bps = current_v / total_value * scale;
                        // For NearestBand: stop at target + band (overweight edge)
                        // For ExactTarget: stop at exact target
                        let stop_bps = match goal {
                            RebalanceGoal::ExactTarget => Decimal::from(cat.target_bps),
                            RebalanceGoal::NearestBand => {
                                (Decimal::from(cat.target_bps) + drift_band).min(dec!(10000))
                            }
                        };
                        if current_bps <= stop_bps {
                            // This sleeve is not overweight — skip cap for it.
                            // Let drift_before - drift_after reject net-bad sells.
                            continue;
                        }
                        let stop_value = stop_bps / scale * total_value;
                        let cap = (current_v - stop_value) / expo;
                        if cap < max_shares {
                            max_shares = cap;
                        }
                    }
                    max_shares.min(qty_remaining[idx]).max(Decimal::ZERO)
                };

                if sell_qty <= Decimal::ZERO {
                    continue;
                }

                // Simulate sell: subtract exposure from values
                let neg_exposure: HashMap<String, Decimal> = candidate
                    .exposure_per_share
                    .iter()
                    .map(|(k, v)| (k.clone(), -(*v) * sell_qty))
                    .collect();
                let drift_after = Self::total_drift_with_buy(
                    &values,
                    categories,
                    total_value,
                    &neg_exposure,
                    goal,
                    drift_band,
                );
                let improvement = drift_before - drift_after;
                if improvement <= Decimal::ZERO {
                    continue;
                }
                let cost = candidate.price * sell_qty;
                if cost <= Decimal::ZERO {
                    continue;
                }
                let score = improvement / cost;
                if score > best_score {
                    best_score = score;
                    best_idx = Some(idx);
                    best_sell_shares = sell_qty;
                }
            }

            let Some(idx) = best_idx else {
                break;
            };

            let candidate = &sell_candidates[idx];
            let batch = if whole_shares_only {
                // Batch only when sole improving candidate
                let improving_count = sell_candidates
                    .iter()
                    .enumerate()
                    .filter(|(i, c)| {
                        if qty_remaining[*i] < Decimal::ONE || c.price <= Decimal::ZERO {
                            return false;
                        }
                        let neg: HashMap<String, Decimal> = c
                            .exposure_per_share
                            .iter()
                            .map(|(k, v)| (k.clone(), -*v))
                            .collect();
                        let da = Self::total_drift_with_buy(
                            &values,
                            categories,
                            total_value,
                            &neg,
                            goal,
                            drift_band,
                        );
                        (drift_before - da) > Decimal::ZERO
                    })
                    .count();

                if improving_count == 1 {
                    let mut cap = qty_remaining[idx].floor().max(Decimal::ONE);
                    for cat in categories.iter().filter(|c| c.is_required && !c.is_cash) {
                        let Some(expo) = candidate.exposure_per_share.get(&cat.category_id) else {
                            continue;
                        };
                        if *expo <= Decimal::ZERO {
                            continue;
                        }
                        let current_v = values.get(&cat.category_id).copied().unwrap_or_default();
                        let stop_bps = match goal {
                            RebalanceGoal::ExactTarget => Decimal::from(cat.target_bps),
                            RebalanceGoal::NearestBand => {
                                (Decimal::from(cat.target_bps) + drift_band).min(dec!(10000))
                            }
                        };
                        let stop_value = stop_bps / dec!(10000) * total_value;
                        if current_v > stop_value {
                            let shares_to_stop =
                                ((current_v - stop_value) / expo).floor().max(Decimal::ONE);
                            if shares_to_stop < cap {
                                cap = shares_to_stop;
                            }
                        }
                    }
                    cap.min(qty_remaining[idx])
                } else {
                    Decimal::ONE
                }
            } else {
                best_sell_shares
            };

            let actual = batch.min(qty_remaining[idx]);
            if actual <= Decimal::ZERO {
                break;
            }

            for (cat_id, expo) in &candidate.exposure_per_share {
                let entry = values.entry(cat_id.clone()).or_default();
                *entry = (*entry - expo * actual).max(Decimal::ZERO);
            }
            proceeds += candidate.price * actual;
            qty_remaining[idx] -= actual;
            shares_sold[idx] += actual;
        }

        // Build sell trades
        let sell_trades: Vec<SuggestedManualTrade> = sell_candidates
            .iter()
            .zip(shares_sold.iter())
            .filter(|(_, &shares)| shares > Decimal::ZERO)
            .map(|(candidate, &shares)| {
                let estimated_amount = shares * candidate.price;
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

                SuggestedManualTrade {
                    action: "sell".to_string(),
                    category_id: primary_cat_id,
                    category_name: primary_cat_name,
                    asset_id: Some(candidate.asset_id.clone()),
                    symbol: Some(candidate.symbol.clone()),
                    name: candidate.name.clone(),
                    quantity: Some(shares),
                    estimated_price: Some(candidate.price),
                    estimated_amount,
                    reason: format!(
                        "{} is overweight — selling reduces portfolio drift.",
                        candidate.symbol
                    ),
                }
            })
            .collect();

        (values, proceeds, sell_trades)
    }

    /// Greedy buy loop. Mutates `values` and returns shares bought per candidate index.
    #[allow(clippy::too_many_arguments)]
    fn run_buy_greedy(
        values: &mut HashMap<String, Decimal>,
        candidates: &[AssetCandidate],
        cash_pool: Decimal,
        categories: &[CategoryState],
        profile: &RebalanceProfile,
        drift_band: Decimal,
        total_value: Decimal,
        scale: Decimal,
    ) -> Vec<Decimal> {
        let mut shares_bought = vec![Decimal::ZERO; candidates.len()];
        let mut cash = cash_pool;

        loop {
            if cash <= Decimal::ZERO {
                break;
            }
            let drift_before = Self::total_drift(
                values,
                categories,
                total_value,
                &profile.rebalance_goal,
                drift_band,
            );

            let mut best_score = Decimal::ZERO;
            let mut best_idx: Option<usize> = None;
            let mut best_fractional_shares = Decimal::ZERO;
            let mut improving_whole_share_candidates = 0usize;

            for (idx, candidate) in candidates.iter().enumerate() {
                let (shares_to_score, amount_to_score, exposure_to_score) =
                    if profile.whole_shares_only {
                        if cash < candidate.price {
                            continue;
                        }
                        (
                            Decimal::ONE,
                            candidate.price,
                            candidate.exposure_per_share.clone(),
                        )
                    } else {
                        let shares = Self::cap_fractional_shares_to_next_bend(
                            candidate,
                            cash,
                            values,
                            categories,
                            total_value,
                            &profile.rebalance_goal,
                            drift_band,
                        );
                        if shares <= Decimal::ZERO {
                            continue;
                        }
                        (
                            shares,
                            candidate.price * shares,
                            Self::exposure_delta(&candidate.exposure_per_share, shares),
                        )
                    };

                let drift_after = Self::total_drift_with_buy(
                    values,
                    categories,
                    total_value,
                    &exposure_to_score,
                    &profile.rebalance_goal,
                    drift_band,
                );
                let improvement = drift_before - drift_after;
                if improvement <= Decimal::ZERO {
                    continue;
                }
                if profile.whole_shares_only {
                    improving_whole_share_candidates += 1;
                }
                let score = improvement / amount_to_score;
                if score > best_score {
                    best_score = score;
                    best_idx = Some(idx);
                    best_fractional_shares = shares_to_score;
                }
            }

            let Some(idx) = best_idx else { break };
            let candidate = &candidates[idx];

            if !profile.whole_shares_only {
                for (cat_id, expo) in &candidate.exposure_per_share {
                    *values.entry(cat_id.clone()).or_default() += expo * best_fractional_shares;
                }
                cash -= candidate.price * best_fractional_shares;
                shares_bought[idx] += best_fractional_shares;
                continue;
            }

            let mut batch = Decimal::ONE;
            if improving_whole_share_candidates == 1 {
                batch = (cash / candidate.price).floor().max(Decimal::ONE);
                for cat in categories.iter().filter(|c| c.is_required && !c.is_cash) {
                    let Some(expo) = candidate.exposure_per_share.get(&cat.category_id) else {
                        continue;
                    };
                    if *expo <= Decimal::ZERO {
                        continue;
                    }
                    let desired_bps = Self::desired_bps_for_goal(
                        cat.target_bps,
                        &profile.rebalance_goal,
                        drift_band,
                    );
                    let desired_value = desired_bps / scale * total_value;
                    let base = values.get(&cat.category_id).copied().unwrap_or_default();
                    if base < desired_value {
                        let cap = ((desired_value - base) / expo).floor().max(Decimal::ONE);
                        if cap < batch {
                            batch = cap;
                        }
                    }
                }
            }

            for (cat_id, expo) in &candidate.exposure_per_share {
                *values.entry(cat_id.clone()).or_default() += expo * batch;
            }
            cash -= candidate.price * batch;
            shares_bought[idx] += batch;
        }

        shares_bought
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
            scenario_mode,
            available_cash,
            total_value,
            categories,
            mut candidates,
            sell_candidates,
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

        // ── Sell phase (SellToRebalance / Hybrid) ────────────────────────────
        //
        // SellToRebalance: always sells overweight, buy pool = sell proceeds only
        //   (available_cash is not used for buys; it stays in the account).
        //
        // Hybrid: uses available cash first. Only sells if at least one required
        //   category is currently overweight outside its band — cash buys cannot
        //   reduce an overweight, so sells are necessary. Buy pool = cash + proceeds.
        //
        // CashFlowOnly: no sells, buy pool = available_cash.

        // Sell phase runs here only for SellToRebalance.
        // Hybrid defers its sell phase to after the cash buy pass (two-pass below).
        let (mut sell_trades, mut sell_proceeds) = match &scenario_mode {
            ScenarioMode::SellToRebalance => {
                let (updated_values, proceeds, trades) = Self::run_sell_phase(
                    &values,
                    total_value,
                    &categories,
                    &sell_candidates,
                    &profile.rebalance_goal,
                    drift_band,
                    profile.whole_shares_only,
                );
                values = updated_values;
                (trades, proceeds)
            }
            _ => (vec![], Decimal::ZERO),
        };

        // Buy pool:
        //   SellToRebalance → sell proceeds only (available_cash untouched).
        //   Hybrid          → available_cash only for pass 1; proceeds added in pass 2.
        //   CashFlowOnly    → available_cash only.
        let buy_pool = match &scenario_mode {
            ScenarioMode::SellToRebalance => sell_proceeds,
            _ => available_cash,
        };

        // ── Emit NoBuyCandidate for required underweight categories with no candidate coverage.
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

        // ── Buy phase(s) via run_buy_greedy ──────────────────────────────────
        //
        // CashFlowOnly / SellToRebalance: single pass with buy_pool.
        //
        // Hybrid (two-pass):
        //   Pass 1 — deploy available_cash first.
        //   Pass 2 — if overweight categories remain after cash buys, run sell
        //             phase on the post-buy state, then deploy proceeds.
        //   This implements "use cash first, sell only what cash cannot fix."

        let shares_bought: Vec<Decimal> = match &scenario_mode {
            ScenarioMode::Hybrid => {
                // Pass 1: buy with available_cash only.
                let mut sb = Self::run_buy_greedy(
                    &mut values,
                    &candidates,
                    available_cash,
                    &categories,
                    &profile,
                    drift_band,
                    total_value,
                    scale,
                );

                // Check if any required category is still overweight outside band
                // in the post-cash-buy state. Cash cannot reduce overweight (only
                // selling can), so this check is authoritative.
                let still_overweight = categories
                    .iter()
                    .filter(|c| c.is_required && !c.is_cash)
                    .any(|c| {
                        if total_value == Decimal::ZERO {
                            return false;
                        }
                        let v = values.get(&c.category_id).copied().unwrap_or_default();
                        let bps = v / total_value * scale;
                        let threshold = match profile.rebalance_goal {
                            RebalanceGoal::ExactTarget => Decimal::from(c.target_bps),
                            RebalanceGoal::NearestBand => {
                                (Decimal::from(c.target_bps) + drift_band).min(dec!(10000))
                            }
                        };
                        bps > threshold
                    });

                if still_overweight && !sell_candidates.is_empty() {
                    // Pass 2a: sell overweight on the post-cash-buy values.
                    let (updated_values, proceeds, extra_sell_trades) = Self::run_sell_phase(
                        &values,
                        total_value,
                        &categories,
                        &sell_candidates,
                        &profile.rebalance_goal,
                        drift_band,
                        profile.whole_shares_only,
                    );
                    values = updated_values;
                    // Merge sell trades into sell_trades (already empty for Hybrid first pass).
                    // We reassign the outer sell_trades/sell_proceeds below by reconstructing.
                    // Pass 2b: buy with proceeds from the hybrid sell.
                    let sb2 = Self::run_buy_greedy(
                        &mut values,
                        &candidates,
                        proceeds,
                        &categories,
                        &profile,
                        drift_band,
                        total_value,
                        scale,
                    );
                    // Accumulate shares and merge sell trades.
                    for (i, s) in sb2.into_iter().enumerate() {
                        sb[i] += s;
                    }
                    // Prepend the hybrid sell trades (before buy trades at output).
                    sell_trades = extra_sell_trades;
                    sell_proceeds = proceeds;
                }

                sb
            }
            _ => Self::run_buy_greedy(
                &mut values,
                &candidates,
                buy_pool,
                &categories,
                &profile,
                drift_band,
                total_value,
                scale,
            ),
        };

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
        // Draw from cash left after kept asset trades (including sell proceeds).
        // Greedy selections below min-trade threshold are dropped, so they must not
        // starve manual sleeve trades.
        let mut manual_cash = buy_pool - trades.iter().map(|t| t.estimated_amount).sum::<Decimal>();
        for cat in &no_candidate_categories {
            if manual_cash <= Decimal::ZERO {
                break;
            }
            let desired_bps =
                Self::desired_bps_for_goal(cat.target_bps, &profile.rebalance_goal, drift_band);
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

        // Prepend sell trades so the final list is: sells then buys.
        let mut all_trades: Vec<SuggestedManualTrade> = sell_trades;
        all_trades.append(&mut trades);
        let trades = all_trades;

        // cash_used = sum of buy trade amounts (post min_trade filter).
        // cash_remaining = original cash + sell proceeds - cash deployed on buys.
        let buy_cash_used: Decimal = trades
            .iter()
            .filter(|t| t.action == "buy")
            .map(|t| t.estimated_amount)
            .sum();
        let cash_used = buy_cash_used;
        let cash_remaining = (available_cash + sell_proceeds - cash_used).max(Decimal::ZERO);

        // After-drift: recompute from initial state + all recommended trades.
        let mut after_values: HashMap<String, Decimal> = categories
            .iter()
            .map(|c| (c.category_id.clone(), c.current_value))
            .collect();
        for trade in &trades {
            let shares = trade.quantity.unwrap_or(Decimal::ZERO);
            if trade.action == "sell" {
                if let Some(asset_id) = &trade.asset_id {
                    if let Some(sc) = sell_candidates.iter().find(|c| &c.asset_id == asset_id) {
                        for (cat_id, expo) in &sc.exposure_per_share {
                            let entry = after_values.entry(cat_id.clone()).or_default();
                            *entry = (*entry - expo * shares).max(Decimal::ZERO);
                        }
                    }
                }
            } else if let Some(asset_id) = &trade.asset_id {
                if let Some(candidate) = candidates.iter().find(|c| &c.asset_id == asset_id) {
                    for (cat_id, expo) in &candidate.exposure_per_share {
                        *after_values.entry(cat_id.clone()).or_default() += expo * shares;
                    }
                }
            } else {
                // Manual sleeve trade: deployed cash lands in target category.
                *after_values.entry(trade.category_id.clone()).or_default() +=
                    trade.estimated_amount;
            }
        }
        // Update cash sleeve: reduce by net cash deployed (buys - sell proceeds).
        let net_cash_change = cash_used - sell_proceeds;
        for cat in categories.iter().filter(|c| c.is_cash) {
            let entry = after_values.entry(cat.category_id.clone()).or_default();
            *entry = (*entry - net_cash_change).max(Decimal::ZERO);
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
