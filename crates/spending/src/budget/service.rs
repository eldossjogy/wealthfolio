use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use uuid::Uuid;
use wealthfolio_core::accounts::{
    account_supports_purpose, AccountPurpose, AccountRepositoryTrait,
};
use wealthfolio_core::activities::ActivityRepositoryTrait;
use wealthfolio_core::taxonomies::{Category, TaxonomyServiceTrait};

use super::model::{
    BudgetCategoryRow, BudgetGroup, BudgetGroupRow, BudgetRolloverSetting,
    BudgetRolloverTargetType, BudgetSnapshot, BudgetSnapshotComputed, BudgetSnapshotState,
    BudgetTarget, BudgetTargetType, BudgetTotals, NewBudgetGroup, NewBudgetGroupAssignment,
    NewBudgetRolloverSetting, NewBudgetTarget, UpdateBudgetGroup,
};
use super::traits::BudgetRepositoryTrait;
use crate::activity_assignments::ActivityTaxonomyAssignmentRepositoryTrait;
use crate::activity_classification::{activity_abs_amount, classify_activity};
use crate::settings::SpendingSettingsService;

const SPENDING_TAXONOMY: &str = "spending_categories";
const INCOME_TAXONOMY: &str = "income_sources";
const DEFAULT_PERIOD_KEY: &str = "default";
const OTHER_GROUP_KEY: &str = "other";

#[derive(Clone, Copy)]
struct DefaultGroup {
    id: &'static str,
    name: &'static str,
    key: &'static str,
    color: &'static str,
    icon: &'static str,
    sort_order: i32,
}

const DEFAULT_GROUPS: [DefaultGroup; 6] = [
    DefaultGroup {
        id: "budget_group_needs",
        name: "Needs",
        key: "needs",
        color: "#4A90A4",
        icon: "Home",
        sort_order: 1,
    },
    DefaultGroup {
        id: "budget_group_wants",
        name: "Wants",
        key: "wants",
        color: "#9B59B6",
        icon: "Sparkles",
        sort_order: 2,
    },
    DefaultGroup {
        id: "budget_group_savings",
        name: "Savings",
        key: "savings",
        color: "#27AE60",
        icon: "PiggyBank",
        sort_order: 3,
    },
    DefaultGroup {
        id: "budget_group_giving",
        name: "Giving",
        key: "giving",
        color: "#E74C3C",
        icon: "Gift",
        sort_order: 4,
    },
    DefaultGroup {
        id: "budget_group_personal",
        name: "Personal",
        key: "personal",
        color: "#1ABC9C",
        icon: "User",
        sort_order: 5,
    },
    DefaultGroup {
        id: "budget_group_other",
        name: "Other",
        key: "other",
        color: "#7F8C8D",
        icon: "MoreHorizontal",
        sort_order: 99,
    },
];

const DEFAULT_ASSIGNMENTS: [(&str, &str); 14] = [
    ("cat_housing", "needs"),
    ("cat_groceries", "needs"),
    ("cat_transport", "needs"),
    ("cat_health", "needs"),
    ("cat_bills", "needs"),
    ("cat_fees", "needs"),
    ("cat_education", "needs"),
    ("cat_food", "wants"),
    ("cat_shopping", "wants"),
    ("cat_entertainment", "wants"),
    ("cat_travel", "wants"),
    ("cat_gifts", "giving"),
    ("cat_personal", "personal"),
    ("cat_other_expense", "other"),
];

type MonthActuals = HashMap<(String, String), f64>;

pub struct BudgetService {
    repo: Arc<dyn BudgetRepositoryTrait>,
    activity_repo: Arc<dyn ActivityRepositoryTrait>,
    account_repo: Arc<dyn AccountRepositoryTrait>,
    assignment_repo: Arc<dyn ActivityTaxonomyAssignmentRepositoryTrait>,
    spending_settings: Arc<SpendingSettingsService>,
    taxonomy_service: Arc<dyn TaxonomyServiceTrait>,
}

impl BudgetService {
    pub fn new(
        repo: Arc<dyn BudgetRepositoryTrait>,
        activity_repo: Arc<dyn ActivityRepositoryTrait>,
        account_repo: Arc<dyn AccountRepositoryTrait>,
        assignment_repo: Arc<dyn ActivityTaxonomyAssignmentRepositoryTrait>,
        spending_settings: Arc<SpendingSettingsService>,
        taxonomy_service: Arc<dyn TaxonomyServiceTrait>,
    ) -> Self {
        Self {
            repo,
            activity_repo,
            account_repo,
            assignment_repo,
            spending_settings,
            taxonomy_service,
        }
    }

