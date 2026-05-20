use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use wealthfolio_core::accounts::{
    account_supports_purpose, AccountPurpose, AccountRepositoryTrait,
};
use wealthfolio_core::activities::{Activity, ActivityRepositoryTrait};
use wealthfolio_core::taxonomies::TaxonomyServiceTrait;

use super::model::{
    CategoryBreakdownRow, CategorySpending, DayBucket, DayCategoryBucket, EventCategorySpending,
    EventSpendingSummary, EventSummariesRequest, MonthlyReport, PeriodSummary, ReportRequest,
    SpendingSummary, SubcategorySpending,
};
use crate::activity_assignments::ActivityTaxonomyAssignmentRepositoryTrait;
use crate::activity_classification::{
    activity_abs_amount, classify_activity, SpendingClassification,
};
use crate::events::EventsService;
use crate::settings::SpendingSettingsService;

const SPENDING_TAXONOMY: &str = "spending_categories";
const INCOME_TAXONOMY: &str = "income_sources";

pub struct AnalyticsService {
    activity_repo: Arc<dyn ActivityRepositoryTrait>,
    account_repo: Arc<dyn AccountRepositoryTrait>,
    assignment_repo: Arc<dyn ActivityTaxonomyAssignmentRepositoryTrait>,
    settings: Arc<SpendingSettingsService>,
    taxonomy_service: Arc<dyn TaxonomyServiceTrait>,
    events_service: Arc<EventsService>,
}

impl AnalyticsService {
    pub fn new(
        activity_repo: Arc<dyn ActivityRepositoryTrait>,
        account_repo: Arc<dyn AccountRepositoryTrait>,
        assignment_repo: Arc<dyn ActivityTaxonomyAssignmentRepositoryTrait>,
        settings: Arc<SpendingSettingsService>,
        taxonomy_service: Arc<dyn TaxonomyServiceTrait>,
        events_service: Arc<EventsService>,
    ) -> Self {
        Self {
            activity_repo,
            account_repo,
            assignment_repo,
            settings,
            taxonomy_service,
            events_service,
        }
    }

    fn resolve_spending_account_types(
        &self,
        account_ids: &[String],
    ) -> Result<(Vec<String>, HashMap<String, String>)> {
        let accounts = self
            .account_repo
            .list(None, Some(false), Some(account_ids))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let account_types: HashMap<String, String> = accounts
            .into_iter()
            .filter(|a| account_supports_purpose(&a.account_type, AccountPurpose::Spending))
            .map(|a| (a.id, a.account_type))
            .collect();
        let spending_account_ids = account_ids
            .iter()
            .filter(|id| account_types.contains_key(id.as_str()))
            .cloned()
            .collect();

        Ok((spending_account_ids, account_types))
    }

