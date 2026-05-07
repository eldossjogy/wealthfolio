//! Rule-matching algorithm. Ported semantics from PR #494's category_rules matcher.

use regex::Regex;

use super::model::{CategorizationRule, RuleMatchType};

#[derive(Debug, Clone)]
pub struct RuleMatch<'r> {
    pub rule: &'r CategorizationRule,
}

/// Returns the highest-priority rule that matches the given activity attributes.
/// `notes` is the merchant/payee string (we use the existing `notes` column).
/// `account_id` and `activity_type` are used for narrowing (account_id-scoped
/// rules and activity_type-narrowed rules).
pub fn match_rules<'r>(
    rules: &'r [CategorizationRule],
    notes: &str,
    activity_type: &str,
    account_id: &str,
) -> Option<RuleMatch<'r>> {
    let normalized = notes.to_uppercase();
    let mut best: Option<&CategorizationRule> = None;

    for rule in rules {
        // Account scope check
        if !rule.is_global {
            match &rule.account_id {
                Some(rule_acc) if rule_acc == account_id => {}
                _ => continue,
            }
        }

        // Activity-type narrowing
        if let Some(rt) = &rule.activity_type {
            if rt != activity_type {
                continue;
            }
        }

        // Pattern match
        let pattern = rule.pattern.to_uppercase();
        let matched = match rule.match_type {
            RuleMatchType::Contains => normalized.contains(&pattern),
            RuleMatchType::StartsWith => normalized.starts_with(&pattern),
            RuleMatchType::Exact => normalized == pattern,
            RuleMatchType::Regex => Regex::new(&rule.pattern)
                .ok()
                .is_some_and(|re| re.is_match(notes)),
        };

        if !matched {
            continue;
        }

        match best {
            None => best = Some(rule),
            Some(current) if rule.priority > current.priority => best = Some(rule),
            _ => {}
        }
    }

    best.map(|rule| RuleMatch { rule })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn rule(name: &str, pattern: &str, mt: RuleMatchType, prio: i32) -> CategorizationRule {
        CategorizationRule {
            id: name.to_string(),
            name: name.to_string(),
            pattern: pattern.to_string(),
            match_type: mt,
            taxonomy_id: Some("spending_categories".to_string()),
            category_id: Some("cat_food".to_string()),
            activity_type: None,
            priority: prio,
            is_global: true,
            account_id: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        }
    }

    #[test]
    fn contains_matches_case_insensitive() {
        let rules = vec![rule("amazon", "AMAZON", RuleMatchType::Contains, 0)];
        let m = match_rules(&rules, "amazon order #123", "WITHDRAWAL", "acct1").unwrap();
        assert_eq!(m.rule.id, "amazon");
    }

    #[test]
    fn higher_priority_wins() {
        let rules = vec![
            rule("a", "FOO", RuleMatchType::Contains, 1),
            rule("b", "FOO", RuleMatchType::Contains, 5),
        ];
        let m = match_rules(&rules, "FOO BAR", "WITHDRAWAL", "acct1").unwrap();
        assert_eq!(m.rule.id, "b");
    }

    #[test]
    fn account_scoped_rule_skipped_for_other_account() {
        let mut r = rule("scoped", "FOO", RuleMatchType::Contains, 10);
        r.is_global = false;
        r.account_id = Some("acct-other".to_string());
        let rules = vec![r];
        let m = match_rules(&rules, "FOO BAR", "WITHDRAWAL", "acct1");
        assert!(m.is_none());
    }
}
