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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Result as CoreResult;
    use crate::portfolio::allocation::{
        AllocationHoldings, CategoryAllocation, PortfolioAllocations, TaxonomyAllocation,
    };
    use crate::portfolio::allocation_targets::model::{
        NewTargetAllocationNode, NewTargetProfile, ProfileStatus, RebalanceTo, ScopeType,
        TargetAllocationNode, TargetProfile, TriggerType,
    };
    use async_trait::async_trait;
    use rust_decimal_macros::dec;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn base_profile(drift_band_bps: i32) -> TargetProfile {
        TargetProfile {
            id: "p1".to_string(),
            name: "Test".to_string(),
            status: ProfileStatus::Active,
            scope_type: ScopeType::All,
            scope_id: None,
            taxonomy_id: "asset_classes".to_string(),
            base_currency: "USD".to_string(),
            trigger_type: TriggerType::Threshold,
            drift_band_bps,
            review_frequency: None,
            next_review_date: None,
            rebalance_to: RebalanceTo::NearestBand,
            allow_sells: false,
            min_trade_amount: "0".to_string(),
            whole_shares_only: false,
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
        }
    }

    fn node(category_id: &str, target_bps: i32) -> TargetAllocationNode {
        TargetAllocationNode {
            id: uuid::Uuid::new_v4().to_string(),
            profile_id: "p1".to_string(),
            category_id: category_id.to_string(),
            target_bps,
            is_locked: false,
            is_required: true,
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
        }
    }

    fn cat(category_id: &str, value: rust_decimal::Decimal) -> CategoryAllocation {
        CategoryAllocation {
            category_id: category_id.to_string(),
            category_name: category_id.to_string(),
            color: "#000000".to_string(),
            value,
            percentage: rust_decimal::Decimal::ZERO,
            children: vec![],
        }
    }

    fn alloc_with(
        categories: Vec<CategoryAllocation>,
        total: rust_decimal::Decimal,
    ) -> PortfolioAllocations {
        let mut pa = PortfolioAllocations::default();
        pa.asset_classes = TaxonomyAllocation {
            taxonomy_id: "asset_classes".to_string(),
            taxonomy_name: "Asset Classes".to_string(),
            color: "#000000".to_string(),
            categories,
        };
        pa.total_value = total;
        pa
    }

    // ── Mocks ────────────────────────────────────────────────────────────────

    struct MockTargetService {
        profile: TargetProfile,
        nodes: Vec<TargetAllocationNode>,
    }

    #[async_trait]
    impl TargetProfileServiceTrait for MockTargetService {
        fn get_profile(&self, _id: &str) -> CoreResult<Option<TargetProfile>> {
            Ok(Some(self.profile.clone()))
        }
        fn list_profiles(&self) -> CoreResult<Vec<TargetProfile>> {
            Ok(vec![self.profile.clone()])
        }
        fn get_active_profile_for_scope(
            &self,
            _scope_type: &str,
            _scope_id: Option<&str>,
        ) -> CoreResult<Option<TargetProfile>> {
            Ok(Some(self.profile.clone()))
        }
        fn list_nodes_for_profile(
            &self,
            _profile_id: &str,
        ) -> CoreResult<Vec<TargetAllocationNode>> {
            Ok(self.nodes.clone())
        }
        async fn create_profile(&self, _input: NewTargetProfile) -> CoreResult<TargetProfile> {
            unimplemented!()
        }
        async fn update_profile(
            &self,
            _id: &str,
            _input: NewTargetProfile,
        ) -> CoreResult<TargetProfile> {
            unimplemented!()
        }
        async fn activate_profile(&self, _id: &str) -> CoreResult<TargetProfile> {
            unimplemented!()
        }
        async fn archive_profile(&self, _id: &str) -> CoreResult<TargetProfile> {
            unimplemented!()
        }
        async fn delete_profile(&self, _id: &str) -> CoreResult<()> {
            unimplemented!()
        }
        async fn save_nodes(
            &self,
            _profile_id: &str,
            _nodes: Vec<NewTargetAllocationNode>,
        ) -> CoreResult<Vec<TargetAllocationNode>> {
            unimplemented!()
        }
    }

    struct MockAllocationService(PortfolioAllocations);

    #[async_trait]
    impl crate::portfolio::allocation::AllocationServiceTrait for MockAllocationService {
        async fn get_portfolio_allocations(
            &self,
            _account_id: &str,
            _base_currency: &str,
        ) -> CoreResult<PortfolioAllocations> {
            Ok(self.0.clone())
        }
        async fn get_portfolio_allocations_for_accounts(
            &self,
            _account_ids: &[String],
            _base_currency: &str,
            _aggregated_account_id: &str,
        ) -> CoreResult<PortfolioAllocations> {
            Ok(self.0.clone())
        }
        async fn get_holdings_by_allocation(
            &self,
            _account_id: &str,
            _base_currency: &str,
            _taxonomy_id: &str,
            _category_id: &str,
        ) -> CoreResult<AllocationHoldings> {
            unimplemented!()
        }
        async fn get_holdings_by_allocation_for_accounts(
            &self,
            _account_ids: &[String],
            _base_currency: &str,
            _taxonomy_id: &str,
            _category_id: &str,
            _aggregated_account_id: &str,
        ) -> CoreResult<AllocationHoldings> {
            unimplemented!()
        }
    }

    fn make_service(
        profile: TargetProfile,
        nodes: Vec<TargetAllocationNode>,
        allocations: PortfolioAllocations,
    ) -> DriftService {
        DriftService::new(
            Arc::new(MockTargetService { profile, nodes }),
            Arc::new(MockAllocationService(allocations)),
        )
    }

    // ── Tests ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn overweight_detected() {
        // EQUITY current=70% (7000 bps), target=60% (6000 bps), band=500 → drift=+1000 → Overweight
        let svc = make_service(
            base_profile(500),
            vec![node("EQUITY", 6000), node("BONDS", 4000)],
            alloc_with(
                vec![cat("EQUITY", dec!(7000)), cat("BONDS", dec!(3000))],
                dec!(10000),
            ),
        );
        let report = svc
            .get_drift_report_for_profile("p1", &[], "USD", "agg")
            .await
            .unwrap();

        let equity = report
            .rows
            .iter()
            .find(|r| r.category_id == "EQUITY")
            .unwrap();
        assert_eq!(equity.current_bps, 7000);
        assert_eq!(equity.target_bps, 6000);
        assert_eq!(equity.drift_bps, 1000);
        assert_eq!(equity.status, DriftStatus::Overweight);
    }

    #[tokio::test]
    async fn underweight_detected() {
        // BONDS current=30% (3000 bps), target=40% (4000 bps), band=500 → drift=-1000 → Underweight
        let svc = make_service(
            base_profile(500),
            vec![node("EQUITY", 6000), node("BONDS", 4000)],
            alloc_with(
                vec![cat("EQUITY", dec!(7000)), cat("BONDS", dec!(3000))],
                dec!(10000),
            ),
        );
        let report = svc
            .get_drift_report_for_profile("p1", &[], "USD", "agg")
            .await
            .unwrap();

        let bonds = report
            .rows
            .iter()
            .find(|r| r.category_id == "BONDS")
            .unwrap();
        assert_eq!(bonds.drift_bps, -1000);
        assert_eq!(bonds.status, DriftStatus::Underweight);
    }

    #[tokio::test]
    async fn in_band_detected() {
        // EQUITY current=61% (6100 bps), target=60% (6000 bps), band=500 → drift=+100 → InBand
        let svc = make_service(
            base_profile(500),
            vec![node("EQUITY", 6000), node("BONDS", 4000)],
            alloc_with(
                vec![cat("EQUITY", dec!(6100)), cat("BONDS", dec!(3900))],
                dec!(10000),
            ),
        );
        let report = svc
            .get_drift_report_for_profile("p1", &[], "USD", "agg")
            .await
            .unwrap();

        let equity = report
            .rows
            .iter()
            .find(|r| r.category_id == "EQUITY")
            .unwrap();
        assert_eq!(equity.drift_bps, 100);
        assert_eq!(equity.status, DriftStatus::InBand);
    }

    #[tokio::test]
    async fn zero_current_marks_is_zero_current_and_underweight() {
        // Node for BONDS but no current allocation → is_zero_current=true, Underweight
        let svc = make_service(
            base_profile(500),
            vec![node("EQUITY", 6000), node("BONDS", 4000)],
            alloc_with(
                vec![cat("EQUITY", dec!(10000))], // no BONDS position
                dec!(10000),
            ),
        );
        let report = svc
            .get_drift_report_for_profile("p1", &[], "USD", "agg")
            .await
            .unwrap();

        let bonds = report
            .rows
            .iter()
            .find(|r| r.category_id == "BONDS")
            .unwrap();
        assert!(bonds.is_zero_current);
        assert_eq!(bonds.current_bps, 0);
        assert_eq!(bonds.drift_bps, -4000);
        assert_eq!(bonds.status, DriftStatus::Underweight);
    }

    #[tokio::test]
    async fn not_targeted_category_appended() {
        // CASH in alloc but not in nodes → NotTargeted row
        let svc = make_service(
            base_profile(500),
            vec![node("EQUITY", 10000)],
            alloc_with(
                vec![cat("EQUITY", dec!(8000)), cat("CASH", dec!(2000))],
                dec!(10000),
            ),
        );
        let report = svc
            .get_drift_report_for_profile("p1", &[], "USD", "agg")
            .await
            .unwrap();

        let cash = report
            .rows
            .iter()
            .find(|r| r.category_id == "CASH")
            .unwrap();
        assert_eq!(cash.status, DriftStatus::NotTargeted);
        assert_eq!(cash.target_bps, 0);
        assert_eq!(cash.current_bps, 2000);
        assert_eq!(cash.drift_bps, 2000);
    }

    #[tokio::test]
    async fn max_drift_bps_from_required_rows() {
        // EQUITY drift=+1000, BONDS drift=-1000 → max_drift_bps=1000
        let svc = make_service(
            base_profile(500),
            vec![node("EQUITY", 6000), node("BONDS", 4000)],
            alloc_with(
                vec![cat("EQUITY", dec!(7000)), cat("BONDS", dec!(3000))],
                dec!(10000),
            ),
        );
        let report = svc
            .get_drift_report_for_profile("p1", &[], "USD", "agg")
            .await
            .unwrap();

        assert_eq!(report.max_drift_bps, 1000);
    }

    #[tokio::test]
    async fn out_of_band_count_correct() {
        // Both EQUITY and BONDS out of band → count=2
        let svc = make_service(
            base_profile(500),
            vec![node("EQUITY", 6000), node("BONDS", 4000)],
            alloc_with(
                vec![cat("EQUITY", dec!(7000)), cat("BONDS", dec!(3000))],
                dec!(10000),
            ),
        );
        let report = svc
            .get_drift_report_for_profile("p1", &[], "USD", "agg")
            .await
            .unwrap();

        assert_eq!(report.out_of_band_count, 2);
    }

    #[tokio::test]
    async fn total_value_zero_all_bps_zero() {
        // Empty portfolio → all current_bps = 0, no drift
        let svc = make_service(
            base_profile(500),
            vec![node("EQUITY", 6000), node("BONDS", 4000)],
            alloc_with(vec![], dec!(0)),
        );
        let report = svc
            .get_drift_report_for_profile("p1", &[], "USD", "agg")
            .await
            .unwrap();

        for row in &report.rows {
            assert_eq!(row.current_bps, 0);
        }
        assert_eq!(report.total_value, dec!(0));
    }

    #[tokio::test]
    async fn value_delta_correct() {
        // EQUITY: current=$7000, target=60%*$10000=$6000 → delta=+$1000
        let svc = make_service(
            base_profile(500),
            vec![node("EQUITY", 6000), node("BONDS", 4000)],
            alloc_with(
                vec![cat("EQUITY", dec!(7000)), cat("BONDS", dec!(3000))],
                dec!(10000),
            ),
        );
        let report = svc
            .get_drift_report_for_profile("p1", &[], "USD", "agg")
            .await
            .unwrap();

        let equity = report
            .rows
            .iter()
            .find(|r| r.category_id == "EQUITY")
            .unwrap();
        assert_eq!(equity.current_value, dec!(7000));
        assert_eq!(equity.target_value, dec!(6000));
        assert_eq!(equity.value_delta, dec!(1000));
    }
}
