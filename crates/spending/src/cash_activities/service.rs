use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use rust_decimal::prelude::ToPrimitive;
use wealthfolio_core::activities::{Activity, ActivityRepositoryTrait};

use super::{
    model::{
        CashActivityFilter, CashActivitySearchRequest, CashActivitySearchResponse,
        CashActivitySortField, CashActivityStatusFilter, CashActivityWithAssignments,
        SortDirection,
    },
    CASH_ACTIVITY_TYPES,
};
use crate::activity_assignments::{ActivityTaxonomyAssignment, ActivityTaxonomyAssignmentService};
use crate::settings::SpendingSettingsService;

/// Service for listing/searching cash activities scoped to the user's spending accounts.
/// Mutation (create/update/delete) goes through the existing core ActivityService;
/// categorization goes through ActivityTaxonomyAssignmentService.
pub struct CashActivityService {
    activity_repo: Arc<dyn ActivityRepositoryTrait>,
    settings: Arc<SpendingSettingsService>,
    assignments: Arc<ActivityTaxonomyAssignmentService>,
}

impl CashActivityService {
    pub fn new(
        activity_repo: Arc<dyn ActivityRepositoryTrait>,
        settings: Arc<SpendingSettingsService>,
        assignments: Arc<ActivityTaxonomyAssignmentService>,
    ) -> Self {
        Self {
            activity_repo,
            settings,
            assignments,
        }
    }

    /// List cash activities matching the (legacy) filter, scoped to opted-in spending accounts.
    /// Returns empty vec if spending tracking is disabled or no accounts opted in.
    /// Kept for backward compatibility with the legacy `list_cash_activities` endpoint.
    pub async fn list(&self, filter: CashActivityFilter) -> Result<Vec<Activity>> {
        let s = self.settings.get().await?;
        if !s.enabled || s.account_ids.is_empty() {
            return Ok(Vec::new());
        }

        let target_accounts = self.resolve_target_accounts(filter.account_ids, &s.account_ids);
        if target_accounts.is_empty() {
            return Ok(Vec::new());
        }

        let mut activities = self
            .activity_repo
            .get_activities_by_account_ids(&target_accounts)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let allowed_types: Vec<String> = filter
            .activity_types
            .unwrap_or_else(|| CASH_ACTIVITY_TYPES.iter().map(|s| s.to_string()).collect());
        activities.retain(|a| allowed_types.iter().any(|t| t == a.effective_type()));

        if let Some(start) = filter.start_date.as_deref() {
            activities.retain(|a| a.activity_date.to_rfc3339().as_str() >= start);
        }
        if let Some(end) = filter.end_date.as_deref() {
            activities.retain(|a| a.activity_date.to_rfc3339().as_str() <= end);
        }

        activities.sort_by(|a, b| b.activity_date.cmp(&a.activity_date));
        Ok(activities)
    }

