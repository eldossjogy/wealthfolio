use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;

use crate::errors::Result as CoreResult;
use crate::portfolio::allocation::AllocationServiceTrait;

use super::model::{DriftReport, DriftRow, DriftStatus, ScopeType};
use super::target_service::TargetProfileServiceTrait;

#[async_trait]
pub trait DriftServiceTrait: Send + Sync {
    /// Compute the drift report for the active profile on a given scope.
    async fn get_drift_report(
        &self,
        scope_type: &str,
        scope_id: Option<&str>,
        account_ids: &[String],
        base_currency: &str,
        aggregated_account_id: &str,
    ) -> CoreResult<Option<DriftReport>>;

    /// Compute the drift report for an explicit profile_id (e.g., draft preview).
    async fn get_drift_report_for_profile(
        &self,
        profile_id: &str,
        account_ids: &[String],
        base_currency: &str,
        aggregated_account_id: &str,
    ) -> CoreResult<DriftReport>;
}

pub struct DriftService {
    target_service: Arc<dyn TargetProfileServiceTrait>,
    allocation_service: Arc<dyn AllocationServiceTrait>,
}

impl DriftService {
    pub fn new(
        target_service: Arc<dyn TargetProfileServiceTrait>,
        allocation_service: Arc<dyn AllocationServiceTrait>,
    ) -> Self {
        Self {
            target_service,
            allocation_service,
        }
    }
}

#[async_trait]
impl DriftServiceTrait for DriftService {
    async fn get_drift_report(
        &self,
        scope_type: &str,
        scope_id: Option<&str>,
        account_ids: &[String],
        base_currency: &str,
        aggregated_account_id: &str,
    ) -> CoreResult<Option<DriftReport>> {
        let profile = self
            .target_service
            .get_active_profile_for_scope(scope_type, scope_id)?;

        let Some(profile) = profile else {
            return Ok(None);
        };

        let report = self
            .get_drift_report_for_profile(
                &profile.id.clone(),
                account_ids,
                base_currency,
                aggregated_account_id,
            )
            .await?;

        Ok(Some(report))
    }

    async fn get_drift_report_for_profile(
        &self,
        profile_id: &str,
        account_ids: &[String],
        base_currency: &str,
        aggregated_account_id: &str,
    ) -> CoreResult<DriftReport> {
        let profile = self
            .target_service
            .get_profile(profile_id)?
            .ok_or_else(|| {
                crate::errors::Error::Database(crate::errors::DatabaseError::NotFound(format!(
                    "TargetProfile {} not found",
                    profile_id
                )))
            })?;

        let nodes = self.target_service.list_nodes_for_profile(profile_id)?;

        // Load current allocations for the scope
        let allocations = self
            .allocation_service
            .get_portfolio_allocations_for_accounts(
                account_ids,
                base_currency,
                aggregated_account_id,
            )
            .await?;

        let total_value = allocations.total_value;

        // Build a map: category_id -> (value, percentage, name, color)
        // Use the taxonomy matching the profile's taxonomy_id
        let taxonomy_alloc = match profile.taxonomy_id.as_str() {
            "asset_classes" => &allocations.asset_classes,
            "sectors" => &allocations.sectors,
            "regions" => &allocations.regions,
            "risk_category" => &allocations.risk_category,
            "security_types" => &allocations.security_types,
            other => allocations
                .custom_groups
                .iter()
                .find(|g| g.taxonomy_id == other)
                .unwrap_or(&allocations.asset_classes),
        };

        let current_by_cat: std::collections::HashMap<
            &str,
            &crate::portfolio::allocation::CategoryAllocation,
        > = taxonomy_alloc
            .categories
            .iter()
            .map(|c| (c.category_id.as_str(), c))
            .collect();

        let _hundred = dec!(100);
        let bps_scale = dec!(10000);

        let mut rows: Vec<DriftRow> = nodes
            .iter()
            .map(|node| {
                let current = current_by_cat.get(node.category_id.as_str());
                let current_value = current.map(|c| c.value).unwrap_or(Decimal::ZERO);
                let category_name = current
                    .map(|c| c.category_name.clone())
                    .unwrap_or_else(|| node.category_id.clone());
                let color = current
                    .map(|c| c.color.clone())
                    .unwrap_or_else(|| "#94a3b8".to_string());

                // bps math
                let current_bps = if total_value > Decimal::ZERO {
                    ((current_value / total_value) * bps_scale)
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)
                } else {
                    0
                };
                let target_bps = node.target_bps;
                let drift_bps = current_bps - target_bps;

                let target_value = if total_value > Decimal::ZERO {
                    total_value * Decimal::from(target_bps) / bps_scale
                } else {
                    Decimal::ZERO
                };
                let value_delta = current_value - target_value;

                let drift_band = profile.drift_band_bps;
                let status = if drift_bps.abs() <= drift_band {
                    DriftStatus::InBand
                } else if drift_bps < 0 {
                    DriftStatus::Underweight
                } else {
                    DriftStatus::Overweight
                };

                DriftRow {
                    category_id: node.category_id.clone(),
                    category_name,
                    color,
                    current_bps,
                    target_bps,
                    drift_bps,
                    current_value,
                    target_value,
                    value_delta,
                    status,
                    is_required: node.is_required,
                    is_zero_current: current_value == Decimal::ZERO,
                }
            })
            .collect();

        // Add NotTargeted rows for categories present in current allocation but not in nodes
        let targeted_ids: std::collections::HashSet<&str> =
            nodes.iter().map(|n| n.category_id.as_str()).collect();

        for cat in &taxonomy_alloc.categories {
            if !targeted_ids.contains(cat.category_id.as_str()) {
                let current_bps = if total_value > Decimal::ZERO {
                    ((cat.value / total_value) * bps_scale)
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)
                } else {
                    0
                };
                rows.push(DriftRow {
                    category_id: cat.category_id.clone(),
                    category_name: cat.category_name.clone(),
                    color: cat.color.clone(),
                    current_bps,
                    target_bps: 0,
                    drift_bps: current_bps,
                    current_value: cat.value,
                    target_value: Decimal::ZERO,
                    value_delta: cat.value,
                    status: DriftStatus::NotTargeted,
                    is_required: false,
                    is_zero_current: cat.value == Decimal::ZERO,
                });
            }
        }

        // Sort: required rows first by abs drift desc, then not-targeted
        rows.sort_by(|a, b| {
            let a_required = a.status != DriftStatus::NotTargeted;
            let b_required = b.status != DriftStatus::NotTargeted;
            b_required
                .cmp(&a_required)
                .then(b.drift_bps.unsigned_abs().cmp(&a.drift_bps.unsigned_abs()))
        });

        let max_drift_bps = rows
            .iter()
            .filter(|r| r.is_required)
            .map(|r| r.drift_bps.unsigned_abs() as i32)
            .max()
            .unwrap_or(0);

        let out_of_band_count = rows
            .iter()
            .filter(|r| {
                r.is_required
                    && matches!(r.status, DriftStatus::Underweight | DriftStatus::Overweight)
            })
            .count();

        let scope_type = ScopeType::try_from(profile.scope_type.as_str()).unwrap_or(ScopeType::All);

        Ok(DriftReport {
            profile_id: profile_id.to_string(),
            scope_type,
            scope_id: profile.scope_id,
            total_value,
            base_currency: base_currency.to_string(),
            max_drift_bps,
            out_of_band_count,
            rows,
        })
    }
}