    pub async fn get(&self, period_key: Option<String>, currency: &str) -> Result<BudgetSnapshot> {
        self.ensure_system_groups().await?;
        let period_key = normalize_period_key(period_key)?;
        let groups = self.repo.list_groups().await?;
        let assignments = self.repo.list_group_assignments().await?;
        let targets = self.repo.list_targets().await?;
        let rollover_settings = self.repo.list_rollover_settings().await?;

        let spending_categories = self.taxonomy_categories(SPENDING_TAXONOMY)?;
        let income_categories = self.taxonomy_categories(INCOME_TAXONOMY)?;
        let spending_category_meta = category_meta(&spending_categories);
        let income_meta = category_meta(&income_categories);
        let top_spending_categories = top_level_categories(&spending_categories);
        let top_income_categories = top_level_categories(&income_categories);

        let is_month_view = period_key != DEFAULT_PERIOD_KEY;
        let actuals_by_month = if is_month_view {
            let earliest_rollover_month = rollover_settings
                .iter()
                .filter(|s| s.enabled && s.start_month <= period_key)
                .map(|s| s.start_month.clone())
                .min()
                .unwrap_or_else(|| period_key.clone());
            self.actuals_by_month(
                &earliest_rollover_month,
                &period_key,
                &spending_category_meta,
                &income_meta,
            )
            .await?
        } else {
            HashMap::new()
        };
        let current_actuals = actuals_by_month
            .get(&period_key)
            .cloned()
            .unwrap_or_default();

        let target_index = TargetIndex::new(&targets);
        let rollover_index = RolloverIndex::new(&rollover_settings);
        let group_by_key: HashMap<String, BudgetGroup> =
            groups.iter().map(|g| (g.key.clone(), g.clone())).collect();
        let other_group_id = group_by_key
            .get(OTHER_GROUP_KEY)
            .map(|g| g.id.clone())
            .ok_or_else(|| anyhow!("Missing Other budget group"))?;

        let assignment_by_category: HashMap<String, String> = assignments
            .iter()
            .filter(|a| a.taxonomy_id == SPENDING_TAXONOMY)
            .map(|a| (a.category_id.clone(), a.group_id.clone()))
            .collect();
        let group_for_category = |category_id: &str| -> String {
            resolve_group_for_category(
                category_id,
                &assignment_by_category,
                &spending_category_meta,
                &other_group_id,
            )
        };

        let mut rows_by_group: HashMap<String, Vec<BudgetCategoryRow>> = HashMap::new();
        for category in &top_spending_categories {
            let group_id = group_for_category(&category.id);
            let actual = current_actuals
                .get(&(SPENDING_TAXONOMY.to_string(), category.id.clone()))
                .copied()
                .unwrap_or(0.0);
            let target =
                target_index.effective_category(&period_key, SPENDING_TAXONOMY, &category.id);
            let rollover = is_month_view
                .then(|| rollover_index.category(SPENDING_TAXONOMY, &category.id))
                .flatten();
            let (rollover_in, rollover_out, remaining) = if let Some(setting) = rollover {
                compute_rollover_for_month(
                    setting,
                    &period_key,
                    |month| target_index.effective_category(month, SPENDING_TAXONOMY, &category.id),
                    |month| {
                        actuals_by_month
                            .get(month)
                            .and_then(|m| {
                                m.get(&(SPENDING_TAXONOMY.to_string(), category.id.clone()))
                            })
                            .copied()
                            .unwrap_or(0.0)
                    },
                )
            } else {
                (0.0, 0.0, target - actual)
            };
            rows_by_group
                .entry(group_id.clone())
                .or_default()
                .push(BudgetCategoryRow {
                    taxonomy_id: SPENDING_TAXONOMY.to_string(),
                    category_id: category.id.clone(),
                    group_id: Some(group_id),
                    parent_id: category.parent_id.clone(),
                    name: category.name.clone(),
                    color: Some(category.color.clone()),
                    icon: category.icon.clone(),
                    target,
                    actual,
                    rollover_in,
                    rollover_out,
                    remaining,
                    overspent: remaining < 0.0,
                    has_default_target: target_index
                        .has_default_category(SPENDING_TAXONOMY, &category.id),
                    has_month_override: target_index.has_month_category(
                        &period_key,
                        SPENDING_TAXONOMY,
                        &category.id,
                    ),
                    rollover_enabled: rollover.is_some(),
                });
        }

        let mut group_rows = Vec::with_capacity(groups.len());
        for group in &groups {
            let mut categories = rows_by_group.remove(&group.id).unwrap_or_default();
            categories.sort_by(|a, b| a.name.cmp(&b.name));
            let category_target_total = categories.iter().map(|c| c.target).sum::<f64>();
            let actual = categories.iter().map(|c| c.actual).sum::<f64>();
            let buffer = target_index.effective_group_buffer(&period_key, &group.id);
            let planned_total = category_target_total + buffer;
            let rollover = is_month_view
                .then(|| rollover_index.group(&group.id))
                .flatten();
            let (rollover_in, rollover_out, remaining) = if let Some(setting) = rollover {
                compute_rollover_for_month(
                    setting,
                    &period_key,
                    |month| {
                        let child_total = categories
                            .iter()
                            .map(|c| {
                                target_index.effective_category(
                                    month,
                                    SPENDING_TAXONOMY,
                                    &c.category_id,
                                )
                            })
                            .sum::<f64>();
                        child_total + target_index.effective_group_buffer(month, &group.id)
                    },
                    |month| {
                        categories
                            .iter()
                            .map(|c| {
                                actuals_by_month
                                    .get(month)
                                    .and_then(|m| {
                                        m.get(&(
                                            SPENDING_TAXONOMY.to_string(),
                                            c.category_id.clone(),
                                        ))
                                    })
                                    .copied()
                                    .unwrap_or(0.0)
                            })
                            .sum::<f64>()
                    },
                )
            } else {
                (0.0, 0.0, planned_total - actual)
            };
            group_rows.push(BudgetGroupRow {
                group: group.clone(),
                category_target_total,
                buffer,
                planned_total,
                actual,
                rollover_in,
                rollover_out,
                remaining,
                overspent: remaining < 0.0,
                rollover_enabled: rollover.is_some(),
                categories,
            });
        }
        group_rows.sort_by(|a, b| {
            a.group
                .sort_order
                .cmp(&b.group.sort_order)
                .then(a.group.name.cmp(&b.group.name))
        });

        let mut income_rows = Vec::with_capacity(top_income_categories.len());
        for category in &top_income_categories {
            let actual = current_actuals
                .get(&(INCOME_TAXONOMY.to_string(), category.id.clone()))
                .copied()
                .unwrap_or(0.0);
            let target =
                target_index.effective_category(&period_key, INCOME_TAXONOMY, &category.id);
            income_rows.push(BudgetCategoryRow {
                taxonomy_id: INCOME_TAXONOMY.to_string(),
                category_id: category.id.clone(),
                group_id: None,
                parent_id: category.parent_id.clone(),
                name: category.name.clone(),
                color: Some(category.color.clone()),
                icon: category.icon.clone(),
                target,
                actual,
                rollover_in: 0.0,
                rollover_out: 0.0,
                remaining: target - actual,
                overspent: false,
                has_default_target: target_index
                    .has_default_category(INCOME_TAXONOMY, &category.id),
                has_month_override: target_index.has_month_category(
                    &period_key,
                    INCOME_TAXONOMY,
                    &category.id,
                ),
                rollover_enabled: false,
            });
        }
        income_rows.sort_by(|a, b| a.name.cmp(&b.name));

        let totals = BudgetTotals {
            spending_planned: group_rows.iter().map(|g| g.planned_total).sum(),
            spending_actual: group_rows.iter().map(|g| g.actual).sum(),
            spending_remaining: group_rows.iter().map(|g| g.remaining).sum(),
            income_planned: income_rows.iter().map(|r| r.target).sum(),
            income_actual: income_rows.iter().map(|r| r.actual).sum(),
            group_buffer: group_rows.iter().map(|g| g.buffer).sum(),
            rollover_in: group_rows.iter().map(|g| g.rollover_in).sum(),
            rollover_out: group_rows.iter().map(|g| g.rollover_out).sum(),
            overspent_count: group_rows.iter().filter(|g| g.overspent).count()
                + group_rows
                    .iter()
                    .flat_map(|g| &g.categories)
                    .filter(|c| c.overspent)
                    .count(),
        };

        Ok(BudgetSnapshot {
            state: BudgetSnapshotState {
                groups,
                group_assignments: assignments,
                targets,
                rollover_settings,
            },
            computed: BudgetSnapshotComputed {
                currency: currency.to_string(),
                period_key,
                group_rows,
                ungrouped_rows: vec![],
                income_rows,
                totals,
            },
        })
    }

