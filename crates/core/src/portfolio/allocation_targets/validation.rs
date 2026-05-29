use crate::errors::{Error as CoreError, Result as CoreResult, ValidationError};

use super::model::{NewTargetAllocationNode, NewTargetProfile, ScopeType};

fn invalid(msg: &str) -> CoreError {
    CoreError::Validation(ValidationError::InvalidInput(msg.to_string()))
}

pub fn validate_new_profile(input: &NewTargetProfile) -> CoreResult<()> {
    if input.name.trim().is_empty() {
        return Err(invalid("Profile name is required"));
    }
    if matches!(input.scope_type, ScopeType::Account | ScopeType::Portfolio)
        && input.scope_id.is_none()
    {
        return Err(invalid("scope_id required for account/portfolio scope"));
    }
    if input.drift_band_bps < 0 || input.drift_band_bps > 10000 {
        return Err(invalid("drift_band_bps must be between 0 and 10000"));
    }
    Ok(())
}

pub fn validate_nodes_sum(nodes: &[NewTargetAllocationNode]) -> CoreResult<()> {
    let total: i32 = nodes.iter().map(|n| n.target_bps).sum();
    if total != 10000 {
        return Err(invalid(&format!(
            "Target allocations must sum to 10000 bps (100%), got {total}"
        )));
    }
    let mut seen = std::collections::HashSet::new();
    for node in nodes {
        if node.target_bps < 0 || node.target_bps > 10000 {
            return Err(invalid(&format!(
                "target_bps for category {} must be between 0 and 10000",
                node.category_id
            )));
        }
        if !seen.insert(&node.category_id) {
            return Err(invalid(&format!(
                "Duplicate category_id: {}",
                node.category_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portfolio::allocation_targets::model::TriggerType;

    fn base_profile(name: &str) -> NewTargetProfile {
        NewTargetProfile {
            name: name.to_string(),
            scope_type: ScopeType::All,
            scope_id: None,
            taxonomy_id: "asset_classes".to_string(),
            trigger_type: TriggerType::Threshold,
            drift_band_bps: 500,
        }
    }

    fn node(category_id: &str, bps: i32) -> NewTargetAllocationNode {
        NewTargetAllocationNode {
            profile_id: "p1".to_string(),
            category_id: category_id.to_string(),
            target_bps: bps,
            is_locked: false,
            is_required: true,
        }
    }

    // ── validate_new_profile ─────────────────────────────────────────────────

    #[test]
    fn profile_empty_name_rejected() {
        let p = base_profile("  ");
        assert!(validate_new_profile(&p).is_err());
    }

    #[test]
    fn profile_valid_passes() {
        assert!(validate_new_profile(&base_profile("My profile")).is_ok());
    }

    #[test]
    fn profile_account_scope_requires_scope_id() {
        let p = NewTargetProfile {
            scope_type: ScopeType::Account,
            scope_id: None,
            ..base_profile("p")
        };
        assert!(validate_new_profile(&p).is_err());
    }

    #[test]
    fn profile_account_scope_with_scope_id_passes() {
        let p = NewTargetProfile {
            scope_type: ScopeType::Account,
            scope_id: Some("acc-1".to_string()),
            ..base_profile("p")
        };
        assert!(validate_new_profile(&p).is_ok());
    }

    #[test]
    fn profile_drift_band_out_of_range_rejected() {
        let p = NewTargetProfile {
            drift_band_bps: 10001,
            ..base_profile("p")
        };
        assert!(validate_new_profile(&p).is_err());
    }

    #[test]
    fn profile_drift_band_zero_allowed() {
        let p = NewTargetProfile {
            drift_band_bps: 0,
            ..base_profile("p")
        };
        assert!(validate_new_profile(&p).is_ok());
    }

    // ── validate_nodes_sum ───────────────────────────────────────────────────

    #[test]
    fn nodes_sum_to_10000_passes() {
        let nodes = vec![node("EQUITY", 6000), node("FIXED_INCOME", 4000)];
        assert!(validate_nodes_sum(&nodes).is_ok());
    }

    #[test]
    fn nodes_not_summing_to_10000_rejected() {
        let nodes = vec![node("EQUITY", 6000), node("FIXED_INCOME", 3000)];
        assert!(validate_nodes_sum(&nodes).is_err());
    }

    #[test]
    fn nodes_duplicate_category_rejected() {
        let nodes = vec![node("EQUITY", 5000), node("EQUITY", 5000)];
        assert!(validate_nodes_sum(&nodes).is_err());
    }

    #[test]
    fn nodes_negative_bps_rejected() {
        let nodes = vec![node("EQUITY", -100), node("FIXED_INCOME", 10100)];
        assert!(validate_nodes_sum(&nodes).is_err());
    }

    #[test]
    fn nodes_zero_target_allowed_when_sum_correct() {
        // Zero-current category can have a target — valid if sum == 10000
        let nodes = vec![
            node("EQUITY", 6000),
            node("FIXED_INCOME", 4000),
            node("BONDS", 0),
        ];
        // Sum is 10000 but BONDS has 0 — still valid per spec
        assert!(validate_nodes_sum(&nodes).is_ok());
    }
}
