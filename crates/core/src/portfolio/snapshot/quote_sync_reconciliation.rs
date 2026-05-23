use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::errors::Result;
use crate::quotes::QuoteServiceTrait;

use super::{AccountStateSnapshot, SnapshotServiceTrait};

pub fn holdings_quantities(snapshot: &AccountStateSnapshot) -> HashMap<String, Decimal> {
    snapshot
        .positions
        .iter()
        .map(|(asset_id, position)| (asset_id.clone(), position.quantity))
        .collect()
}

pub async fn reconcile_quote_sync_from_snapshot(
    quote_service: &dyn QuoteServiceTrait,
    snapshot: &AccountStateSnapshot,
) -> Result<()> {
    let current_holdings = holdings_quantities(snapshot);
    quote_service
        .update_position_status_from_holdings(&current_holdings)
        .await
}

pub async fn reconcile_quote_sync_from_latest_account_snapshots(
    snapshot_service: &dyn SnapshotServiceTrait,
    quote_service: &dyn QuoteServiceTrait,
    account_ids: &[String],
) -> Result<bool> {
    if account_ids.is_empty() {
        return Ok(false);
    }

    let mut current_holdings: HashMap<String, Decimal> = HashMap::new();
    for account_id in account_ids {
        let Some(snapshot) = snapshot_service.get_latest_holdings_snapshot(account_id)? else {
            continue;
        };
        for (asset_id, quantity) in holdings_quantities(&snapshot) {
            *current_holdings.entry(asset_id).or_insert(Decimal::ZERO) += quantity;
        }
    }

    if current_holdings.is_empty() {
        return Ok(false);
    }

    quote_service
        .update_position_status_from_holdings(&current_holdings)
        .await?;
    Ok(true)
}