    pub async fn create_group(
        &self,
        input: NewBudgetGroup,
        period_key: Option<String>,
        currency: &str,
    ) -> Result<BudgetSnapshot> {
        self.repo
            .create_group(NewBudgetGroup {
                id: input.id,
                key: Some(format!("custom_{}", Uuid::new_v4())),
                name: input.name,
                color: input.color,
                icon: input.icon,
                sort_order: input.sort_order,
                is_system: false,
            })
            .await?;
        self.get(period_key, currency).await
    }

    pub async fn update_group(
        &self,
        id: &str,
        patch: UpdateBudgetGroup,
        period_key: Option<String>,
        currency: &str,
    ) -> Result<BudgetSnapshot> {
        self.repo.update_group(id, patch).await?;
        self.get(period_key, currency).await
    }

    pub async fn delete_group(
        &self,
        id: &str,
        reassign_to_group_id: &str,
        period_key: Option<String>,
        currency: &str,
    ) -> Result<BudgetSnapshot> {
        let groups = self.repo.list_groups().await?;
        groups
            .iter()
            .find(|g| g.id == id)
            .ok_or_else(|| anyhow!("Budget group not found"))?;
        if reassign_to_group_id == id {
            return Err(anyhow!(
                "Cannot reassign categories to the group being deleted"
            ));
        }
        if !groups.iter().any(|g| g.id == reassign_to_group_id) {
            return Err(anyhow!("Reassignment budget group not found"));
        }
        let assignments = self.repo.list_group_assignments().await?;
        let reassignments = assignments
            .into_iter()
            .filter(|a| a.group_id == id)
            .map(|a| NewBudgetGroupAssignment {
                id: Some(a.id),
                group_id: reassign_to_group_id.to_string(),
                taxonomy_id: a.taxonomy_id,
                category_id: a.category_id,
            })
            .collect::<Vec<_>>();
        self.repo.upsert_group_assignments(reassignments).await?;
        self.repo.delete_group(id).await?;
        self.get(period_key, currency).await
    }

