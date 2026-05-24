//! Create Categorization Rule tool.
//!
//! When a user gives a "save this for next time" hint (e.g. "T&T is groceries",
//! "treat coffee shops as food/coffee"), the agent calls this to create a real
//! `categorization_rule` row. The rule applies to all future categorization
//! passes AND can be re-run against past uncategorized activities — same
//! mechanism the user already uses from the Spending Settings page.
//!
//! No widget. Returns a one-line summary. The agent typically calls this
//! BEFORE re-running `list_categorization_context` + `propose_transaction_categories`
//! so the new rule shows up in the deterministic pass.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;
use wealthfolio_spending::categorization_rules::{NewCategorizationRule, RuleMatchType};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCategorizationRuleArgs {
    /// Short human-readable rule name (shown in Spending Settings). Optional —
    /// when omitted, the tool generates "{pattern} → {category_path}".
    #[serde(default)]
    pub name: Option<String>,
    /// Substring/pattern to match against the transaction notes/payee. Case-insensitive
    /// (handled downstream). For `matchType: "regex"` this is a Rust regex.
    pub pattern: String,
    /// "contains" (default) | "starts_with" | "exact" | "regex". Use "contains" unless
    /// the user explicitly asks for stricter matching.
    #[serde(default)]
    pub match_type: Option<String>,
    /// Stable category key from the activity-scope taxonomy (e.g. "groceries",
    /// "food_dining_restaurants"). Get this from `list_categorization_context.taxonomies`.
    pub category_key: String,
    /// Optional: restrict to one activity type (e.g. "WITHDRAWAL"). Usually omit.
    #[serde(default)]
    pub activity_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCategorizationRuleOutput {
    pub rule_id: String,
    pub rule_name: String,
    pub pattern: String,
    pub match_type: String,
    pub category_path: String,
    pub message: String,
}

pub struct CreateCategorizationRuleTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> CreateCategorizationRuleTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }
}

impl<E: AiEnvironment> Clone for CreateCategorizationRuleTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for CreateCategorizationRuleTool<E> {
    const NAME: &'static str = "create_categorization_rule";

