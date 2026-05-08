use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use wealthfolio_core::activities::{Activity, ActivityRepositoryTrait};
use wealthfolio_core::taxonomies::TaxonomyServiceTrait;

use super::model::{
    CategoryBreakdownRow, CategorySpending, DayBucket, DayCategoryBucket, EventCategorySpending,
    EventSpendingSummary, EventSummariesRequest, MonthlyReport, PeriodSummary, ReportRequest,
    SpendingSummary, SubcategorySpending,
};
use crate::activity_assignments::ActivityTaxonomyAssignmentRepositoryTrait;
use crate::events::EventsService;
use crate::settings::SpendingSettingsService;

const SPENDING_TAXONOMY: &str = "spending_categories";
const INCOME_TAXONOMY: &str = "income_sources";

const INCOME_TYPES: &[&str] = &["DEPOSIT", "TRANSFER_IN", "INTEREST"];
const OUTFLOW_TYPES: &[&str] = &["WITHDRAWAL", "TRANSFER_OUT", "FEE"];

pub struct AnalyticsService {
    activity_repo: Arc<dyn ActivityRepositoryTrait>,
    assignment_repo: Arc<dyn ActivityTaxonomyAssignmentRepositoryTrait>,
    settings: Arc<SpendingSettingsService>,
    taxonomy_service: Arc<dyn TaxonomyServiceTrait>,
    events_service: Arc<EventsService>,
}

impl AnalyticsService {
    pub fn new(
        activity_repo: Arc<dyn ActivityRepositoryTrait>,
        assignment_repo: Arc<dyn ActivityTaxonomyAssignmentRepositoryTrait>,
        settings: Arc<SpendingSettingsService>,
        taxonomy_service: Arc<dyn TaxonomyServiceTrait>,
        events_service: Arc<EventsService>,
    ) -> Self {
        Self {
            activity_repo,
            assignment_repo,
            settings,
            taxonomy_service,
            events_service,
        }
    }

    /// Compute a monthly report covering [start_date, end_date].
    /// "Prior" period uses an equally-sized window immediately preceding the current one.
    pub async fn monthly_report(&self, req: ReportRequest) -> Result<MonthlyReport> {
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

        let start = DateTime::parse_from_rfc3339(&req.start_date)?.with_timezone(&Utc);
        let end = DateTime::parse_from_rfc3339(&req.end_date)?.with_timezone(&Utc);
        let period_secs = (end - start).num_seconds().max(1);
        let prior_end = start - Duration::seconds(1);
        let prior_start = prior_end - Duration::seconds(period_secs);

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

        let current = summarize(&current_acts);
        let prior = summarize(&prior_acts);

        // Per-day buckets (current period only)
        let mut by_day_map: HashMap<NaiveDate, (f64, f64)> = HashMap::new();
        for a in &current_acts {
            let d = a.activity_date.naive_utc().date();
            let entry = by_day_map.entry(d).or_insert((0.0, 0.0));
            let amt = a
                .amount
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0)
                .abs();
            if INCOME_TYPES.contains(&a.effective_type()) {
                entry.0 += amt;
            } else if OUTFLOW_TYPES.contains(&a.effective_type()) {
                entry.1 += amt;
            }
        }
        let mut by_day: Vec<DayBucket> = by_day_map
            .into_iter()
            .map(|(d, (income, outflow))| DayBucket {
                date: format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day()),
                income,
                outflow,
            })
            .collect();
        by_day.sort_by(|a, b| a.date.cmp(&b.date));

        // Category breakdown — fetch assignments for the activities in scope
        let mut spending_acc: HashMap<(String, String), (f64, usize)> = HashMap::new();
        let mut income_acc: HashMap<(String, String), (f64, usize)> = HashMap::new();
        // (date, taxonomy_id, category_id) → (amount, count)
        let mut by_day_cat_acc: HashMap<(String, String, String), (f64, usize)> = HashMap::new();
        for a in &current_acts {
            let amt = a
                .amount
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0)
                .abs();
            let day = a.activity_date.naive_utc().date();
            let day_str = format!("{:04}-{:02}-{:02}", day.year(), day.month(), day.day());
            let assignments = self.assignment_repo.list_for_activity(&a.id).await?;
            let is_income = INCOME_TYPES.contains(&a.effective_type());
            let is_outflow = OUTFLOW_TYPES.contains(&a.effective_type());
            for asg in assignments {
                let bucket = if asg.taxonomy_id == SPENDING_TAXONOMY && is_outflow {
                    Some(&mut spending_acc)
                } else if asg.taxonomy_id == INCOME_TAXONOMY && is_income {
                    Some(&mut income_acc)
                } else {
                    None
                };
                if let Some(b) = bucket {
                    let entry = b
                        .entry((asg.taxonomy_id.clone(), asg.category_id.clone()))
                        .or_insert((0.0, 0));
                    entry.0 += amt;
                    entry.1 += 1;
                    // Same activity → same (day, taxonomy, category) bucket.
                    let dc = by_day_cat_acc
                        .entry((
                            day_str.clone(),
                            asg.taxonomy_id.clone(),
                            asg.category_id.clone(),
                        ))
                        .or_insert((0.0, 0));
                    dc.0 += amt;
                    dc.1 += 1;
                }
            }
        }

        let mut spending_breakdown: Vec<CategoryBreakdownRow> = spending_acc
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