    pub async fn assign_category_to_group(
        &self,
        category_id: String,
        group_id: String,
        period_key: Option<String>,
        currency: &str,
    ) -> Result<BudgetSnapshot> {
        self.repo
            .upsert_group_assignment(NewBudgetGroupAssignment {
                id: None,
                group_id,
                taxonomy_id: SPENDING_TAXONOMY.to_string(),
                category_id,
            })
            .await?;
        self.get(period_key, currency).await
    }

    pub async fn reset_groups(
        &self,
        period_key: Option<String>,
        currency: &str,
    ) -> Result<BudgetSnapshot> {
        let groups = self
            .repo
            .upsert_system_groups(default_group_inputs())
            .await?;
        let group_by_key: HashMap<String, String> =
            groups.into_iter().map(|g| (g.key, g.id)).collect();
        self.repo
            .upsert_group_assignments(default_assignment_inputs(&group_by_key))
            .await?;
        self.get(period_key, currency).await
    }

    pub async fn upsert_target(
        &self,
        target: NewBudgetTarget,
        period_key: Option<String>,
        currency: &str,
    ) -> Result<BudgetSnapshot> {
        validate_period_key(&target.period_key)?;
        self.repo.upsert_target(target).await?;
        self.get(period_key, currency).await
    }

    pub async fn delete_target(
        &self,
        id: &str,
        period_key: Option<String>,
        currency: &str,
    ) -> Result<BudgetSnapshot> {
        self.repo.delete_target(id).await?;
        self.get(period_key, currency).await
    }

    pub async fn upsert_rollover_setting(
        &self,
        setting: NewBudgetRolloverSetting,
        period_key: Option<String>,
        currency: &str,
    ) -> Result<BudgetSnapshot> {
        validate_month_key(&setting.start_month)?;
        match setting.target_type {
            BudgetRolloverTargetType::Group if setting.enabled => {
                let group_id = setting
                    .group_id
                    .as_ref()
                    .ok_or_else(|| anyhow!("Group rollover requires groupId"))?;
                let categories = self.categories_for_group(group_id).await?;
                self.repo
                    .disable_category_rollovers(SPENDING_TAXONOMY, &categories)
                    .await?;
            }
            BudgetRolloverTargetType::Category if setting.enabled => {
                let category_id = setting
                    .category_id
                    .as_ref()
                    .ok_or_else(|| anyhow!("Category rollover requires categoryId"))?;
                let group_id = self.group_id_for_category(category_id).await?;
                let group_rollover_enabled = self
                    .repo
                    .list_rollover_settings()
                    .await?
                    .into_iter()
                    .any(|r| {
                        r.enabled
                            && matches!(r.target_type, BudgetRolloverTargetType::Group)
                            && r.group_id.as_deref() == Some(group_id.as_str())
                    });
                if group_rollover_enabled {
                    return Err(anyhow!(
                        "Disable group rollover before enabling category rollover"
                    ));
                }
            }
            _ => {}
        }
        self.repo.upsert_rollover_setting(setting).await?;
        self.get(period_key, currency).await
    }

    pub async fn delete_rollover_setting(
        &self,
        id: &str,
        period_key: Option<String>,
        currency: &str,
    ) -> Result<BudgetSnapshot> {
        self.repo.delete_rollover_setting(id).await?;
        self.get(period_key, currency).await
    }

    pub async fn copy_period_targets(
        &self,
        source_period_key: &str,
        target_period_key: &str,
        overwrite: bool,
        currency: &str,
    ) -> Result<BudgetSnapshot> {
        validate_period_key(source_period_key)?;
        validate_month_key(target_period_key)?;
        if source_period_key == target_period_key {
            return Err(anyhow!("Source and target months must differ"));
        }
        self.repo
            .copy_period_targets(source_period_key, target_period_key, overwrite)
            .await?;
        self.get(Some(target_period_key.to_string()), currency)
            .await
    }