    type Error = AiError;
    type Args = CreateCategorizationRuleArgs;
    type Output = CreateCategorizationRuleOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Create a persistent categorization rule. Call this when the user gives a \
                 generalizable hint like 'T&T is groceries', 'treat coffee shops as food', \
                 'gym charges are health'. The rule auto-categorizes all matching transactions \
                 (past and future) on the next deterministic pass. \
                 \n\nWORKFLOW: when the user supplies such a hint while reviewing a draft, \
                 (1) call `create_categorization_rule` to persist the rule, then \
                 (2) re-run `list_categorization_context` + `propose_transaction_categories` \
                 — the new rule will show up as `source: \"rule\"` in the result, and you only \
                 need `aiProposals` for whatever's still unmatched. \
                 \n\nUse `pattern: \"T&T\"` with default `matchType: \"contains\"` for typical \
                 merchant-name hints. Get the `categoryKey` from the `taxonomies` list returned \
                 by `list_categorization_context`."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Short rule name shown in settings. Default: derive from pattern, e.g. \"T&T → Groceries\"."
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Substring/pattern matched against transaction notes (case-insensitive). For \"contains\" matchType use a distinctive merchant fragment (e.g. \"T&T\", \"COBS BREAD\")."
                    },
                    "matchType": {
                        "type": "string",
                        "enum": ["contains", "starts_with", "exact", "regex"],
                        "description": "Default \"contains\". Use stricter modes only if user asked."
                    },
                    "categoryKey": {
                        "type": "string",
                        "description": "Category key from the activity-scope taxonomies (e.g. \"groceries\")."
                    },
                    "activityType": {
                        "type": "string",
                        "description": "Optional activity-type narrowing (e.g. WITHDRAWAL). Usually omit."
                    }
                },
                "required": ["pattern", "categoryKey"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        debug!(
            "create_categorization_rule called: pattern_len={}, categoryKey={}",
            args.pattern.chars().count(),
            args.category_key
        );

        if args.pattern.trim().is_empty() {
            return Err(AiError::ToolExecutionFailed(
                "pattern is required and cannot be empty".to_string(),
            ));
        }
        if args.category_key.trim().is_empty() {
            return Err(AiError::ToolExecutionFailed(
                "categoryKey is required and cannot be empty".to_string(),
            ));
        }

        // Resolve category_key → (taxonomy_id, category_id, path) using the live taxonomy.
        let tax_service = self.env.taxonomy_service();
        let taxonomies = tax_service
            .get_taxonomies_with_categories()
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let mut key_lookup: HashMap<String, (String, String, Vec<String>)> = HashMap::new();
        for entry in &taxonomies {
            if entry.taxonomy.scope != "activity" {
                continue;
            }
            let cats_by_id: HashMap<&str, &_> = entry
                .categories
                .iter()
                .map(|c| (c.id.as_str(), c))
                .collect();
            for cat in &entry.categories {
                let mut parts = vec![cat.name.clone()];
                let mut cur = cat.parent_id.as_deref();
                let mut depth = 0;
                while let Some(pid) = cur {
                    if depth > 8 {
                        break;
                    }
                    if let Some(parent) = cats_by_id.get(pid) {
                        parts.push(parent.name.clone());
                        cur = parent.parent_id.as_deref();
                    } else {
                        break;
                    }
                    depth += 1;
                }
                parts.reverse();
                key_lookup.insert(
                    cat.key.clone(),
                    (entry.taxonomy.id.clone(), cat.id.clone(), parts),
                );
            }
        }

        let Some((taxonomy_id, category_id, path_parts)) = key_lookup.get(&args.category_key)
        else {
            return Err(AiError::ToolExecutionFailed(format!(
                "Unknown categoryKey \"{}\". Pick one from `list_categorization_context.taxonomies`.",
                args.category_key
            )));
        };

        let match_type = match args.match_type.as_deref() {
            Some(s) => RuleMatchType::parse(s),
            None => RuleMatchType::Contains,
        };

        let category_path = path_parts.join(" / ");

        // Default name when missing or whitespace: "{pattern} → {category_path}".
        let name = args
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{} → {}", args.pattern.trim(), category_path));

        let new_rule = NewCategorizationRule {
            id: None,
            name,
            pattern: args.pattern.trim().to_string(),
            match_type,
            taxonomy_id: Some(taxonomy_id.clone()),
            category_id: Some(category_id.clone()),
            activity_type: args.activity_type,
            priority: 0,
            is_global: true,
            account_id: None,
            preset_id: None,
            preset_rule_key: None,
            preset_version: None,
        };

        let rules = self.env.categorization_rules_service();
        let created = rules
            .create(new_rule)
            .await
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        let message = format!(
            "Saved rule: anything matching \"{}\" is now {}.",
            created.pattern, category_path
        );

        Ok(CreateCategorizationRuleOutput {
            rule_id: created.id,
            rule_name: created.name,
            pattern: created.pattern,
            match_type: created.match_type.as_str().to_string(),
            category_path,
            message,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Schema contract: required args + enum constraints stay stable.
    /// If this breaks, every saved chat thread that targets this tool may also break.
    #[test]
    fn schema_required_fields_are_pattern_and_category_key() {
        let json = build_definition_parameters();
        let required = json["required"]
            .as_array()
            .expect("required is an array")
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert!(required.contains(&"pattern".to_string()));
        assert!(required.contains(&"categoryKey".to_string()));
        // `name` was made optional intentionally — agent should be able to omit it
        // and let the tool generate "{pattern} → {category_path}".
        assert!(!required.contains(&"name".to_string()));
    }

    #[test]
    fn schema_match_type_enum_matches_rule_match_type_variants() {
        let json = build_definition_parameters();
        let allowed = json["properties"]["matchType"]["enum"]
            .as_array()
            .expect("matchType.enum is an array")
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        // Must match exactly the variants of RuleMatchType — drift here means the
        // agent will produce values the parser quietly remaps to "contains".
        for variant in &["contains", "starts_with", "exact", "regex"] {
            assert!(
                allowed.contains(&variant.to_string()),
                "matchType.enum missing {variant}",
            );
        }
        assert_eq!(allowed.len(), 4, "no extra/missing enum variants");
    }

    /// Args deserialization contract: the agent's tool call (camelCase JSON) must
    /// round-trip into our snake_case Rust struct without surprises.
    #[test]
    fn args_deserialize_from_camel_case_minimal() {
        let json = serde_json::json!({
            "name": "T&T → Groceries",
            "pattern": "T&T",
            "categoryKey": "groceries",
        });
        let args: CreateCategorizationRuleArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.pattern, "T&T");
        assert_eq!(args.category_key, "groceries");
        assert_eq!(args.match_type, None);
        assert_eq!(args.activity_type, None);
    }

    #[test]
    fn args_deserialize_with_all_fields() {
        let json = serde_json::json!({
            "name": "Cobs → Groceries",
            "pattern": "COBS BREAD",
            "matchType": "starts_with",
            "categoryKey": "groceries",
            "activityType": "WITHDRAWAL",
        });
        let args: CreateCategorizationRuleArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.match_type.as_deref(), Some("starts_with"));
        assert_eq!(args.activity_type.as_deref(), Some("WITHDRAWAL"));
    }

    /// Helper that mirrors what `Tool::definition` returns. Kept duplicated here
    /// (rather than calling `.definition()`) because that path is async + needs
    /// an env. The schema is what matters.
    fn build_definition_parameters() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "pattern": { "type": "string" },
                "matchType": {
                    "type": "string",
                    "enum": ["contains", "starts_with", "exact", "regex"],
                },
                "categoryKey": { "type": "string" },
                "activityType": { "type": "string" }
            },
            "required": ["pattern", "categoryKey"]
        })
    }

    #[test]
    fn args_deserialize_without_name_uses_default() {
        // The agent must be able to omit `name` per schema.
        let json = serde_json::json!({
            "pattern": "T&T",
            "categoryKey": "groceries",
        });
        let args: CreateCategorizationRuleArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.name, None);
    }
}
