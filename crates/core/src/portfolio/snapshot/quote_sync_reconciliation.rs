use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::constants::PORTFOLIO_TOTAL_ACCOUNT_ID;
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

pub async fn reconcile_quote_sync_from_latest_total_snapshot(
    snapshot_service: &dyn SnapshotServiceTrait,
    quote_service: &dyn QuoteServiceTrait,
) -> Result<bool> {
    let Some(total_snapshot) =
        snapshot_service.get_latest_holdings_snapshot(PORTFOLIO_TOTAL_ACCOUNT_ID)?
    else {
        return Ok(false);
    };

    reconcile_quote_sync_from_snapshot(quote_service, &total_snapshot).await?;
    Ok(true)
}