    async fn ensure_system_groups(&self) -> Result<()> {
        let existing_keys: HashSet<String> = self
            .repo
            .list_groups()
            .await?
            .into_iter()
            .map(|g| g.key)
            .collect();
        let missing = default_group_inputs()
            .into_iter()
            .filter(|g| {
                g.key
                    .as_ref()
                    .is_some_and(|key| !existing_keys.contains(key))
            })
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            self.repo.upsert_system_groups(missing).await?;
        }
        Ok(())
    }

    fn taxonomy_categories(&self, taxonomy_id: &str) -> Result<Vec<Category>> {
        Ok(self
            .taxonomy_service
            .get_taxonomy(taxonomy_id)?
            .map(|t| t.categories)
            .unwrap_or_default())
    }

    async fn categories_for_group(&self, group_id: &str) -> Result<Vec<String>> {
        let assignments = self.repo.list_group_assignments().await?;
        let categories = self.taxonomy_categories(SPENDING_TAXONOMY)?;
        let meta = category_meta(&categories);
        let groups = self.repo.list_groups().await?;
        let other_group_id = groups
            .into_iter()
            .find(|g| g.key == OTHER_GROUP_KEY)
            .map(|g| g.id)
            .ok_or_else(|| anyhow!("Missing Other budget group"))?;
        let assignment_by_category = assignments
            .into_iter()
            .filter(|a| a.taxonomy_id == SPENDING_TAXONOMY)
            .map(|a| (a.category_id, a.group_id))
            .collect::<HashMap<_, _>>();

        Ok(top_level_categories(&categories)
            .into_iter()
            .filter(|category| {
                resolve_group_for_category(
                    &category.id,
                    &assignment_by_category,
                    &meta,
                    &other_group_id,
                ) == group_id
            })
            .map(|category| category.id)
            .collect())
    }

    async fn group_id_for_category(&self, category_id: &str) -> Result<String> {
        let assignments = self.repo.list_group_assignments().await?;
        let categories = self.taxonomy_categories(SPENDING_TAXONOMY)?;
        let meta = category_meta(&categories);
        let groups = self.repo.list_groups().await?;
        let other_group_id = groups
            .iter()
            .find(|g| g.key == OTHER_GROUP_KEY)
            .map(|g| g.id.clone())
            .ok_or_else(|| anyhow!("Missing Other budget group"))?;
        let assignment_by_category = assignments
            .into_iter()
            .map(|a| (a.category_id, a.group_id))
            .collect::<HashMap<_, _>>();
        Ok(resolve_group_for_category(
            category_id,
            &assignment_by_category,
            &meta,
            &other_group_id,
        ))
    }

    async fn actuals_by_month(
        &self,
        start_period: &str,
        end_period: &str,
        spending_meta: &HashMap<String, Category>,
        income_meta: &HashMap<String, Category>,
    ) -> Result<HashMap<String, MonthActuals>> {
        let settings = self.spending_settings.get().await?;
        if !settings.enabled || settings.account_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let accounts = self
            .account_repo
            .list(None, Some(false), Some(&settings.account_ids))
            .map_err(|e| anyhow!(e.to_string()))?;
        let account_types: HashMap<String, String> = accounts
            .into_iter()
            .filter(|a| account_supports_purpose(&a.account_type, AccountPurpose::Spending))
            .map(|a| (a.id, a.account_type))
            .collect();
        if account_types.is_empty() {
            return Ok(HashMap::new());
        }
        let account_ids = account_types.keys().cloned().collect::<Vec<_>>();
        let start = month_start(start_period)?;
        let end = month_end(end_period)?;
        let activities = self
            .activity_repo
            .get_activities_by_account_ids(&account_ids)
            .map_err(|e| anyhow!(e.to_string()))?
            .into_iter()
            .filter(|a| a.activity_date >= start && a.activity_date <= end)
            .collect::<Vec<_>>();
        let activity_ids = activities.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
        let assignments = self
            .assignment_repo
            .list_for_activities(&activity_ids)
            .await?;
        let mut assignments_by_activity: HashMap<String, Vec<_>> = HashMap::new();
        for assignment in assignments {
            assignments_by_activity
                .entry(assignment.activity_id.clone())
                .or_default()
                .push(assignment);
        }

        let mut actuals: HashMap<String, MonthActuals> = HashMap::new();
        for activity in activities {
            let Some(account_type) = account_types.get(&activity.account_id) else {
                continue;
            };
            let classification = classify_activity(&activity, account_type);
            let amount = activity_abs_amount(&activity);
            let spending = classification.spending_amount(amount);
            let income = classification.income_amount(amount);
            if spending == 0.0 && income == 0.0 {
                continue;
            }
            let month = period_key_for_date(activity.activity_date);
            let month_actuals = actuals.entry(month).or_default();
            for assignment in assignments_by_activity
                .get(&activity.id)
                .into_iter()
                .flatten()
            {
                if assignment.taxonomy_id == SPENDING_TAXONOMY && spending != 0.0 {
                    let top_id = top_category_id(&assignment.category_id, spending_meta);
                    *month_actuals
                        .entry((SPENDING_TAXONOMY.to_string(), top_id))
                        .or_insert(0.0) += spending;
                } else if assignment.taxonomy_id == INCOME_TAXONOMY && income != 0.0 {
                    let top_id = top_category_id(&assignment.category_id, income_meta);
                    *month_actuals
                        .entry((INCOME_TAXONOMY.to_string(), top_id))
                        .or_insert(0.0) += income;
                }
            }
        }
        Ok(actuals)
    }
}

