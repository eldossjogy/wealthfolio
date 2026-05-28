use crate::errors::{Error as CoreError, Result as CoreResult, ValidationError};
use rust_decimal::Decimal;
use std::str::FromStr;

use super::model::{NewTargetAllocationNode, NewTargetProfile, ScopeType, TriggerType};

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
    if matches!(
        input.trigger_type,
        TriggerType::Calendar | TriggerType::Combined
    ) && input.review_frequency.is_none()
    {
        return Err(invalid(
            "review_frequency is required for calendar and combined triggers",
        ));
    }
    let min_trade = Decimal::from_str(&input.min_trade_amount)
        .map_err(|_| invalid("min_trade_amount must be a valid decimal"))?;
    if min_trade < Decimal::ZERO {
        return Err(invalid("min_trade_amount must be >= 0"));
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