    /// Search/filter/paginate cash activities. Powers the spending Transactions page.
    /// Server-side pipeline: filters → sort → paginate → join assignments for the page slice.
    pub async fn search(
        &self,
        req: CashActivitySearchRequest,
    ) -> Result<CashActivitySearchResponse> {
        let s = self.settings.get().await?;
        if !s.enabled || s.account_ids.is_empty() {
            return Ok(CashActivitySearchResponse {
                items: Vec::new(),
                total_count: 0,
            });
        }

        let target_accounts = self.resolve_target_accounts(req.account_ids, &s.account_ids);
        if target_accounts.is_empty() {
            return Ok(CashActivitySearchResponse {
                items: Vec::new(),
                total_count: 0,
            });
        }

        let mut activities = self
            .activity_repo
            .get_activities_by_account_ids(&target_accounts)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let allowed_types: Vec<String> = req
            .activity_types
            .unwrap_or_else(|| CASH_ACTIVITY_TYPES.iter().map(|s| s.to_string()).collect());
        activities.retain(|a| allowed_types.iter().any(|t| t == a.effective_type()));

        if let Some(start) = req.start_date.as_deref() {
            activities.retain(|a| a.activity_date.to_rfc3339().as_str() >= start);
        }
        if let Some(end) = req.end_date.as_deref() {
            activities.retain(|a| a.activity_date.to_rfc3339().as_str() <= end);
        }

        if let Some(events) = req.event_ids.as_deref() {
            if !events.is_empty() {
                activities.retain(|a| {
                    a.event_id
                        .as_deref()
                        .map(|id| events.iter().any(|e| e == id))
                        .unwrap_or(false)
                });
            }
        }

        if let Some(min) = req.min_amount {
            activities.retain(|a| {
                a.amount
                    .map(|d| d.abs().to_f64().unwrap_or(0.0) >= min)
                    .unwrap_or(false)
            });
        }
        if let Some(max) = req.max_amount {
            activities.retain(|a| {
                a.amount
                    .map(|d| d.abs().to_f64().unwrap_or(0.0) <= max)
                    .unwrap_or(false)
            });
        }

        if let Some(needle) = req.search.as_deref() {
            let needle = needle.trim().to_lowercase();
            if !needle.is_empty() {
                activities.retain(|a| {
                    let notes = a.notes.as_deref().unwrap_or("").to_lowercase();
                    notes.contains(&needle)
                });
            }
        }

        // Status / category filters need assignments; fetch in batch first.
        let needs_assignments_for_filter = req.status != CashActivityStatusFilter::All
            || req
                .category_ids
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false)
            || req
                .subcategory_ids
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false);

        if needs_assignments_for_filter {
            let ids: Vec<String> = activities.iter().map(|a| a.id.clone()).collect();
            let assignments = self.assignments.list_for_activities(&ids).await?;
            let by_activity = group_assignments(&assignments);

            activities.retain(|a| {
                let asgs = by_activity.get(a.id.as_str());
                let has_category = asgs.map(|v| !v.is_empty()).unwrap_or(false);

                match req.status {
                    CashActivityStatusFilter::All => {}
                    CashActivityStatusFilter::NeedsReview => {
                        if !a.needs_review {
                            return false;
                        }
                    }
                    CashActivityStatusFilter::Uncategorized => {
                        if has_category {
                            return false;
                        }
                    }
                    CashActivityStatusFilter::Categorized => {
                        if !has_category {
                            return false;
                        }
                    }
                }

                if let Some(cats) = req.category_ids.as_deref() {
                    if !cats.is_empty() {
                        let any = asgs
                            .map(|v| {
                                v.iter()
                                    .any(|asg| cats.iter().any(|c| c == &asg.category_id))
                            })
                            .unwrap_or(false);
                        if !any {
                            return false;
                        }
                    }
                }
                if let Some(subs) = req.subcategory_ids.as_deref() {
                    if !subs.is_empty() {
                        let any = asgs
                            .map(|v| {
                                v.iter()
                                    .any(|asg| subs.iter().any(|c| c == &asg.category_id))
                            })
                            .unwrap_or(false);
                        if !any {
                            return false;
                        }
                    }
                }

                true
            });
        }

        // Sort
        match req.sort_by {
            CashActivitySortField::Date => match req.sort_dir {
                SortDirection::Desc => {
                    activities.sort_by(|a, b| b.activity_date.cmp(&a.activity_date))
                }
                SortDirection::Asc => {
                    activities.sort_by(|a, b| a.activity_date.cmp(&b.activity_date))
                }
            },
            CashActivitySortField::Amount => {
                activities.sort_by(|a, b| {
                    let av = a.amount.map(|d| d.abs()).unwrap_or_default();
                    let bv = b.amount.map(|d| d.abs()).unwrap_or_default();
                    match req.sort_dir {
                        SortDirection::Desc => bv.cmp(&av),
                        SortDirection::Asc => av.cmp(&bv),
                    }
                });
            }
        }

        let total_count = activities.len();

        // Paginate
        let offset = req.offset.min(total_count);
        let end = offset.saturating_add(req.limit).min(total_count);
        let page: Vec<Activity> = activities.drain(offset..end).collect();
        // Drop the rest — we no longer need them.
        drop(activities);

        // Batch-fetch assignments for the paginated slice (always — clients use them for display).
        let page_ids: Vec<String> = page.iter().map(|a| a.id.clone()).collect();
        let asgs = self.assignments.list_for_activities(&page_ids).await?;
        let mut by_activity = group_assignments_owned(asgs);

        let items: Vec<CashActivityWithAssignments> = page
            .into_iter()
            .map(|a| {
                let assignments = by_activity.remove(&a.id).unwrap_or_default();
                CashActivityWithAssignments {
                    activity: a,
                    assignments,
                }
            })
            .collect();

        Ok(CashActivitySearchResponse { items, total_count })
    }

    /// Set or clear the `event_id` on an activity. Pass `None` to clear.
    /// No-op-friendly: returns the updated activity.
    pub async fn set_event(&self, activity_id: &str, event_id: Option<String>) -> Result<Activity> {
        self.activity_repo
            .set_activity_event_id(activity_id, event_id)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    fn resolve_target_accounts(
        &self,
        requested: Option<Vec<String>>,
        opted_in: &[String],
    ) -> Vec<String> {
        match requested {
            Some(ids) => ids.into_iter().filter(|id| opted_in.contains(id)).collect(),
            None => opted_in.to_vec(),
        }
    }
}

fn group_assignments<'a>(
    assignments: &'a [ActivityTaxonomyAssignment],
) -> HashMap<&'a str, Vec<&'a ActivityTaxonomyAssignment>> {
    let mut map: HashMap<&str, Vec<&ActivityTaxonomyAssignment>> = HashMap::new();
    for a in assignments {
        map.entry(a.activity_id.as_str()).or_default().push(a);
    }
    map
}

fn group_assignments_owned(
    assignments: Vec<ActivityTaxonomyAssignment>,
) -> HashMap<String, Vec<ActivityTaxonomyAssignment>> {
    let mut map: HashMap<String, Vec<ActivityTaxonomyAssignment>> = HashMap::new();
    for a in assignments {
        map.entry(a.activity_id.clone()).or_default().push(a);
    }
    map
}
