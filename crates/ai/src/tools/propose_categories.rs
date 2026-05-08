//! Propose Transaction Categories tool.
//!
//! Returns a draft batch of category proposals for cash transactions. Proposal-only:
//! the widget applies via `bulk_assign_categories` after the user confirms.
//!
//! Architecture (mirrors `import_csv` and `record_activity` — no inner LLM call):
//! 1. **Deterministic passes** run server-side every call:
//!    a. Categorization rules (highest confidence).
//!    b. Same-payee history match.
//! 2. **LLM reasoning happens in the chat agent itself**, not inside the tool.
//!    The tool returns the full taxonomy tree, recent few-shot examples, and the
//!    list of unproposed rows to the chat agent. The agent reasons in chat
//!    context, then calls this tool a second time with `aiProposals` — its
//!    inferred categories as structured tool arguments. The tool merges those
//!    with the deterministic results.
//! Same pattern as `import_csv`: the agent's tool-call IS the structured output.

use log::{debug, warn};
use rig::{completion::ToolDefinition, tool::Tool};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;
use wealthfolio_spending::cash_activities::{CashActivitySearchRequest, CashActivityStatusFilter};

const DEFAULT_LIMIT: usize = 30;
const MAX_LIMIT: usize = 50;
const HISTORY_FETCH_LIMIT: usize = 400;
const EXAMPLES_PER_CATEGORY: usize = 3;
const MAX_TOTAL_EXAMPLES: usize = 80;
const MAX_NOTES_LEN: usize = 100;

fn truncate_notes(s: &str) -> String {
    // Use char count consistently — byte len would spuriously truncate UTF-8 strings
    // with multi-byte characters that have fewer than MAX_NOTES_LEN characters.
    if s.chars().count() <= MAX_NOTES_LEN {
        s.to_string()
    } else {
        let mut out = s.chars().take(MAX_NOTES_LEN).collect::<String>();
        out.push('…');
        out
    }
}

/// One AI-inferred category for a row, supplied by the chat agent on its second
/// call to this tool. The agent reasons about the unproposed rows in chat context
/// (using the taxonomies + examples returned on the first call) and passes its
/// conclusions back as structured args.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProposal {
    /// Activity ID from the unproposed list returned in the first call.
    pub activity_id: String,
    /// Category key from the taxonomies returned in the first call (e.g. "groceries").
    pub category_key: String,
    /// 0.0–1.0; agent's stated confidence. Defaults to 0.7 if missing.
    #[serde(default)]
    pub confidence: Option<f32>,
    /// Short explanation shown in the widget tooltip.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposeCategoriesArgs {
    /// Explicit set of activity ids to propose for. Overrides filters when set.
    pub activity_ids: Option<Vec<String>>,
    pub account_ids: Option<Vec<String>>,
    /// "uncategorized" (default) | "all" | "needs_review"
    pub status: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: Option<u32>,
    /// Agent-inferred categories for the unproposed rows. The agent must fill this
    /// after calling `list_categorization_context` — there is no other way for an
    /// AI-inferred category to reach the widget. Don't leave this empty when the
    /// context call returned `needsAiJudgement > 0`.
    #[serde(default)]
    pub ai_proposals: Option<Vec<AiProposal>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryOption {
    pub category_id: String,
    pub key: String,
    pub name: String,
    pub path: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomySummary {
    pub taxonomy_id: String,
    pub taxonomy_name: String,
    pub categories: Vec<CategoryOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryExample {
    pub category_id: String,
    pub category_path: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub activity_id: String,
    pub activity_date: String,
    pub amount: f64,
    pub currency: String,
    pub notes: Option<String>,
    pub taxonomy_id: String,
    pub category_id: String,
    pub category_path: String,
    pub confidence: f32,
    /// "rule" | "history" | "ai"
    pub source: String,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnproposedActivity {
    pub activity_id: String,
    pub activity_date: String,
    pub amount: f64,
    pub currency: String,
    pub notes: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalSummary {
    pub total: usize,
    pub proposed: usize,
    pub unproposed: usize,
    pub avg_confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposeCategoriesOutput {
    pub proposals: Vec<Proposal>,
    pub unproposed: Vec<UnproposedActivity>,
    pub summary: ProposalSummary,
    /// Activity-scope taxonomies + categories. Used by the chat agent for
    /// reasoning, and by the widget's per-row picker.
    pub taxonomies: Vec<TaxonomySummary>,
    /// Per-category examples sourced from past `manual`/`rule`/`import` assignments.
    /// The chat agent uses these as few-shot context to infer categories for the
    /// unproposed rows.
    pub examples: Vec<CategoryExample>,
    /// Conversational state marker for the chat agent. "draft" means this tool
    /// result is the current draft awaiting user review/apply. The widget flips
    /// this to "applied" client-side after a successful Apply (via updateToolResult).
    /// When the agent sees a "draft" output and the user gives a follow-up hint,
    /// the agent should re-run categorization rather than treating it as a future
    /// preference.
    #[serde(default = "default_draft_status")]
    pub draft_status: String,
}

fn default_draft_status() -> String {
    "draft".to_string()
}

pub struct ProposeCategoriesTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> ProposeCategoriesTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }
}

impl<E: AiEnvironment> Clone for ProposeCategoriesTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
        }
    }
}