pub(crate) struct TargetIndex<'a> {
    targets: &'a [BudgetTarget],
}

impl<'a> TargetIndex<'a> {
    pub(crate) fn new(targets: &'a [BudgetTarget]) -> Self {
        Self { targets }
    }

    pub(crate) fn effective_category(
        &self,
        period: &str,
        taxonomy_id: &str,
        category_id: &str,
    ) -> f64 {
        self.month_category(period, taxonomy_id, category_id)
            .or_else(|| self.default_category(taxonomy_id, category_id))
            .map(parse_amount)
            .unwrap_or(0.0)
    }

    pub(crate) fn effective_group_buffer(&self, period: &str, group_id: &str) -> f64 {
        self.month_group_buffer(period, group_id)
            .or_else(|| self.default_group_buffer(group_id))
            .map(parse_amount)
            .unwrap_or(0.0)
    }

    pub(crate) fn has_default_category(&self, taxonomy_id: &str, category_id: &str) -> bool {
        self.default_category(taxonomy_id, category_id).is_some()
    }

    pub(crate) fn has_month_category(
        &self,
        period: &str,
        taxonomy_id: &str,
        category_id: &str,
    ) -> bool {
        self.month_category(period, taxonomy_id, category_id)
            .is_some()
    }

    pub(crate) fn has_month_group_buffer(&self, period: &str, group_id: &str) -> bool {
        self.month_group_buffer(period, group_id).is_some()
    }

    fn month_category(&self, period: &str, taxonomy_id: &str, category_id: &str) -> Option<&str> {
        self.targets
            .iter()
            .find(|t| {
                matches!(t.target_type, BudgetTargetType::Category)
                    && t.period_key == period
                    && t.taxonomy_id.as_deref() == Some(taxonomy_id)
                    && t.category_id.as_deref() == Some(category_id)
            })
            .map(|t| t.amount.as_str())
    }

    fn default_category(&self, taxonomy_id: &str, category_id: &str) -> Option<&str> {
        self.month_category(DEFAULT_PERIOD_KEY, taxonomy_id, category_id)
    }

    fn month_group_buffer(&self, period: &str, group_id: &str) -> Option<&str> {
        self.targets
            .iter()
            .find(|t| {
                matches!(t.target_type, BudgetTargetType::GroupBuffer)
                    && t.period_key == period
                    && t.group_id.as_deref() == Some(group_id)
            })
            .map(|t| t.amount.as_str())
    }

    fn default_group_buffer(&self, group_id: &str) -> Option<&str> {
        self.month_group_buffer(DEFAULT_PERIOD_KEY, group_id)
    }
}

struct RolloverIndex<'a> {
    settings: &'a [BudgetRolloverSetting],
}

impl<'a> RolloverIndex<'a> {
    fn new(settings: &'a [BudgetRolloverSetting]) -> Self {
        Self { settings }
    }

    fn category(&self, taxonomy_id: &str, category_id: &str) -> Option<&'a BudgetRolloverSetting> {
        self.settings.iter().find(|s| {
            s.enabled
                && matches!(s.target_type, BudgetRolloverTargetType::Category)
                && s.taxonomy_id.as_deref() == Some(taxonomy_id)
                && s.category_id.as_deref() == Some(category_id)
        })
    }

    fn group(&self, group_id: &str) -> Option<&'a BudgetRolloverSetting> {
        self.settings.iter().find(|s| {
            s.enabled
                && matches!(s.target_type, BudgetRolloverTargetType::Group)
                && s.group_id.as_deref() == Some(group_id)
        })
    }
}

fn compute_rollover_for_month(
    setting: &BudgetRolloverSetting,
    period_key: &str,
    target_for_month: impl Fn(&str) -> f64,
    actual_for_month: impl Fn(&str) -> f64,
) -> (f64, f64, f64) {
    if setting.start_month.as_str() > period_key {
        let target = target_for_month(period_key);
        let actual = actual_for_month(period_key);
        return (0.0, 0.0, target - actual);
    }
    let mut carry = parse_amount(&setting.starting_balance);
    for month in month_keys_between(&setting.start_month, period_key) {
        let rollover_in = carry;
        let target = target_for_month(&month);
        let actual = actual_for_month(&month);
        let rollover_out = rollover_in + target - actual;
        if month == period_key {
            return (rollover_in, rollover_out, rollover_in + target - actual);
        }
        carry = rollover_out;
    }
    (
        0.0,
        0.0,
        target_for_month(period_key) - actual_for_month(period_key),
    )
}

fn default_group_inputs() -> Vec<NewBudgetGroup> {
    DEFAULT_GROUPS
        .iter()
        .map(|g| NewBudgetGroup {
            id: Some(g.id.to_string()),
            name: g.name.to_string(),
            key: Some(g.key.to_string()),
            color: Some(g.color.to_string()),
            icon: Some(g.icon.to_string()),
            sort_order: Some(g.sort_order),
            is_system: true,
        })
        .collect()
}

