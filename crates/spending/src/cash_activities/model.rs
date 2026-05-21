use serde::{Deserialize, Serialize};

use crate::activity_assignments::ActivityTaxonomyAssignment;
use wealthfolio_core::activities::Activity;

/// Filter for listing cash activities. All fields optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashActivityFilter {
    /// Restrict to specific accounts (intersected with the spending account list).
    /// If None, all spending accounts are queried.
    pub account_ids: Option<Vec<String>>,
    /// Restrict to a date window (RFC3339 strings on either side; both inclusive).
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    /// Restrict to specific activity_types. If None, defaults to CASH_ACTIVITY_TYPES.
    pub activity_types: Option<Vec<String>>,
}

/// Status filter for cash-activity search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CashActivityStatusFilter {
    All,
    NeedsReview,
    Uncategorized,
    Categorized,
}

impl Default for CashActivityStatusFilter {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CashActivitySortField {
    Date,
    Amount,
}

impl Default for CashActivitySortField {
    fn default() -> Self {
        Self::Date
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    Desc,
}

impl Default for SortDirection {
    fn default() -> Self {
        Self::Desc
    }
}

/// Search request for cash activities. Powers the spending Transactions page.
/// All filters optional. Server-side: filters → sort → paginate → join assignments.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashActivitySearchRequest {
    /// Free-text search over notes (payee). Case-insensitive contains-match.
    pub search: Option<String>,
    /// Restrict to these accounts (intersected with the spending account list).
    pub account_ids: Option<Vec<String>>,
    /// Restrict to specific activity_types. If None, defaults to CASH_ACTIVITY_TYPES.
    pub activity_types: Option<Vec<String>>,
    /// Filter to activities assigned to any of these top-level categories
    /// (caller is responsible for expanding subcategories).
    pub category_ids: Option<Vec<String>>,
    /// Filter to activities assigned to specific (sub)category ids.
    pub subcategory_ids: Option<Vec<String>>,
    /// Filter to activities tagged with these events (uses Activity.event_id).
    pub event_ids: Option<Vec<String>>,
    /// Status: All / NeedsReview / Uncategorized / Categorized.
    #[serde(default)]
    pub status: CashActivityStatusFilter,
    /// Date window — RFC3339 strings, inclusive.
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    /// Absolute amount range (operates on |amount|).
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
    /// Sort.
    #[serde(default)]
    pub sort_by: CashActivitySortField,
    #[serde(default)]
    pub sort_dir: SortDirection,
    /// Pagination.
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// Activity row enriched with its single-select activity-scope assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashActivityWithAssignments {
    #[serde(flatten)]
    pub activity: Activity,
    /// All activity-scope assignments for this row (typically 0 or 1 for single-select).
    pub assignments: Vec<ActivityTaxonomyAssignment>,
    /// Spending event tag, sourced from the `activity_events` join table.
    /// `None` when the activity isn't tagged. Surfaced here rather than on
    /// the core `Activity` so the portfolio-side struct stays free of
    /// spending-domain coupling; the frontend reads this field via the
    /// `CashActivity` flattened projection.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
}

/// Paginated response for cash-activity search.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashActivitySearchResponse {
    pub items: Vec<CashActivityWithAssignments>,
    /// Total rows matching the filters (for pagination UI).
    pub total_count: usize,
}
