use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use wealthfolio_core::activities::ActivityRepositoryTrait;

use super::matcher::{compile_rules, match_compiled};
use super::model::{
    CategorizationRule, NewCategorizationRule, RuleMatchType, UpdateCategorizationRule,
};
use super::presets::{self, ImportPresetResult, RulePresetSummary};
use super::traits::CategorizationRulesRepositoryTrait;
use crate::activity_assignments::{
    ActivityTaxonomyAssignmentService, NewActivityTaxonomyAssignment,
};

pub struct CategorizationRulesService {
    repo: Arc<dyn CategorizationRulesRepositoryTrait>,
    activity_repo: Arc<dyn ActivityRepositoryTrait>,
    assignment_service: Arc<ActivityTaxonomyAssignmentService>,
}

impl CategorizationRulesService {
    pub fn new(
        repo: Arc<dyn CategorizationRulesRepositoryTrait>,
        activity_repo: Arc<dyn ActivityRepositoryTrait>,
        assignment_service: Arc<ActivityTaxonomyAssignmentService>,
    ) -> Self {
        Self {
            repo,
            activity_repo,
            assignment_service,
        }
    }

    pub async fn list(&self) -> Result<Vec<CategorizationRule>> {
        self.repo.list().await
    }
    pub async fn get(&self, id: &str) -> Result<Option<CategorizationRule>> {
        self.repo.get(id).await
    }
    pub async fn create(&self, new_rule: NewCategorizationRule) -> Result<CategorizationRule> {
        self.repo.create(new_rule).await
    }
    pub async fn update(
        &self,
        id: &str,
        patch: UpdateCategorizationRule,
    ) -> Result<CategorizationRule> {
        self.repo.update(id, patch).await
    }
    pub async fn delete(&self, id: &str) -> Result<()> {
        self.repo.delete(id).await
    }

    /// Re-run all rules against existing activities. Returns count of activities
    /// matched by a rule (a rule that fires counts toward the total even when it
    /// has no category target to write — matches the prior count semantics).
    /// Filters to the provided account ids when non-empty (typically the spending accounts).
    ///
    /// `only_uncategorized=true` skips activities that already have any activity-scope
    /// assignment (spending_categories or income_sources). Default safe behavior.
    /// `only_uncategorized=false` overwrites existing rule/ai/history/import-sourced
    /// assignments with the new rule target.
    ///
    /// **Manual categorizations (`source = "manual"`) are always preserved**, in
    /// both modes. A user's explicit choice should never be wiped by a rule re-run.
    pub async fn rerun_all(
        &self,
        account_ids: &[String],
        only_uncategorized: bool,
    ) -> Result<usize> {
        if account_ids.is_empty() {
            return Ok(0);
        }
        let activities = self
            .activity_repo
            .get_activities_by_account_ids(account_ids)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let ids: Vec<String> = activities.iter().map(|a| a.id.clone()).collect();
        let assignments = self.assignment_service.list_for_activities(&ids).await?;
        let skip: std::collections::HashSet<String> = assignments
            .into_iter()
            .filter(|a| a.taxonomy_id == "spending_categories" || a.taxonomy_id == "income_sources")
            .filter(|a| only_uncategorized || a.source == "manual")
            .map(|a| a.activity_id)
            .collect();

        let rules = self.repo.list().await?;
        let compiled = compile_rules(&rules);

        let mut matched_count = 0usize;
        let mut writes: Vec<NewActivityTaxonomyAssignment> = Vec::with_capacity(activities.len());
        for a in &activities {
            if skip.contains(&a.id) {
                continue;
            }
            let notes_raw = a.notes.as_deref().unwrap_or("");
            let notes_upper = notes_raw.to_uppercase();
            let Some(m) = match_compiled(
                &compiled,
                &notes_upper,
                notes_raw,
                a.effective_type(),
                &a.account_id,
            ) else {
                continue;
            };
            matched_count += 1;
            if let (Some(tax_id), Some(cat_id)) =
                (m.rule.taxonomy_id.clone(), m.rule.category_id.clone())
            {
                writes.push(NewActivityTaxonomyAssignment {
                    id: None,
                    activity_id: a.id.clone(),
                    taxonomy_id: tax_id,
                    category_id: cat_id,
                    weight: 10_000,
                    source: "rule".to_string(),
                });
            }
        }

        self.assignment_service.bulk_apply(writes).await?;
        Ok(matched_count)
    }

    /// List the bundled presets, marking which ones the user already has installed
    /// and at what version. Used by the picker UI on the rules page.
    pub async fn list_presets(&self) -> Result<Vec<RulePresetSummary>> {
        let installed_rules = self.repo.list().await?;
        let installed_versions = presets::installed_versions(
            installed_rules
                .iter()
                .map(|r| (&r.preset_id, &r.preset_version)),
        );
        Ok(presets::load_all_presets()
            .into_iter()
            .map(|p| {
                let installed_version = installed_versions.get(&p.preset_id).cloned();
                RulePresetSummary {
                    installed: installed_version.is_some(),
                    installed_version,
                    rule_count: p.rules.len(),
                    preset_id: p.preset_id,
                    preset_version: p.preset_version,
                    name: p.name,
                    description: p.description,
                    language: p.language,
                }
            })
            .collect())
    }

    /// Import a preset's rules into the user's DB. Skips rules already installed
    /// (by `(preset_id, preset_rule_key)`) and rules whose `categoryKey` doesn't
    /// resolve to a seeded category. Idempotent — safe to call repeatedly.
    ///
    /// `category_resolver` maps a category `key` (e.g. "food_groceries") to the
    /// pair `(taxonomy_id, category_id)`. Caller (typically the IPC layer)
    /// builds it from the taxonomy service.
    pub async fn import_preset(
        &self,
        preset_id: &str,
        category_resolver: &HashMap<String, (String, String)>,
    ) -> Result<ImportPresetResult> {
        let preset = presets::load_preset(preset_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown preset: {preset_id}"))?;

        let existing_rules = self.repo.list().await?;
        let installed_keys = presets::installed_rule_keys(
            existing_rules
                .iter()
                .map(|r| (&r.preset_id, &r.preset_rule_key)),
        );

        let mut result = ImportPresetResult {
            preset_id: preset.preset_id.clone(),
            preset_version: preset.preset_version.clone(),
            total: preset.rules.len(),
            ..Default::default()
        };

        for rule in preset.rules {
            if installed_keys.contains(&(preset.preset_id.clone(), rule.key.clone())) {
                result.skipped_existing += 1;
                continue;
            }
            let Some((tax_id, cat_id)) = category_resolver.get(&rule.category_key).cloned() else {
                log::warn!(
                    "Preset '{}' rule '{}' references unknown categoryKey '{}' — skipped",
                    preset.preset_id,
                    rule.key,
                    rule.category_key,
                );
                result.skipped_unknown_category += 1;
                continue;
            };
            self.repo
                .create(NewCategorizationRule {
                    id: None,
                    name: rule.name,
                    pattern: rule.pattern,
                    match_type: RuleMatchType::parse(&rule.match_type),
                    taxonomy_id: Some(tax_id),
                    category_id: Some(cat_id),
                    activity_type: None,
                    priority: rule.priority,
                    is_global: true,
                    account_id: None,
                    preset_id: Some(preset.preset_id.clone()),
                    preset_rule_key: Some(rule.key),
                    preset_version: Some(preset.preset_version.clone()),
                })
                .await?;
            result.added += 1;
        }
        Ok(result)
    }
}
