use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use wealthfolio_core::accounts::{
    account_supports_purpose, AccountPurpose, AccountRepositoryTrait,
};
use wealthfolio_core::activities::{Activity, ActivityRepositoryTrait};

use super::{
    model::{
        CashActivity, CashActivityFilter, CashActivitySearchRequest, CashActivitySearchResponse,
        CashActivitySortField, CashActivityStatusFilter, SortDirection,
    },
    CASH_ACTIVITY_TYPES,
};
use crate::activity_assignments::{ActivityTaxonomyAssignment, ActivityTaxonomyAssignmentService};
use crate::activity_classification::{classify_activity, SpendingClassification};
use crate::settings::SpendingSettingsService;

/// Service for listing/searching activities scoped to the user's spending accounts.
/// Mutation (create/update/delete) goes through the existing core ActivityService;
/// categorization goes through ActivityTaxonomyAssignmentService.
pub struct CashActivityService {
    activity_repo: Arc<dyn ActivityRepositoryTrait>,
    account_repo: Arc<dyn AccountRepositoryTrait>,
    settings: Arc<SpendingSettingsService>,
    assignments: Arc<ActivityTaxonomyAssignmentService>,
    activity_events: Arc<dyn crate::activity_events::ActivityEventsRepositoryTrait>,
}

impl CashActivityService {
    pub fn new(
        activity_repo: Arc<dyn ActivityRepositoryTrait>,
        account_repo: Arc<dyn AccountRepositoryTrait>,
        settings: Arc<SpendingSettingsService>,
        assignments: Arc<ActivityTaxonomyAssignmentService>,
        activity_events: Arc<dyn crate::activity_events::ActivityEventsRepositoryTrait>,
    ) -> Self {
        Self {
            activity_repo,
            account_repo,
            settings,
            assignments,
            activity_events,
        }
    }