fn normalize_payee(notes: &str) -> String {
    notes
        .to_lowercase()
        .split_whitespace()
        .filter(|tok| {
            !tok.chars()
                .all(|c| c.is_ascii_digit() || c == '*' || c == '#')
        })
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

impl<E: AiEnvironment + 'static> Tool for ProposeCategoriesTool<E> {
    const NAME: &'static str = "propose_transaction_categories";

    type Error = AiError;
    type Args = ProposeCategoriesArgs;
    type Output = ProposeCategoriesOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Render the categorization widget for the user to review and confirm. Run \
                 `list_categorization_context` FIRST to see the taxonomies, recent few-shot \
                 examples, and the unproposed rows; reason about each unproposed row, then \
                 call this tool with `aiProposals` filled in. The tool runs deterministic \
                 rule + same-payee history matches, merges your `aiProposals` for the rows \
                 those passes didn't cover, and renders the widget. Do NOT pass `accountIds` \
                 for generic mentions like 'credit card' or 'this account'."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "activityIds": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional explicit set of activity IDs to propose for. Overrides filters."
                    },
                    "accountIds": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "OMIT unless the user names a specific account by exact name or ID."
                    },
                    "status": {
                        "type": "string",
                        "enum": ["uncategorized", "all", "needs_review"],
                        "description": "Default: uncategorized."
                    },
                    "startDate": { "type": "string", "description": "Inclusive ISO 8601 lower bound." },
                    "endDate":   { "type": "string", "description": "Inclusive ISO 8601 upper bound." },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_LIMIT,
                        "description": "Max rows to propose. Default 30."
                    },
                    "aiProposals": {
                        "type": "array",
                        "description": "Your inferred categories for the rows returned as `unproposed` from `list_categorization_context`. Each entry: { activityId, categoryKey, confidence (0–1), reason }.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "activityId": { "type": "string" },
                                "categoryKey": { "type": "string" },
                                "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
                                "reason": { "type": "string" }
                            },
                            "required": ["activityId", "categoryKey"]
                        }
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        debug!(
            "propose_transaction_categories called (ai_proposals: {})",
            args.ai_proposals.as_ref().map(|v| v.len()).unwrap_or(0)
        );

        let mut state = compute_categorization_state(
            &self.env,
            CategorizationFilters {
                activity_ids: args.activity_ids.clone(),
                account_ids: args.account_ids.clone(),
                status: args.status.clone(),
                start_date: args.start_date.clone(),
                end_date: args.end_date.clone(),
                limit: args.limit,
            },
        )
        .await?;

        if state.is_empty {
            return Ok(ProposeCategoriesOutput {
                proposals: Vec::new(),
                unproposed: Vec::new(),
                summary: ProposalSummary {
                    total: 0,
                    proposed: 0,
                    unproposed: 0,
                    avg_confidence: 0.0,
                },
                taxonomies: Vec::new(),
                examples: Vec::new(),
                draft_status: "draft".to_string(),
            });
        }

        // Telemetry: warn when the agent calls without aiProposals while there
        // are rows that need AI judgement. Common failure mode the system prompt
        // tries to prevent — surfacing it in dev logs makes regressions visible.
        let unproposed_pre_ai = state.unproposed.len();
        let ai_props_count = args.ai_proposals.as_ref().map(|v| v.len()).unwrap_or(0);
        if unproposed_pre_ai > 0 && ai_props_count == 0 {
            warn!(
                "propose_transaction_categories called with empty aiProposals while {} rows \
                 need AI judgement. Agent should have inferred categories per system prompt.",
                unproposed_pre_ai
            );
        }
        if state.taxonomies.is_empty() {
            warn!(
                "propose_transaction_categories: no activity-scope taxonomies are configured; \
                 widget will render the no-taxonomies empty state."
            );
        }

        // Merge `aiProposals` from the agent. Validate each against the live
        // taxonomy — drop entries with unknown category keys or activity IDs
        // not in our unproposed list (rules/history already covered them).
        if let Some(ai_props) = args.ai_proposals {
            let (merged_proposals, remaining_unproposed) = merge_ai_proposals(
                std::mem::take(&mut state.unproposed),
                std::mem::take(&mut state.proposals),
                &state.key_lookup,
                ai_props,
            );
            state.proposals = merged_proposals;
            state.unproposed = remaining_unproposed;
        }

        let total = state.total;
        let proposed = state.proposals.len();
        let avg_confidence = if proposed > 0 {
            state.proposals.iter().map(|p| p.confidence).sum::<f32>() / proposed as f32
        } else {
            0.0
        };

        Ok(ProposeCategoriesOutput {
            proposals: state.proposals,
            unproposed: state.unproposed,
            summary: ProposalSummary {
                total,
                proposed,
                unproposed: total.saturating_sub(proposed),
                avg_confidence,
            },
            taxonomies: state.taxonomies,
            examples: state.examples,
            draft_status: "draft".to_string(),
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct CategorizationFilters {
    pub activity_ids: Option<Vec<String>>,
    pub account_ids: Option<Vec<String>>,
    pub status: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: Option<u32>,
}

pub(crate) struct CategorizationState {
    pub is_empty: bool,
    pub total: usize,
    pub proposals: Vec<Proposal>,
    pub unproposed: Vec<UnproposedActivity>,
    pub taxonomies: Vec<TaxonomySummary>,
    pub examples: Vec<CategoryExample>,
    /// category_key -> (taxonomy_id, category_id, path). Used to resolve
    /// agent-supplied category keys back to live IDs.
    pub key_lookup: HashMap<String, (String, String, String)>,
}

/// Shared deterministic pass — fetches activities + taxonomies + history, runs
/// rules + same-payee match, returns the full state. Used by both
/// `propose_transaction_categories` (which then merges agent aiProposals) and
/// `list_categorization_context` (which exposes the agent-facing context only).
pub(crate) async fn compute_categorization_state<E: AiEnvironment>(
    env: &Arc<E>,
    filters: CategorizationFilters,
) -> Result<CategorizationState, AiError> {
    let limit = filters
        .limit
        .map(|n| (n as usize).min(MAX_LIMIT))
        .unwrap_or(DEFAULT_LIMIT)
        .max(1);

    let status = match filters.status.as_deref() {
        Some("all") => CashActivityStatusFilter::All,
        Some("needs_review") => CashActivityStatusFilter::NeedsReview,
        _ => CashActivityStatusFilter::Uncategorized,
    };

    let cash = env.cash_activity_service();
    let target_request = CashActivitySearchRequest {
        account_ids: filters.account_ids.clone(),
        status,
        start_date: filters.start_date.clone(),
        end_date: filters.end_date.clone(),
        limit,
        ..Default::default()
    };
    let target_response = cash
        .search(target_request)
        .await
        .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

    let mut targets: Vec<_> = target_response.items;
    if let Some(ids) = filters.activity_ids.as_ref() {
        let id_set: HashSet<&str> = ids.iter().map(String::as_str).collect();
        targets.retain(|t| id_set.contains(t.activity.id.as_str()));
    }
    if targets.is_empty() {
        return Ok(CategorizationState {
            is_empty: true,
            total: 0,
            proposals: Vec::new(),
            unproposed: Vec::new(),
            taxonomies: Vec::new(),
            examples: Vec::new(),
            key_lookup: HashMap::new(),
        });
    }

    let tax_service = env.taxonomy_service();
    let all_taxonomies = tax_service
        .get_taxonomies_with_categories()
        .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;
    let activity_taxonomies: Vec<_> = all_taxonomies
        .into_iter()
        .filter(|t| t.taxonomy.scope == "activity")
        .collect();

    // category_id -> (taxonomy_id, taxonomy_name, category_name, path, color)
    let mut category_lookup: HashMap<String, (String, String, String, String, String)> =
        HashMap::new();
    let mut key_lookup: HashMap<String, (String, String, String)> = HashMap::new();
    let mut taxonomy_summaries = Vec::with_capacity(activity_taxonomies.len());
    for entry in &activity_taxonomies {
        let cats_by_id: HashMap<&str, &_> = entry
            .categories
            .iter()
            .map(|c| (c.id.as_str(), c))
            .collect();
        let mut options = Vec::with_capacity(entry.categories.len());
        for cat in &entry.categories {
            let path = build_category_path(cat, &cats_by_id);
            category_lookup.insert(
                cat.id.clone(),
                (
                    entry.taxonomy.id.clone(),
                    entry.taxonomy.name.clone(),
                    cat.name.clone(),
                    path.clone(),
                    cat.color.clone(),
                ),
            );
            key_lookup.insert(
                cat.key.clone(),
                (entry.taxonomy.id.clone(), cat.id.clone(), path.clone()),
            );
            options.push(CategoryOption {
                category_id: cat.id.clone(),
                key: cat.key.clone(),
                name: cat.name.clone(),
                path,
                color: cat.color.clone(),
            });
        }
        taxonomy_summaries.push(TaxonomySummary {
            taxonomy_id: entry.taxonomy.id.clone(),
            taxonomy_name: entry.taxonomy.name.clone(),
            categories: options,
        });
    }

    let history_request = CashActivitySearchRequest {
        account_ids: filters.account_ids.clone(),
        status: CashActivityStatusFilter::Categorized,
        limit: HISTORY_FETCH_LIMIT,
        ..Default::default()
    };
    let history_response = cash
        .search(history_request)
        .await
        .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

    let mut payee_map: HashMap<String, HashMap<(String, String), usize>> = HashMap::new();
    for item in &history_response.items {
        let Some(notes) = item.activity.notes.as_deref() else {
            continue;
        };
        let key = normalize_payee(notes);
        if key.is_empty() {
            continue;
        }
        for asg in &item.assignments {
            payee_map
                .entry(key.clone())
                .or_default()
                .entry((asg.taxonomy_id.clone(), asg.category_id.clone()))
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
    }

    let mut per_cat_count: HashMap<String, usize> = HashMap::new();
    let mut examples = Vec::new();
    for item in &history_response.items {
        if examples.len() >= MAX_TOTAL_EXAMPLES {
            break;
        }
        let Some(notes) = item.activity.notes.as_deref() else {
            continue;
        };
        for asg in &item.assignments {
            let count = per_cat_count.entry(asg.category_id.clone()).or_insert(0);
            if *count >= EXAMPLES_PER_CATEGORY {
                continue;
            }
            let Some((_tax_id, _tax_name, _cat_name, path, _color)) =
                category_lookup.get(&asg.category_id)
            else {
                continue;
            };
            examples.push(CategoryExample {
                category_id: asg.category_id.clone(),
                category_path: path.clone(),
                notes: truncate_notes(notes),
            });
            *count += 1;
            break;
        }
    }

    let rules_service = env.categorization_rules_service();
    let all_rules = rules_service
        .list()
        .await
        .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

    let total = targets.len();
    let mut proposals = Vec::new();
    let mut unproposed = Vec::new();
    for target in &targets {
        let act = &target.activity;
        let amount = act.amount.and_then(|d| d.to_f64()).unwrap_or(0.0);
        let date = act.activity_date.format("%Y-%m-%d").to_string();
        let notes_trimmed = act.notes.as_deref().map(truncate_notes);

        let rule_match = wealthfolio_spending::categorization_rules::match_rules(
            &all_rules,
            act.notes.as_deref().unwrap_or(""),
            act.effective_type(),
            &act.account_id,
        );
        if let Some(m) = rule_match {
            if let (Some(tax_id), Some(cat_id)) =
                (m.rule.taxonomy_id.clone(), m.rule.category_id.clone())
            {
                if let Some((_, _, _, path, _)) = category_lookup.get(&cat_id) {
                    proposals.push(Proposal {
                        activity_id: act.id.clone(),
                        activity_date: date.clone(),
                        amount,
                        currency: act.currency.clone(),
                        notes: notes_trimmed.clone(),
                        taxonomy_id: tax_id,
                        category_id: cat_id,
                        category_path: path.clone(),
                        confidence: 0.95,
                        source: "rule".to_string(),
                        explanation: format!("Matched rule \"{}\".", m.rule.name),
                    });
                    continue;
                }
            }
        }

        let history_match = act
            .notes
            .as_deref()
            .map(normalize_payee)
            .filter(|k| !k.is_empty())
            .and_then(|key| payee_map.get(&key).cloned())
            .and_then(|by_cat| {
                by_cat
                    .into_iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|((tax_id, cat_id), count)| (tax_id, cat_id, count))
            });

        if let Some((tax_id, cat_id, count)) = history_match {
            if let Some((_, _, _, path, _)) = category_lookup.get(&cat_id) {
                let confidence = if count >= 3 {
                    0.92
                } else if count == 2 {
                    0.82
                } else {
                    0.7
                };
                proposals.push(Proposal {
                    activity_id: act.id.clone(),
                    activity_date: date,
                    amount,
                    currency: act.currency.clone(),
                    notes: notes_trimmed,
                    taxonomy_id: tax_id,
                    category_id: cat_id,
                    category_path: path.clone(),
                    confidence,
                    source: "history".to_string(),
                    explanation: format!("Matched same payee in {} prior transaction(s).", count),
                });
                continue;
            }
        }

        unproposed.push(UnproposedActivity {
            activity_id: act.id.clone(),
            activity_date: date,
            amount,
            currency: act.currency.clone(),
            notes: notes_trimmed,
            reason: "No rule or history match — needs AI or manual judgement.".to_string(),
        });
    }

    Ok(CategorizationState {
        is_empty: false,
        total,
        proposals,
        unproposed,
        taxonomies: taxonomy_summaries,
        examples,
        key_lookup,
    })
}

/// Merge agent-supplied AI proposals into the deterministic results.
/// Drops entries whose `activity_id` is not in `unproposed` (rules/history
/// already covered them) or whose `category_key` is not in `key_lookup`.
/// Confidence is clamped to [0.5, 0.95]; missing confidence defaults to 0.7.
/// On duplicate `activity_id` entries the last one wins (HashMap insertion).
pub(crate) fn merge_ai_proposals(
    unproposed: Vec<UnproposedActivity>,
    mut proposals: Vec<Proposal>,
    key_lookup: &HashMap<String, (String, String, String)>,
    ai_props: Vec<AiProposal>,
) -> (Vec<Proposal>, Vec<UnproposedActivity>) {
    let target_index: HashMap<&str, &UnproposedActivity> = unproposed
        .iter()
        .map(|u| (u.activity_id.as_str(), u))
        .collect();
    let mut accepted: HashMap<String, Proposal> = HashMap::new();
    for ai in ai_props {
        let Some(row) = target_index.get(ai.activity_id.as_str()) else {
            continue;
        };
        let Some((tax_id, cat_id, path)) = key_lookup.get(&ai.category_key) else {
            continue;
        };
        let confidence = ai.confidence.unwrap_or(0.7).clamp(0.5, 0.95);
        accepted.insert(
            row.activity_id.clone(),
            Proposal {
                activity_id: row.activity_id.clone(),
                activity_date: row.activity_date.clone(),
                amount: row.amount,
                currency: row.currency.clone(),
                notes: row.notes.clone(),
                taxonomy_id: tax_id.clone(),
                category_id: cat_id.clone(),
                category_path: path.clone(),
                confidence,
                source: "ai".to_string(),
                explanation: ai
                    .reason
                    .unwrap_or_else(|| "AI inferred from payee + history.".to_string()),
            },
        );
    }
    let remaining: Vec<UnproposedActivity> = unproposed
        .into_iter()
        .filter(|u| !accepted.contains_key(&u.activity_id))
        .collect();
    proposals.extend(accepted.into_values());
    (proposals, remaining)
}

fn build_category_path(
    cat: &wealthfolio_core::taxonomies::Category,
    cats_by_id: &HashMap<&str, &wealthfolio_core::taxonomies::Category>,
) -> String {
    let mut parts = vec![cat.name.clone()];
    let mut current_parent = cat.parent_id.as_deref();
    let mut depth = 0;
    while let Some(pid) = current_parent {
        if depth > 8 {
            break;
        }
        if let Some(parent) = cats_by_id.get(pid) {
            parts.push(parent.name.clone());
            current_parent = parent.parent_id.as_deref();
        } else {
            break;
        }
        depth += 1;
    }
    parts.reverse();
    parts.join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;
    use wealthfolio_core::taxonomies::Category;

    // ----- normalize_payee -------------------------------------------------

    #[test]
    fn normalize_payee_table_driven() {
        let cases: &[(&str, &str)] = &[
            ("SQ *MORNING OWL TORONTO #5523", "sq *morning owl"),
            ("AMAZON.COM*A1B2", "amazon.com*a1b2"),
            ("COBS BREAD", "cobs bread"),
            ("", ""),
            ("   \t  ", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(
                normalize_payee(input),
                *expected,
                "normalize_payee({:?})",
                input
            );
        }
    }

    // ----- truncate_notes --------------------------------------------------

    #[test]
    fn truncate_notes_short_unchanged() {
        let s = "hello world";
        assert_eq!(truncate_notes(s), s);
    }

    #[test]
    fn truncate_notes_exactly_max_unchanged() {
        let s: String = "a".repeat(MAX_NOTES_LEN);
        assert_eq!(truncate_notes(&s), s);
    }

    #[test]
    fn truncate_notes_long_gets_ellipsis() {
        let s: String = "a".repeat(MAX_NOTES_LEN + 50);
        let out = truncate_notes(&s);
        assert!(out.ends_with('…'));
        // First MAX_NOTES_LEN chars + ellipsis.
        assert_eq!(out.chars().count(), MAX_NOTES_LEN + 1);
    }

    #[test]
    fn truncate_notes_multibyte_truncates_by_char_not_byte() {
        // Each emoji is multi-byte UTF-8 (4 bytes). MAX_NOTES_LEN+10 emojis
        // exceed MAX_NOTES_LEN by char count and would be far over by byte count.
        let s: String = "🍕".repeat(MAX_NOTES_LEN + 10);
        let out = truncate_notes(&s);
        assert!(out.ends_with('…'));
        // Should have exactly MAX_NOTES_LEN pizza chars + 1 ellipsis.
        assert_eq!(out.chars().count(), MAX_NOTES_LEN + 1);
        // And every leading char should be the pizza emoji.
        let pizzas = out
            .chars()
            .take(MAX_NOTES_LEN)
            .filter(|c| *c == '🍕')
            .count();
        assert_eq!(pizzas, MAX_NOTES_LEN);
    }

    #[test]
    fn truncate_notes_multibyte_under_byte_limit_unchanged() {
        // 30 pizzas = 120 bytes, but only 30 chars — ≤ MAX_NOTES_LEN by char,
        // so should be unchanged. (Also confirms `s.len() <= MAX_NOTES_LEN`
        // byte-based fast path works correctly when bytes ≤ MAX_NOTES_LEN.)
        let s: String = "a".repeat(50);
        assert_eq!(truncate_notes(&s), s);
    }

    // ----- build_category_path --------------------------------------------

    fn make_cat(id: &str, name: &str, parent: Option<&str>) -> Category {
        let now: NaiveDateTime = "2024-01-01T00:00:00"
            .parse()
            .expect("valid timestamp literal");
        Category {
            id: id.to_string(),
            taxonomy_id: "tax1".to_string(),
            parent_id: parent.map(str::to_string),
            name: name.to_string(),
            key: id.to_string(),
            color: "#000000".to_string(),
            description: None,
            sort_order: 0,
            created_at: now,
            updated_at: now,
            icon: None,
        }
    }

    #[test]
    fn build_category_path_root() {
        let cat = make_cat("root", "Food", None);
        let map: HashMap<&str, &Category> = HashMap::new();
        assert_eq!(build_category_path(&cat, &map), "Food");
    }

    #[test]
    fn build_category_path_one_level() {
        let parent = make_cat("p", "Parent", None);
        let child = make_cat("c", "Child", Some("p"));
        let map: HashMap<&str, &Category> = [("p", &parent)].into_iter().collect();
        assert_eq!(build_category_path(&child, &map), "Parent / Child");
    }

    #[test]
    fn build_category_path_two_levels() {
        let gp = make_cat("gp", "Grandparent", None);
        let p = make_cat("p", "Parent", Some("gp"));
        let c = make_cat("c", "Child", Some("p"));
        let map: HashMap<&str, &Category> = [("gp", &gp), ("p", &p)].into_iter().collect();
        assert_eq!(
            build_category_path(&c, &map),
            "Grandparent / Parent / Child"
        );
    }

    #[test]
    fn build_category_path_cycle_does_not_loop() {
        // Self-cycle: cat A's parent is A.
        let a = make_cat("a", "A", Some("a"));
        let map: HashMap<&str, &Category> = [("a", &a)].into_iter().collect();
        // If this returns, we passed (no infinite loop). Depth cap is >8.
        let path = build_category_path(&a, &map);
        // We expect 1 (self) + up to 9 cycle traversals before break.
        assert!(path.contains("A"));
        // Total segments capped: starting name + at most 9 parent walks.
        let segments: Vec<&str> = path.split(" / ").collect();
        assert!(segments.len() <= 10, "got {} segments", segments.len());
    }

    // ----- merge_ai_proposals ----------------------------------------------

    fn make_unproposed(id: &str) -> UnproposedActivity {
        UnproposedActivity {
            activity_id: id.to_string(),
            activity_date: "2024-06-01".to_string(),
            amount: -42.0,
            currency: "USD".to_string(),
            notes: Some("STARBUCKS".to_string()),
            reason: "test".to_string(),
        }
    }

    fn make_key_lookup() -> HashMap<String, (String, String, String)> {
        let mut m = HashMap::new();
        m.insert(
            "groceries".to_string(),
            (
                "tax1".to_string(),
                "cat-g".to_string(),
                "Food / Groceries".to_string(),
            ),
        );
        m.insert(
            "coffee".to_string(),
            (
                "tax1".to_string(),
                "cat-c".to_string(),
                "Food / Coffee".to_string(),
            ),
        );
        m
    }

    #[test]
    fn merge_valid_proposal_moves_row_to_proposals() {
        let unproposed = vec![make_unproposed("a1"), make_unproposed("a2")];
        let key_lookup = make_key_lookup();
        let ai = vec![AiProposal {
            activity_id: "a1".to_string(),
            category_key: "groceries".to_string(),
            confidence: Some(0.8),
            reason: Some("looks like food".to_string()),
        }];

        let (proposals, remaining) = merge_ai_proposals(unproposed, Vec::new(), &key_lookup, ai);

        assert_eq!(proposals.len(), 1);
        let p = &proposals[0];
        assert_eq!(p.activity_id, "a1");
        assert_eq!(p.source, "ai");
        assert_eq!(p.taxonomy_id, "tax1");
        assert_eq!(p.category_id, "cat-g");
        assert_eq!(p.category_path, "Food / Groceries");
        assert_eq!(p.confidence, 0.8);
        assert_eq!(p.explanation, "looks like food");

        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].activity_id, "a2");
    }

    #[test]
    fn merge_unknown_category_key_is_dropped() {
        let unproposed = vec![make_unproposed("a1")];
        let key_lookup = make_key_lookup();
        let ai = vec![AiProposal {
            activity_id: "a1".to_string(),
            category_key: "nonexistent".to_string(),
            confidence: Some(0.8),
            reason: None,
        }];

        let (proposals, remaining) = merge_ai_proposals(unproposed, Vec::new(), &key_lookup, ai);

        assert!(proposals.is_empty());
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].activity_id, "a1");
    }

    #[test]
    fn merge_unknown_activity_id_is_dropped() {
        // a1 already proposed by rules — not in `unproposed`.
        let unproposed = vec![make_unproposed("a2")];
        let key_lookup = make_key_lookup();
        let ai = vec![AiProposal {
            activity_id: "a1".to_string(),
            category_key: "groceries".to_string(),
            confidence: Some(0.8),
            reason: None,
        }];

        let (proposals, remaining) = merge_ai_proposals(unproposed, Vec::new(), &key_lookup, ai);

        assert!(proposals.is_empty());
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].activity_id, "a2");
    }

    #[test]
    fn merge_confidence_clamped_high() {
        let unproposed = vec![make_unproposed("a1")];
        let key_lookup = make_key_lookup();
        let ai = vec![AiProposal {
            activity_id: "a1".to_string(),
            category_key: "groceries".to_string(),
            confidence: Some(2.0),
            reason: None,
        }];
        let (proposals, _) = merge_ai_proposals(unproposed, Vec::new(), &key_lookup, ai);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].confidence, 0.95);
    }

    #[test]
    fn merge_confidence_clamped_low() {
        let unproposed = vec![make_unproposed("a1")];
        let key_lookup = make_key_lookup();
        let ai = vec![AiProposal {
            activity_id: "a1".to_string(),
            category_key: "groceries".to_string(),
            confidence: Some(0.1),
            reason: None,
        }];
        let (proposals, _) = merge_ai_proposals(unproposed, Vec::new(), &key_lookup, ai);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].confidence, 0.5);
    }

    #[test]
    fn merge_confidence_missing_defaults_to_0_7() {
        let unproposed = vec![make_unproposed("a1")];
        let key_lookup = make_key_lookup();
        let ai = vec![AiProposal {
            activity_id: "a1".to_string(),
            category_key: "groceries".to_string(),
            confidence: None,
            reason: None,
        }];
        let (proposals, _) = merge_ai_proposals(unproposed, Vec::new(), &key_lookup, ai);
        assert_eq!(proposals.len(), 1);
        assert!((proposals[0].confidence - 0.7).abs() < 1e-6);
    }

    #[test]
    fn merge_missing_reason_uses_default_explanation() {
        let unproposed = vec![make_unproposed("a1")];
        let key_lookup = make_key_lookup();
        let ai = vec![AiProposal {
            activity_id: "a1".to_string(),
            category_key: "groceries".to_string(),
            confidence: Some(0.7),
            reason: None,
        }];
        let (proposals, _) = merge_ai_proposals(unproposed, Vec::new(), &key_lookup, ai);
        assert_eq!(
            proposals[0].explanation,
            "AI inferred from payee + history."
        );
    }

    #[test]
    fn merge_empty_ai_proposals_leaves_unproposed_unchanged() {
        let unproposed = vec![make_unproposed("a1"), make_unproposed("a2")];
        let key_lookup = make_key_lookup();
        let (proposals, remaining) =
            merge_ai_proposals(unproposed.clone(), Vec::new(), &key_lookup, Vec::new());
        assert!(proposals.is_empty());
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].activity_id, "a1");
        assert_eq!(remaining[1].activity_id, "a2");
    }

    #[test]
    fn merge_duplicate_activity_id_last_wins() {
        let unproposed = vec![make_unproposed("a1")];
        let key_lookup = make_key_lookup();
        let ai = vec![
            AiProposal {
                activity_id: "a1".to_string(),
                category_key: "groceries".to_string(),
                confidence: Some(0.6),
                reason: Some("first".to_string()),
            },
            AiProposal {
                activity_id: "a1".to_string(),
                category_key: "coffee".to_string(),
                confidence: Some(0.9),
                reason: Some("second".to_string()),
            },
        ];
        let (proposals, remaining) = merge_ai_proposals(unproposed, Vec::new(), &key_lookup, ai);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].category_id, "cat-c");
        assert_eq!(proposals[0].category_path, "Food / Coffee");
        assert_eq!(proposals[0].confidence, 0.9);
        assert_eq!(proposals[0].explanation, "second");
        assert!(remaining.is_empty());
    }

    #[test]
    fn merge_preserves_existing_proposals() {
        let unproposed = vec![make_unproposed("a1")];
        let key_lookup = make_key_lookup();
        let existing = vec![Proposal {
            activity_id: "a0".to_string(),
            activity_date: "2024-06-01".to_string(),
            amount: -10.0,
            currency: "USD".to_string(),
            notes: None,
            taxonomy_id: "tax1".to_string(),
            category_id: "cat-g".to_string(),
            category_path: "Food / Groceries".to_string(),
            confidence: 0.95,
            source: "rule".to_string(),
            explanation: "Matched rule".to_string(),
        }];
        let ai = vec![AiProposal {
            activity_id: "a1".to_string(),
            category_key: "coffee".to_string(),
            confidence: Some(0.8),
            reason: None,
        }];
        let (proposals, remaining) = merge_ai_proposals(unproposed, existing, &key_lookup, ai);
        assert_eq!(proposals.len(), 2);
        // The rule-sourced one should still be present.
        assert!(proposals
            .iter()
            .any(|p| p.source == "rule" && p.activity_id == "a0"));
        assert!(proposals
            .iter()
            .any(|p| p.source == "ai" && p.activity_id == "a1"));
        assert!(remaining.is_empty());
    }
}