fn default_assignment_inputs(
    group_by_key: &HashMap<String, String>,
) -> Vec<NewBudgetGroupAssignment> {
    DEFAULT_ASSIGNMENTS
        .iter()
        .filter_map(|(category_id, group_key)| {
            group_by_key
                .get(*group_key)
                .map(|group_id| NewBudgetGroupAssignment {
                    id: Some(format!("bga_{}", category_id)),
                    group_id: group_id.clone(),
                    taxonomy_id: SPENDING_TAXONOMY.to_string(),
                    category_id: (*category_id).to_string(),
                })
        })
        .collect()
}

pub(crate) fn category_meta(categories: &[Category]) -> HashMap<String, Category> {
    categories
        .iter()
        .map(|c| (c.id.clone(), c.clone()))
        .collect()
}

pub(crate) fn top_level_categories(categories: &[Category]) -> Vec<Category> {
    let mut categories = categories
        .iter()
        .filter(|c| c.parent_id.is_none())
        .cloned()
        .collect::<Vec<_>>();
    categories.sort_by(|a, b| a.sort_order.cmp(&b.sort_order).then(a.name.cmp(&b.name)));
    categories
}

pub(crate) fn resolve_group_for_category(
    category_id: &str,
    assignment_by_category: &HashMap<String, String>,
    category_meta: &HashMap<String, Category>,
    other_group_id: &str,
) -> String {
    let mut current = Some(category_id.to_string());
    while let Some(id) = current {
        if let Some(group_id) = assignment_by_category.get(&id) {
            return group_id.clone();
        }
        current = category_meta.get(&id).and_then(|c| c.parent_id.clone());
    }
    other_group_id.to_string()
}

pub(crate) fn top_category_id(category_id: &str, meta: &HashMap<String, Category>) -> String {
    let mut current = category_id.to_string();
    while let Some(parent_id) = meta.get(&current).and_then(|c| c.parent_id.clone()) {
        current = parent_id;
    }
    current
}

fn parse_amount(value: &str) -> f64 {
    value.parse::<f64>().unwrap_or(0.0)
}

fn normalize_period_key(period_key: Option<String>) -> Result<String> {
    match period_key {
        Some(key) if key == DEFAULT_PERIOD_KEY => Ok(key),
        Some(key) => {
            validate_month_key(&key)?;
            Ok(key)
        }
        None => Ok(period_key_for_date(Utc::now())),
    }
}

fn validate_period_key(period_key: &str) -> Result<()> {
    if period_key == DEFAULT_PERIOD_KEY {
        Ok(())
    } else {
        validate_month_key(period_key)
    }
}

fn validate_month_key(period_key: &str) -> Result<()> {
    if period_key.len() != 7 {
        return Err(anyhow!("Invalid budget period key"));
    }
    let year = period_key[0..4].parse::<i32>()?;
    let month = period_key[5..7].parse::<u32>()?;
    if &period_key[4..5] != "-" || !(1..=12).contains(&month) {
        return Err(anyhow!("Invalid budget period key"));
    }
    NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| anyhow!("Invalid budget period key"))?;
    Ok(())
}

fn period_key_for_date(date: DateTime<Utc>) -> String {
    format!("{:04}-{:02}", date.year(), date.month())
}

fn month_start(period_key: &str) -> Result<DateTime<Utc>> {
    let (year, month) = parse_month(period_key)?;
    Ok(Utc
        .with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow!("Invalid budget period key"))?)
}

fn month_end(period_key: &str) -> Result<DateTime<Utc>> {
    let (year, month) = parse_month(period_key)?;
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    Ok(Utc
        .with_ymd_and_hms(next_year, next_month, 1, 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow!("Invalid budget period key"))?
        - chrono::Duration::milliseconds(1))
}

fn month_keys_between(start: &str, end: &str) -> Vec<String> {
    let Ok((mut year, mut month)) = parse_month(start) else {
        return Vec::new();
    };
    let Ok((end_year, end_month)) = parse_month(end) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while year < end_year || (year == end_year && month <= end_month) {
        out.push(format!("{:04}-{:02}", year, month));
        if month == 12 {
            year += 1;
            month = 1;
        } else {
            month += 1;
        }
    }
    out
}