fn summarize(acts: &[&Activity]) -> PeriodSummary {
    let mut income = 0.0;
    let mut outflow = 0.0;
    for a in acts {
        let amt = a
            .amount
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
            .unwrap_or(0.0)
            .abs();
        if INCOME_TYPES.contains(&a.effective_type()) {
            income += amt;
        } else if OUTFLOW_TYPES.contains(&a.effective_type()) {
            outflow += amt;
        }
    }
    PeriodSummary {
        income,
        outflow,
        net: income - outflow,
        count: acts.len(),
    }
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
            .get_activities_by_account_ids(&s.account_ids)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Pre-load assignments per activity in scope (only for outflow + spending taxonomy)
        let mut assign_by_act: HashMap<String, Option<String>> = HashMap::new();
        for a in &activities {
            if !OUTFLOW_TYPES.contains(&a.effective_type()) {
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
                    if !OUTFLOW_TYPES.contains(&a.effective_type()) {
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
                &currency,
            ));
        }

        Ok(out)
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
    currency: &str,
) -> SpendingSummary {
    let mut by_month: HashMap<String, f64> = HashMap::new();
    let mut by_account: HashMap<String, f64> = HashMap::new();
    let mut by_category: HashMap<String, CategorySpending> = HashMap::new();
    let mut by_subcategory: HashMap<String, SubcategorySpending> = HashMap::new();
    let mut by_month_by_category: HashMap<String, HashMap<String, f64>> = HashMap::new();
    let mut by_month_by_subcategory: HashMap<String, HashMap<String, f64>> = HashMap::new();
    let mut total = 0.0;

    for a in activities {
        let amt = a
            .amount
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
            .unwrap_or(0.0)
            .abs();
        if amt == 0.0 {
            continue;
        }
        total += amt;
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

    let n_months = by_month.len() as f64;
    let monthly_average = if n_months > 0.0 {
        total / n_months
    } else {
        0.0
    };

    SpendingSummary {
        period: period.to_string(),
        by_month,
        by_category,
        by_subcategory,
        by_account,
        by_month_by_category,
        by_month_by_subcategory,
        total_spending: total,
        currency: currency.to_string(),
        monthly_average,
        transaction_count: activities.len(),
        yoy_growth: None,
    }
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
    /// Only WITHDRAWAL/TRANSFER_OUT/FEE activities tagged with `event_id` count toward spend.
    pub async fn event_spending_summaries(
        &self,
        req: EventSummariesRequest,
    ) -> Result<Vec<EventSpendingSummary>> {
        let s = self.settings.get().await?;
        if !s.enabled || s.account_ids.is_empty() {
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
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc));
        let window_end = req
            .end_date
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc));

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
            .get_activities_by_account_ids(&s.account_ids)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let mut by_event: HashMap<String, Vec<Activity>> = HashMap::new();
        for a in activities {
            if let Some(eid) = a.event_id.clone() {
                if !OUTFLOW_TYPES.contains(&a.effective_type()) {
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
                let amt = a
                    .amount
                    .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
                    .unwrap_or(0.0)
                    .abs();
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

            out.push(EventSpendingSummary {
                event_id: ev.event.id,
                event_name: ev.event.name,
                event_type_id: ev.event.event_type_id.clone(),
                event_type_name: ev.event_type_name,
                event_type_color: type_color.get(&ev.event.event_type_id).cloned().flatten(),
                start_date: ev.event.start_date,
                end_date: ev.event.end_date,
                total_spending: total,
                transaction_count: acts.len(),
                currency: currency.clone(),
                by_category,
                daily_spending: daily,
            });
        }

        Ok(out)
    }
}