    /// Compute a monthly report covering [start_date, end_date].
    /// "Prior" period uses an equally-sized window immediately preceding the current one.
    /// `timezone` (IANA name, may be empty) drives per-day bucketing so a
    /// midnight-local activity lands on the date the user perceives. Empty/
    /// invalid values fall back to UTC.
    pub async fn monthly_report(
        &self,
        req: ReportRequest,
        timezone: &str,
    ) -> Result<MonthlyReport> {
        let s = self.settings.get().await?;
        if !s.enabled || s.account_ids.is_empty() {
            return Ok(MonthlyReport {
                current: PeriodSummary::default(),
                prior: PeriodSummary::default(),
                spending_breakdown: vec![],
                income_breakdown: vec![],
                by_day: vec![],
                by_day_by_category: vec![],
            });
        }
        let target_accounts: Vec<String> = match req.account_ids.clone() {
            Some(ids) => ids
                .into_iter()
                .filter(|id| s.account_ids.contains(id))
                .collect(),
            None => s.account_ids.clone(),
        };
        if target_accounts.is_empty() {
            return Ok(MonthlyReport {
                current: PeriodSummary::default(),
                prior: PeriodSummary::default(),
                spending_breakdown: vec![],
                income_breakdown: vec![],
                by_day: vec![],
                by_day_by_category: vec![],
            });
        }
        let (target_accounts, account_types) =
            self.resolve_spending_account_types(&target_accounts)?;
        if target_accounts.is_empty() {
            return Ok(MonthlyReport {
                current: PeriodSummary::default(),
                prior: PeriodSummary::default(),
                spending_breakdown: vec![],
                income_breakdown: vec![],
                by_day: vec![],
                by_day_by_category: vec![],
            });
        }

        let start = DateTime::parse_from_rfc3339(&req.start_date)?.with_timezone(&Utc);
        let end = DateTime::parse_from_rfc3339(&req.end_date)?.with_timezone(&Utc);
        // Prior window: same length, immediately preceding `start`. The `−1s`
        // exclusive boundary already shifts prior_end one tick below current's
        // start; we therefore subtract `period_secs − 1` (not `period_secs`)
        // so prior_start..=prior_end covers the same number of seconds as
        // current. Without the −1, prior was always one second shorter.
        let period_secs = (end - start).num_seconds().max(1);
        let prior_end = start - Duration::seconds(1);
        let prior_start = prior_end - Duration::seconds((period_secs - 1).max(0));

        let activities = self
            .activity_repo
            .get_activities_by_account_ids(&target_accounts)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let in_window = |a: &Activity, lo: DateTime<Utc>, hi: DateTime<Utc>| {
            a.activity_date >= lo && a.activity_date <= hi
        };

        let current_acts: Vec<&Activity> = activities
            .iter()
            .filter(|a| in_window(a, start, end))
            .collect();
        let prior_acts: Vec<&Activity> = activities
            .iter()
            .filter(|a| in_window(a, prior_start, prior_end))
            .collect();

        let current = summarize(&current_acts, &account_types);
        let prior = summarize(&prior_acts, &account_types);

        // Per-day buckets (current period only)
        let mut by_day_map: HashMap<NaiveDate, (f64, f64)> = HashMap::new();
        for a in &current_acts {
            let Some(classification) = classification_for(a, &account_types) else {
                continue;
            };
            let amt = activity_abs_amount(a);
            let income_amount = classification.income_amount(amt);
            let spending_amount = classification.spending_amount(amt);
            if income_amount == 0.0 && spending_amount == 0.0 {
                continue;
            }
            let d = wealthfolio_core::utils::time_utils::activity_date_in_user_timezone(
                a.activity_date,
                timezone,
            );
            let entry = by_day_map.entry(d).or_insert((0.0, 0.0));
            entry.0 += income_amount;
            entry.1 += spending_amount;
        }
        let positive_spending_days: HashSet<String> = by_day_map
            .iter()
            .filter(|(_, (_, outflow))| *outflow > 0.0)
            .map(|(d, _)| format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day()))
            .collect();
        let mut by_day: Vec<DayBucket> = by_day_map
            .into_iter()
            .map(|(d, (income, outflow))| DayBucket {
                date: format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day()),
                income,
                outflow: outflow.max(0.0),
            })
            .filter(|bucket| bucket.income != 0.0 || bucket.outflow != 0.0)
            .collect();
        by_day.sort_by(|a, b| a.date.cmp(&b.date));

        // Category breakdown — fetch assignments for the activities in scope
        let mut spending_acc: HashMap<(String, String), (f64, usize)> = HashMap::new();
        let mut income_acc: HashMap<(String, String), (f64, usize)> = HashMap::new();
        // (date, taxonomy_id, category_id) → (amount, count)
        let mut by_day_cat_acc: HashMap<(String, String, String), (f64, usize)> = HashMap::new();
        for a in &current_acts {
            let Some(classification) = classification_for(a, &account_types) else {
                continue;
            };
            let amt = activity_abs_amount(a);
            let income_amount = classification.income_amount(amt);
            let spending_amount = classification.spending_amount(amt);
            if income_amount == 0.0 && spending_amount == 0.0 {
                continue;
            }
            let day = wealthfolio_core::utils::time_utils::activity_date_in_user_timezone(
                a.activity_date,
                timezone,
            );
            let day_str = format!("{:04}-{:02}-{:02}", day.year(), day.month(), day.day());
            let assignments = self.assignment_repo.list_for_activity(&a.id).await?;
            for asg in assignments {
                let bucket = if asg.taxonomy_id == SPENDING_TAXONOMY && spending_amount != 0.0 {
                    Some((&mut spending_acc, spending_amount))
                } else if asg.taxonomy_id == INCOME_TAXONOMY && income_amount != 0.0 {
                    Some((&mut income_acc, income_amount))
                } else {
                    None
                };
                if let Some((b, bucket_amount)) = bucket {
                    let entry = b
                        .entry((asg.taxonomy_id.clone(), asg.category_id.clone()))
                        .or_insert((0.0, 0));
                    entry.0 += bucket_amount;
                    entry.1 += 1;
                    // Same activity → same (day, taxonomy, category) bucket.
                    let dc = by_day_cat_acc
                        .entry((
                            day_str.clone(),
                            asg.taxonomy_id.clone(),
                            asg.category_id.clone(),
                        ))
                        .or_insert((0.0, 0));
                    dc.0 += bucket_amount;
                    dc.1 += 1;
                }
            }
        }

        let mut spending_breakdown: Vec<CategoryBreakdownRow> = spending_acc
            .into_iter()
            .filter(|(_, (amount, _))| current.outflow > 0.0 && *amount != 0.0)
            .map(
                |((taxonomy_id, category_id), (amount, count))| CategoryBreakdownRow {
                    taxonomy_id,
                    category_id,
                    amount,
                    count,
                },
            )
            .collect();
        spending_breakdown.sort_by(|a, b| {
            b.amount
                .partial_cmp(&a.amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut income_breakdown: Vec<CategoryBreakdownRow> = income_acc
            .into_iter()
            .map(
                |((taxonomy_id, category_id), (amount, count))| CategoryBreakdownRow {
                    taxonomy_id,
                    category_id,
                    amount,
                    count,
                },
            )
            .collect();
        income_breakdown.sort_by(|a, b| {
            b.amount
                .partial_cmp(&a.amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut by_day_by_category: Vec<DayCategoryBucket> = by_day_cat_acc
            .into_iter()
            .filter(|((date, taxonomy_id, _), (amount, _))| {
                *amount != 0.0
                    && (taxonomy_id == INCOME_TAXONOMY || positive_spending_days.contains(date))
            })
            .map(
                |((date, taxonomy_id, category_id), (amount, count))| DayCategoryBucket {
                    date,
                    taxonomy_id,
                    category_id,
                    amount,
                    count,
                },
            )
            .collect();
        by_day_by_category.sort_by(|a, b| a.date.cmp(&b.date));

        Ok(MonthlyReport {
            current,
            prior,
            spending_breakdown,
            income_breakdown,
            by_day,
            by_day_by_category,
        })
    }
}

fn summarize(acts: &[&Activity], account_types: &HashMap<String, String>) -> PeriodSummary {
    let mut income = 0.0;
    let mut outflow = 0.0;
    let mut count = 0;
    for a in acts {
        let Some(classification) = classification_for(a, account_types) else {
            continue;
        };
        let amt = activity_abs_amount(a);
        let income_amount = classification.income_amount(amt);
        let spending_amount = classification.spending_amount(amt);
        if income_amount == 0.0 && spending_amount == 0.0 {
            continue;
        }
        income += income_amount;
        outflow += spending_amount;
        count += 1;
    }
    // Display `outflow` as a non-negative magnitude (a refund-only period shows
    // "$0 spent" rather than "-$50"), but compute `net` from the signed outflow
    // so refunds correctly flow into net cashflow as positive contributions.
    // This keeps net = income - signed_outflow in agreement with the insight
    // pipeline's `Headline.net_cashflow` (insight/service.rs:337).
    PeriodSummary {
        income,
        outflow: outflow.max(0.0),
        net: income - outflow,
        count,
    }
}

fn classification_for(
    activity: &Activity,
    account_types: &HashMap<String, String>,
) -> Option<SpendingClassification> {
    account_types
        .get(&activity.account_id)
        .map(|account_type| classify_activity(activity, account_type))
}

// ====================== SpendingSummary (PR-style multi-period rollup) ======================

impl AnalyticsService {
    /// Compute spending summaries for the periods consumed by the spending overview UI:
    /// `TOTAL`, `YTD`, `LAST_YEAR`, `TWO_YEARS_AGO`. The frontend picks the relevant one.
    ///
    /// `include_event_ids` — if Some(non-empty), only activities with `event_id` in this set are counted.
    /// `include_all_events` — if true, only activities that ARE tagged with any event are counted.
    pub async fn spending_summary(
        &self,
        include_event_ids: Option<Vec<String>>,
        include_all_events: Option<bool>,
    ) -> Result<Vec<SpendingSummary>> {
        let s = self.settings.get().await?;
        let mut out = Vec::with_capacity(4);
        if !s.enabled || s.account_ids.is_empty() {
            for period in ["TOTAL", "YTD", "LAST_YEAR", "TWO_YEARS_AGO"] {
                out.push(empty_summary(period));
            }
            return Ok(out);
        }
        let (target_accounts, account_types) =
            self.resolve_spending_account_types(&s.account_ids)?;
        if target_accounts.is_empty() {
            for period in ["TOTAL", "YTD", "LAST_YEAR", "TWO_YEARS_AGO"] {
                out.push(empty_summary(period));
            }
            return Ok(out);
        }

        // Pull category metadata for spending_categories (for names + colors + parent map)
        let taxonomy_with_cats = self
            .taxonomy_service
            .get_taxonomy(SPENDING_TAXONOMY)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let categories = taxonomy_with_cats
            .map(|tw| tw.categories)
            .unwrap_or_default();
        let mut cat_meta: HashMap<String, (String, Option<String>, Option<String>)> =
            HashMap::new();
        for c in &categories {
            // (name, color_opt, parent_id_opt)
            cat_meta.insert(
                c.id.clone(),
                (c.name.clone(), Some(c.color.clone()), c.parent_id.clone()),
            );
        }

        // Load all activities for the spending accounts
        let activities = self
            .activity_repo
            .get_activities_by_account_ids(&target_accounts)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Pre-load assignments per activity in scope (only for outflow + spending taxonomy)
        let mut assign_by_act: HashMap<String, Option<String>> = HashMap::new();
        for a in &activities {
            let Some(classification) = classification_for(a, &account_types) else {
                continue;
            };
            if classification.spending_amount(activity_abs_amount(a)) == 0.0 {
                continue;
            }
            let assignments = self.assignment_repo.list_for_activity(&a.id).await?;
            let cat = assignments
                .into_iter()
                .find(|x| x.taxonomy_id == SPENDING_TAXONOMY)
                .map(|x| x.category_id);
            assign_by_act.insert(a.id.clone(), cat);
        }

        // Event filter set
        let include_set: Option<HashSet<String>> = include_event_ids
            .as_ref()
            .filter(|v| !v.is_empty())
            .map(|v| v.iter().cloned().collect());
        let only_with_events = include_all_events.unwrap_or(false);

        let now = Utc::now();
        let year_now = now.year();
        let ytd_start = NaiveDate::from_ymd_opt(year_now, 1, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .unwrap();
        let last_year_start = NaiveDate::from_ymd_opt(year_now - 1, 1, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .unwrap();
        let last_year_end = NaiveDate::from_ymd_opt(year_now - 1, 12, 31)
            .and_then(|d| d.and_hms_opt(23, 59, 59))
            .unwrap();
        let two_years_ago_start = NaiveDate::from_ymd_opt(year_now - 2, 1, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .unwrap();
        let two_years_ago_end = NaiveDate::from_ymd_opt(year_now - 2, 12, 31)
            .and_then(|d| d.and_hms_opt(23, 59, 59))
            .unwrap();

        let currency = s
            .account_ids
            .first()
            .and_then(|_| activities.first().map(|a| a.currency.clone()))
            .unwrap_or_else(|| "USD".to_string());

        // Filter helper for an activity
        let activity_passes = |a: &Activity| -> bool {
            // Event filter (a's event_id must satisfy)
            if only_with_events && a.event_id.is_none() {
                return false;
            }
            if let Some(set) = &include_set {
                match &a.event_id {
                    Some(eid) if set.contains(eid) => {}
                    _ => return false,
                }
            }
            true
        };

        for period in ["TOTAL", "YTD", "LAST_YEAR", "TWO_YEARS_AGO"] {
            let in_window: Vec<&Activity> = activities
                .iter()
                .filter(|a| {
                    let Some(classification) = classification_for(a, &account_types) else {
                        return false;
                    };
                    if classification.spending_amount(activity_abs_amount(a)) == 0.0 {
                        return false;
                    }
                    let dt = a.activity_date.naive_utc();
                    let in_period = match period {
                        "TOTAL" => true,
                        "YTD" => dt >= ytd_start,
                        "LAST_YEAR" => dt >= last_year_start && dt <= last_year_end,
                        "TWO_YEARS_AGO" => dt >= two_years_ago_start && dt <= two_years_ago_end,
                        _ => false,
                    };
                    in_period && activity_passes(a)
                })
                .collect();

            out.push(build_summary(
                period,
                &in_window,
                &assign_by_act,
                &cat_meta,
                &account_types,
                &currency,
            ));
        }

        Ok(out)
    }
}

fn activity_date_in_window(
    activity_date: &DateTime<Utc>,
    start: Option<&DateTime<Utc>>,
    end: Option<&DateTime<Utc>>,
) -> bool {
    start.is_none_or(|start| activity_date >= start) && end.is_none_or(|end| activity_date <= end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal::Decimal;
    use serde_json::Value;
    use wealthfolio_core::accounts::account_types;
    use wealthfolio_core::activities::ActivityStatus;

    fn spending_activity(
        id: &str,
        activity_type: &str,
        amount: i64,
        category_id: &str,
        month: u32,
    ) -> (Activity, (String, Option<String>)) {
        let activity = Activity {
            id: id.to_string(),
            account_id: "card-account".to_string(),
            asset_id: None,
            activity_type: activity_type.to_string(),
            activity_type_override: None,
            source_type: None,
            subtype: None,
            status: ActivityStatus::Posted,
            activity_date: Utc.with_ymd_and_hms(2024, month, 10, 12, 0, 0).unwrap(),
            settlement_date: None,
            quantity: None,
            unit_price: None,
            amount: Some(Decimal::new(amount, 0)),
            fee: None,
            currency: "USD".to_string(),
            fx_rate: None,
            notes: None,
            metadata: None::<Value>,
            source_system: None,
            source_record_id: None,
            source_group_id: None,
            idempotency_key: None,
            import_run_id: None,
            is_user_modified: false,
            needs_review: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            event_id: None,
        };

        (activity, (id.to_string(), Some(category_id.to_string())))
    }

    fn build_credit_card_summary(activities: &[Activity]) -> SpendingSummary {
        let activity_refs: Vec<&Activity> = activities.iter().collect();
        let account_types = HashMap::from([(
            "card-account".to_string(),
            account_types::CREDIT_CARD.to_string(),
        )]);
        let cat_meta = HashMap::from([
            (
                "groceries".to_string(),
                ("Groceries".to_string(), Some("#1".to_string()), None),
            ),
            (
                "travel".to_string(),
                ("Travel".to_string(), Some("#2".to_string()), None),
            ),
        ]);
        let assign_by_act = activities
            .iter()
            .map(|activity| {
                let category = if activity.id == "charge" {
                    "groceries"
                } else {
                    "travel"
                };
                (activity.id.clone(), Some(category.to_string()))
            })
            .collect();

        build_summary(
            "TOTAL",
            &activity_refs,
            &assign_by_act,
            &cat_meta,
            &account_types,
            "USD",
        )
    }

    #[test]
    fn build_summary_preserves_refund_buckets_when_period_stays_positive() {
        let (charge, _) = spending_activity("charge", "WITHDRAWAL", 200, "groceries", 1);
        let (refund, _) = spending_activity("refund", "CREDIT", 50, "travel", 2);

        let summary = build_credit_card_summary(&[charge, refund]);

        assert_eq!(summary.total_spending, 150.0);
        assert_eq!(summary.by_month.get("2024-01"), Some(&200.0));
        assert_eq!(summary.by_month.get("2024-02"), Some(&-50.0));
        assert_eq!(summary.by_category["groceries"].amount, 200.0);
        assert_eq!(summary.by_category["travel"].amount, -50.0);
        assert_eq!(summary.transaction_count, 1);
    }

    #[test]
    fn build_summary_clears_buckets_when_refunds_exceed_charges() {
        let (charge, _) = spending_activity("charge", "WITHDRAWAL", 100, "groceries", 1);
        let (refund, _) = spending_activity("refund", "CREDIT", 150, "travel", 2);

        let summary = build_credit_card_summary(&[charge, refund]);

        assert_eq!(summary.total_spending, 0.0);
        assert!(summary.by_month.is_empty());
        assert!(summary.by_category.is_empty());
        assert!(summary.by_account.is_empty());
        assert_eq!(summary.transaction_count, 0);
    }

    #[test]
    fn event_summary_window_filters_activity_dates() {
        let start = Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 2, 29, 23, 59, 59).unwrap();
        let in_window = Utc.with_ymd_and_hms(2024, 2, 10, 12, 0, 0).unwrap();
        let before_window = Utc.with_ymd_and_hms(2024, 1, 31, 23, 59, 59).unwrap();
        let after_window = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();

        assert!(activity_date_in_window(
            &in_window,
            Some(&start),
            Some(&end)
        ));
        assert!(!activity_date_in_window(
            &before_window,
            Some(&start),
            Some(&end)
        ));
        assert!(!activity_date_in_window(
            &after_window,
            Some(&start),
            Some(&end)
        ));
    }
}

fn empty_summary(period: &str) -> SpendingSummary {
    SpendingSummary {
        period: period.to_string(),
        by_month: HashMap::new(),
        by_category: HashMap::new(),
        by_subcategory: HashMap::new(),
        by_account: HashMap::new(),
        by_month_by_category: HashMap::new(),
        by_month_by_subcategory: HashMap::new(),
        total_spending: 0.0,
        currency: "USD".to_string(),
        monthly_average: 0.0,
        transaction_count: 0,
        yoy_growth: None,
    }
}

fn build_summary(
    period: &str,
    activities: &[&Activity],
    assign_by_act: &HashMap<String, Option<String>>,
    cat_meta: &HashMap<String, (String, Option<String>, Option<String>)>,
    account_types: &HashMap<String, String>,
    currency: &str,
) -> SpendingSummary {
    let mut by_month: HashMap<String, f64> = HashMap::new();
    let mut by_account: HashMap<String, f64> = HashMap::new();
    let mut by_category: HashMap<String, CategorySpending> = HashMap::new();
    let mut by_subcategory: HashMap<String, SubcategorySpending> = HashMap::new();
    let mut by_month_by_category: HashMap<String, HashMap<String, f64>> = HashMap::new();
    let mut by_month_by_subcategory: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for a in activities {
        let Some(classification) = classification_for(a, account_types) else {
            continue;
        };
        let amt = classification.spending_amount(activity_abs_amount(a));
        if amt == 0.0 {
            continue;
        }
        let dt = a.activity_date.naive_utc();
        let month_key = format!("{:04}-{:02}", dt.year(), dt.month());
        *by_month.entry(month_key.clone()).or_insert(0.0) += amt;
        *by_account.entry(a.account_id.clone()).or_insert(0.0) += amt;

        // Resolve which top-level category + (optional) subcategory this activity belongs to
        let assigned_cat_id = assign_by_act.get(&a.id).and_then(|opt| opt.as_ref());
        let (top_cat_id, sub_cat_id, top_name, top_color, sub_name) = match assigned_cat_id {
            Some(cid) => match cat_meta.get(cid) {
                Some((name, color, parent_id)) => match parent_id {
                    // assigned to a subcategory: parent is the top-level
                    Some(pid) => {
                        let parent = cat_meta.get(pid);
                        let parent_name = parent
                            .map(|(n, _, _)| n.clone())
                            .unwrap_or_else(|| pid.clone());
                        let parent_color = parent.and_then(|(_, c, _)| c.clone());
                        (
                            Some(pid.clone()),
                            Some(cid.clone()),
                            parent_name,
                            parent_color,
                            name.clone(),
                        )
                    }
                    // assigned to a top-level category
                    None => (
                        Some(cid.clone()),
                        None,
                        name.clone(),
                        color.clone(),
                        String::new(),
                    ),
                },
                None => (Some(cid.clone()), None, cid.clone(), None, String::new()),
            },
            None => (None, None, "Uncategorized".to_string(), None, String::new()),
        };

        let top_key = top_cat_id
            .clone()
            .unwrap_or_else(|| "uncategorized".to_string());
        let cat_entry = by_category
            .entry(top_key.clone())
            .or_insert(CategorySpending {
                category_id: top_cat_id.clone(),
                category_name: top_name.clone(),
                color: top_color.clone(),
                amount: 0.0,
                transaction_count: 0,
            });
        cat_entry.amount += amt;
        cat_entry.transaction_count += 1;

        if let Some(sub_id) = sub_cat_id.clone() {
            let sub_entry = by_subcategory
                .entry(sub_id.clone())
                .or_insert(SubcategorySpending {
                    subcategory_id: Some(sub_id.clone()),
                    subcategory_name: sub_name,
                    category_id: top_cat_id.clone(),
                    category_name: top_name.clone(),
                    color: top_color.clone(),
                    amount: 0.0,
                    transaction_count: 0,
                });
            sub_entry.amount += amt;
            sub_entry.transaction_count += 1;

            *by_month_by_subcategory
                .entry(month_key.clone())
                .or_default()
                .entry(sub_id)
                .or_insert(0.0) += amt;
        }

        *by_month_by_category
            .entry(month_key)
            .or_default()
            .entry(top_key)
            .or_insert(0.0) += amt;
    }

    let total: f64 = by_month.values().sum();
    if total <= 0.0 {
        by_month.clear();
        by_account.clear();
        by_category.clear();
        by_subcategory.clear();
        by_month_by_category.clear();
        by_month_by_subcategory.clear();
    } else {
        retain_nonzero_values(&mut by_month);
        retain_nonzero_values(&mut by_account);
        retain_nonzero_nested_values(&mut by_month_by_category);
        retain_nonzero_nested_values(&mut by_month_by_subcategory);
        by_category.retain(|_, value| value.amount != 0.0);
        by_subcategory.retain(|_, value| value.amount != 0.0);
    }

    let n_months = by_month.len() as f64;
    let monthly_average = if n_months > 0.0 {
        total / n_months
    } else {
        0.0
    };
    let transaction_count = if total > 0.0 {
        activities
            .iter()
            .filter(|activity| {
                classification_for(activity, account_types)
                    .map(|classification| {
                        classification.spending_amount(activity_abs_amount(activity)) > 0.0
                    })
                    .unwrap_or(false)
            })
            .count()
    } else {
        0
    };

    SpendingSummary {
        period: period.to_string(),
        by_month,
        by_category,
        by_subcategory,
        by_account,
        by_month_by_category,
        by_month_by_subcategory,
        total_spending: total.max(0.0),
        currency: currency.to_string(),
        monthly_average,
        transaction_count,
        yoy_growth: None,
    }
}

fn retain_nonzero_values(values: &mut HashMap<String, f64>) {
    values.retain(|_, amount| *amount != 0.0);
}

fn retain_nonzero_nested_values(values: &mut HashMap<String, HashMap<String, f64>>) {
    values.retain(|_, inner| {
        retain_nonzero_values(inner);
        !inner.is_empty()
    });
}

// Helper to silence unused warning when Duration not referenced elsewhere
#[allow(dead_code)]
fn _silence_duration() {
    let _ = Duration::seconds(0);
}

// ====================== EventSpendingSummary (per-event rollups) ======================

impl AnalyticsService {
    /// Compute per-event spending summaries. Each event in the events table is intersected
    /// with the optional date window (events whose date range overlaps with [start, end]).
    /// Tagged activities count according to account-aware spending classification.
    pub async fn event_spending_summaries(
        &self,
        req: EventSummariesRequest,
    ) -> Result<Vec<EventSpendingSummary>> {
        let s = self.settings.get().await?;
        if !s.enabled || s.account_ids.is_empty() {
            return Ok(Vec::new());
        }
        let (target_accounts, account_types) =
            self.resolve_spending_account_types(&s.account_ids)?;
        if target_accounts.is_empty() {
            return Ok(Vec::new());
        }

        let events = self.events_service.list_events_with_names().await?;
        if events.is_empty() {
            return Ok(Vec::new());
        }
        let event_types = self.events_service.list_types().await?;
        let type_color: HashMap<String, Option<String>> = event_types
            .iter()
            .map(|t| (t.id.clone(), t.color.clone()))
            .collect();

        // Optional date window
        let window_start = req
            .start_date
            .as_deref()
            .map(|s| DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&Utc)))
            .transpose()?;
        let window_end = req
            .end_date
            .as_deref()
            .map(|s| DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&Utc)))
            .transpose()?;

        let in_window = |event_start: &str, event_end: &str| -> bool {
            // Events have YYYY-MM-DD date strings; compare lexicographically
            if let Some(ws) = window_start {
                let we_iso = format!("{}-{:02}-{:02}", ws.year(), ws.month(), ws.day());
                if event_end < we_iso.as_str() {
                    return false;
                }
            }
            if let Some(we) = window_end {
                let ws_iso = format!("{}-{:02}-{:02}", we.year(), we.month(), we.day());
                if event_start > ws_iso.as_str() {
                    return false;
                }
            }
            true
        };

        // Load category metadata for spending taxonomy
        let taxonomy_with_cats = self
            .taxonomy_service
            .get_taxonomy(SPENDING_TAXONOMY)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let categories = taxonomy_with_cats
            .map(|tw| tw.categories)
            .unwrap_or_default();
        let cat_meta: HashMap<String, (String, Option<String>, Option<String>)> = categories
            .iter()
            .map(|c| {
                (
                    c.id.clone(),
                    (c.name.clone(), Some(c.color.clone()), c.parent_id.clone()),
                )
            })
            .collect();

        // Load all activities once and index by event_id
        let activities = self
            .activity_repo
            .get_activities_by_account_ids(&target_accounts)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let mut by_event: HashMap<String, Vec<Activity>> = HashMap::new();
        for a in activities {
            if !activity_date_in_window(
                &a.activity_date,
                window_start.as_ref(),
                window_end.as_ref(),
            ) {
                continue;
            }
            if let Some(eid) = a.event_id.clone() {
                let Some(classification) = classification_for(&a, &account_types) else {
                    continue;
                };
                if classification.spending_amount(activity_abs_amount(&a)) == 0.0 {
                    continue;
                }
                by_event.entry(eid).or_default().push(a);
            }
        }

        let currency = req.currency.unwrap_or_else(|| "USD".to_string());

        let mut out = Vec::with_capacity(events.len());
        for ev in events {
            if !in_window(&ev.event.start_date, &ev.event.end_date) {
                continue;
            }
            let acts = by_event.remove(&ev.event.id).unwrap_or_default();

            let mut total = 0.0f64;
            let mut by_category: HashMap<String, EventCategorySpending> = HashMap::new();
            let mut daily: HashMap<String, f64> = HashMap::new();

            for a in &acts {
                let Some(classification) = classification_for(a, &account_types) else {
                    continue;
                };
                let amt = classification.spending_amount(activity_abs_amount(a));
                if amt == 0.0 {
                    continue;
                }
                total += amt;
                let dt = a.activity_date.naive_utc();
                let day = format!("{:04}-{:02}-{:02}", dt.year(), dt.month(), dt.day());
                *daily.entry(day).or_insert(0.0) += amt;

                // Resolve category for spending taxonomy via the assignments table
                let assignments = self.assignment_repo.list_for_activity(&a.id).await?;
                let asg = assignments
                    .into_iter()
                    .find(|x| x.taxonomy_id == SPENDING_TAXONOMY);
                let (cat_id_opt, cat_name, cat_color) = match asg {
                    Some(asg) => match cat_meta.get(&asg.category_id) {
                        Some((name, color, _parent)) => {
                            (Some(asg.category_id.clone()), name.clone(), color.clone())
                        }
                        None => (Some(asg.category_id.clone()), asg.category_id, None),
                    },
                    None => (None, "Uncategorized".to_string(), None),
                };
                let key = cat_id_opt
                    .clone()
                    .unwrap_or_else(|| "uncategorized".to_string());
                let entry = by_category.entry(key).or_insert(EventCategorySpending {
                    category_id: cat_id_opt,
                    category_name: cat_name,
                    color: cat_color,
                    amount: 0.0,
                    transaction_count: 0,
                });
                entry.amount += amt;
                entry.transaction_count += 1;
            }
            let transaction_count = acts
                .iter()
                .filter(|activity| {
                    classification_for(activity, &account_types)
                        .map(|classification| {
                            classification.spending_amount(activity_abs_amount(activity)) > 0.0
                        })
                        .unwrap_or(false)
                })
                .count();
            if total <= 0.0 {
                total = 0.0;
                daily.clear();
                by_category.clear();
            } else {
                daily.retain(|_, amount| *amount != 0.0);
                by_category.retain(|_, value| value.amount != 0.0);
            }

            out.push(EventSpendingSummary {
                event_id: ev.event.id,
                event_name: ev.event.name,
                event_type_id: ev.event.event_type_id.clone(),
                event_type_name: ev.event_type_name,
                event_type_color: type_color.get(&ev.event.event_type_id).cloned().flatten(),
                start_date: ev.event.start_date,
                end_date: ev.event.end_date,
                total_spending: total,
                transaction_count: if total > 0.0 { transaction_count } else { 0 },
                currency: currency.clone(),
                by_category,
                daily_spending: daily,
            });
        }

        Ok(out)
    }
}