fn parse_month(period_key: &str) -> Result<(i32, u32)> {
    validate_month_key(period_key)?;
    Ok((period_key[0..4].parse()?, period_key[5..7].parse()?))
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime};

    use super::*;

    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    }

    fn target(
        period_key: &str,
        target_type: BudgetTargetType,
        taxonomy_id: Option<&str>,
        category_id: Option<&str>,
        group_id: Option<&str>,
        amount: &str,
    ) -> BudgetTarget {
        BudgetTarget {
            id: format!(
                "{}-{}-{}",
                period_key,
                category_id.or(group_id).unwrap_or("target"),
                amount
            ),
            period_key: period_key.to_string(),
            target_type,
            taxonomy_id: taxonomy_id.map(str::to_string),
            category_id: category_id.map(str::to_string),
            group_id: group_id.map(str::to_string),
            amount: amount.to_string(),
            created_at: ts(),
            updated_at: ts(),
        }
    }

    fn rollover_setting(start_month: &str, starting_balance: &str) -> BudgetRolloverSetting {
        BudgetRolloverSetting {
            id: "rollover".to_string(),
            target_type: BudgetRolloverTargetType::Category,
            taxonomy_id: Some(SPENDING_TAXONOMY.to_string()),
            category_id: Some("cat_groceries".to_string()),
            group_id: None,
            enabled: true,
            start_month: start_month.to_string(),
            starting_balance: starting_balance.to_string(),
            created_at: ts(),
            updated_at: ts(),
        }
    }

    #[test]
    fn seeded_groups_use_savings_label() {
        let names = DEFAULT_GROUPS.iter().map(|g| g.name).collect::<Vec<_>>();

        assert!(names.contains(&"Savings"));
        assert!(!names.contains(&"Saving & Investment"));
        assert!(!names.contains(&"Saving & Investments"));
    }

    #[test]
    fn target_index_uses_sparse_month_overrides_over_defaults() {
        let targets = vec![
            target(
                DEFAULT_PERIOD_KEY,
                BudgetTargetType::Category,
                Some(SPENDING_TAXONOMY),
                Some("cat_groceries"),
                None,
                "200",
            ),
            target(
                "2026-03",
                BudgetTargetType::Category,
                Some(SPENDING_TAXONOMY),
                Some("cat_groceries"),
                None,
                "300",
            ),
            target(
                DEFAULT_PERIOD_KEY,
                BudgetTargetType::GroupBuffer,
                None,
                None,
                Some("budget_group_needs"),
                "500",
            ),
        ];
        let index = TargetIndex::new(&targets);

        assert_eq!(
            index.effective_category("2026-03", SPENDING_TAXONOMY, "cat_groceries"),
            300.0
        );
        assert_eq!(
            index.effective_category("2026-04", SPENDING_TAXONOMY, "cat_groceries"),
            200.0
        );
        assert_eq!(
            index.effective_category("2026-04", SPENDING_TAXONOMY, "cat_travel"),
            0.0
        );
        assert_eq!(
            index.effective_group_buffer("2026-04", "budget_group_needs"),
            500.0
        );
        assert!(index.has_default_category(SPENDING_TAXONOMY, "cat_groceries"));
        assert!(index.has_month_category("2026-03", SPENDING_TAXONOMY, "cat_groceries"));
    }

    #[test]
    fn rollover_ignores_months_before_future_start_month() {
        let setting = rollover_setting("2026-06", "25");

        let (rollover_in, rollover_out, remaining) =
            compute_rollover_for_month(&setting, "2026-05", |_| 100.0, |_| 40.0);

        assert_eq!(rollover_in, 0.0);
        assert_eq!(rollover_out, 0.0);
        assert_eq!(remaining, 60.0);
    }

    #[test]
    fn rollover_recomputes_multi_year_chain_from_start_month() {
        let setting = rollover_setting("2025-01", "10");

        let (rollover_in, rollover_out, remaining) = compute_rollover_for_month(
            &setting,
            "2026-05",
            |_| 100.0,
            |month| match month {
                "2025-01" => 25.0,
                "2026-05" => 40.0,
                _ => 0.0,
            },
        );

        assert_eq!(rollover_in, 1585.0);
        assert_eq!(rollover_out, 1645.0);
        assert_eq!(remaining, 1645.0);
    }

    #[test]
    fn missing_target_with_spending_creates_negative_remaining() {
        let setting = rollover_setting("2026-05", "0");

        let (rollover_in, rollover_out, remaining) =
            compute_rollover_for_month(&setting, "2026-05", |_| 0.0, |_| 30.0);

        assert_eq!(rollover_in, 0.0);
        assert_eq!(rollover_out, -30.0);
        assert_eq!(remaining, -30.0);
    }

    #[test]
    fn refund_month_increases_remaining() {
        let setting = rollover_setting("2026-05", "0");

        let (rollover_in, rollover_out, remaining) =
            compute_rollover_for_month(&setting, "2026-05", |_| 100.0, |_| -25.0);

        assert_eq!(rollover_in, 0.0);
        assert_eq!(rollover_out, 125.0);
        assert_eq!(remaining, 125.0);
    }

    #[test]
    fn month_keys_are_strictly_validated() {
        assert!(validate_period_key(DEFAULT_PERIOD_KEY).is_ok());
        assert!(validate_period_key("2026-05").is_ok());
        assert!(validate_period_key("2026-13").is_err());
        assert!(validate_period_key("2026-5").is_err());
    }
}