    /// List cash activities matching the (legacy) filter, scoped to opted-in
    /// spending accounts. Returns empty vec if spending tracking is disabled
    /// or no accounts opted in.
    ///
    /// Returns `CashActivity` (same shape as `search()` items)
    /// so consumers get the activity row, its category assignments, and its
    /// event tag in a single round-trip. Before the activity_events
    /// refactor, `Activity` carried `event_id` directly; we now JOIN it in
    /// here so the frontend doesn't need a second query (and so a single
    /// regression on either path can't diverge from the other — `list()`
    /// previously missed the event-tag enrichment `search()` got).
    pub async fn list(&self, filter: CashActivityFilter) -> Result<Vec<CashActivity>> {
        let s = self.settings.get().await?;
        if !s.enabled || s.account_ids.is_empty() {
            return Ok(Vec::new());
        }

        let (target_accounts, account_types) =
            self.resolve_target_accounts(filter.account_ids, &s.account_ids)?;
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
        retain_classified_cash_activities(&mut activities, &account_types);

        retain_by_date_range(
            &mut activities,
            filter.start_date.as_deref(),
            filter.end_date.as_deref(),
        )?;

        activities.sort_by(|a, b| b.activity_date.cmp(&a.activity_date));

        // Batch-enrich with assignments + event tags. Mirrors the tail of
        // `search()`. The ids list is the *retained* rows, so we never fetch
        // joins for activities we've already filtered out.
        let ids: Vec<String> = activities.iter().map(|a| a.id.clone()).collect();
        let asgs = self.assignments.list_for_activities(&ids).await?;
        let mut by_activity = group_assignments_owned(asgs);
        let mut tag_map = self.activity_events.list_for_activities(&ids).await?;
        let items: Vec<CashActivity> = activities
            .into_iter()
            .map(|a| {
                let assignments = by_activity.remove(&a.id).unwrap_or_default();
                let event_id = tag_map.remove(&a.id);
                CashActivity {
                    activity: a,
                    assignments,
                    event_id,
                }
            })
            .collect();
        Ok(items)
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

        let (target_accounts, account_types) =
            self.resolve_target_accounts(req.account_ids, &s.account_ids)?;
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
        retain_classified_cash_activities(&mut activities, &account_types);

        retain_by_date_range(
            &mut activities,
            req.start_date.as_deref(),
            req.end_date.as_deref(),
        )?;

        if let Some(events) = req.event_ids.as_deref() {
            if !events.is_empty() {
                // Load per-activity tags from the join table once, then
                // filter in-memory. Mirrors the analytics services' pattern.
                let activity_ids: Vec<String> = activities.iter().map(|a| a.id.clone()).collect();
                let tag_map = self
                    .activity_events
                    .list_for_activities(&activity_ids)
                    .await?;
                activities.retain(|a| {
                    tag_map
                        .get(&a.id)
                        .map(|tag| events.iter().any(|e| e == tag))
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

        // Batch-fetch assignments + event tags for the paginated slice.
        // (Always — clients use both for display.)
        let page_ids: Vec<String> = page.iter().map(|a| a.id.clone()).collect();
        let asgs = self.assignments.list_for_activities(&page_ids).await?;
        let mut by_activity = group_assignments_owned(asgs);
        let mut tag_map = self.activity_events.list_for_activities(&page_ids).await?;

        let items: Vec<CashActivity> = page
            .into_iter()
            .map(|a| {
                let assignments = by_activity.remove(&a.id).unwrap_or_default();
                let event_id = tag_map.remove(&a.id);
                CashActivity {
                    activity: a,
                    assignments,
                    event_id,
                }
            })
            .collect();

        Ok(CashActivitySearchResponse { items, total_count })
    }

    /// Set or clear the spending-event tag on an activity. Pass `None` to clear.
    ///
    /// **Return contract**: returns the underlying `Activity` row, which does
    /// **not** carry the new tag — `event_id` lives on the `activity_events`
    /// join table, not on the activity row itself. Callers that need to read
    /// the post-write tag back must round-trip through `search()` / `list()`
    /// (which JOIN the tag in via `CashActivity`). The existing frontend
    /// caller (`useCashActivities`) discards this return value and refetches
    /// via the spending caches, which is the intended pattern.
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
    ) -> Result<(Vec<String>, HashMap<String, String>)> {
        let target_accounts: Vec<String> = match requested {
            Some(ids) => ids.into_iter().filter(|id| opted_in.contains(id)).collect(),
            None => opted_in.to_vec(),
        };
        if target_accounts.is_empty() {
            return Ok((target_accounts, HashMap::new()));
        }

        let accounts = self
            .account_repo
            .list(None, Some(false), Some(&target_accounts))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let account_types: HashMap<String, String> = accounts
            .into_iter()
            .filter(|account| {
                account_supports_purpose(&account.account_type, AccountPurpose::Spending)
            })
            .map(|account| (account.id, account.account_type))
            .collect();

        let target_accounts = target_accounts
            .into_iter()
            .filter(|id| account_types.contains_key(id))
            .collect();

        Ok((target_accounts, account_types))
    }
}

fn retain_classified_cash_activities(
    activities: &mut Vec<Activity>,
    account_types: &HashMap<String, String>,
) {
    activities.retain(|activity| {
        account_types
            .get(&activity.account_id)
            .map(|account_type| classify_activity(activity, account_type))
            .is_some_and(|classification| {
                matches!(
                    classification,
                    SpendingClassification::Income
                        | SpendingClassification::Expense
                        | SpendingClassification::ExpenseRefund
                )
            })
    });
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

fn retain_by_date_range(
    activities: &mut Vec<Activity>,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<()> {
    let start = parse_filter_datetime(start_date)?;
    let end = parse_filter_datetime(end_date)?;

    if start.is_some() || end.is_some() {
        activities
            .retain(|a| activity_date_in_range(&a.activity_date, start.as_ref(), end.as_ref()));
    }

    Ok(())
}

fn parse_filter_datetime(value: Option<&str>) -> Result<Option<DateTime<Utc>>> {
    value
        .map(|value| DateTime::parse_from_rfc3339(value).map(|date| date.with_timezone(&Utc)))
        .transpose()
        .map_err(Into::into)
}

fn activity_date_in_range(
    activity_date: &DateTime<Utc>,
    start: Option<&DateTime<Utc>>,
    end: Option<&DateTime<Utc>>,
) -> bool {
    start.is_none_or(|start| activity_date >= start) && end.is_none_or(|end| activity_date <= end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_date_filter_compares_instants_not_rfc3339_strings() {
        let activity_date = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let same_start = parse_filter_datetime(Some("2024-01-01T00:00:00.000Z"))
            .unwrap()
            .unwrap();
        let same_end = parse_filter_datetime(Some("2024-01-01T00:00:00.000Z"))
            .unwrap()
            .unwrap();
        let after_end = DateTime::parse_from_rfc3339("2024-01-01T00:00:01Z")
            .unwrap()
            .with_timezone(&Utc);

        assert!(activity_date_in_range(
            &activity_date,
            Some(&same_start),
            Some(&same_end)
        ));
        assert!(!activity_date_in_range(&after_end, None, Some(&same_end)));
    }
}
