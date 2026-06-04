use log::{debug, error, info, warn};

use super::super::models::{BrokerSyncStatusDetail, HoldingsDiff, SyncHoldingsResponse};
use super::super::progress::{SyncProgressPayload, SyncProgressReporter, SyncStatus};
use super::super::sync_readiness::{resolve_holdings_readiness, ProviderReadiness};
use super::super::traits::BrokerApiClient;
use super::{AccountSyncJob, HoldingsPhaseContext, SyncOrchestrator};
use crate::broker_ingest::{ImportRunMode, ImportRunStatus, ImportRunSummary};

impl<P: SyncProgressReporter> SyncOrchestrator<P> {
    pub(super) async fn sync_holdings_phase(
        &self,
        api_client: &dyn BrokerApiClient,
        job: &AccountSyncJob,
        provider_holdings_status: Option<&BrokerSyncStatusDetail>,
        context: HoldingsPhaseContext,
    ) -> SyncHoldingsResponse {
        let mut summary = SyncHoldingsResponse::default();

        match resolve_holdings_readiness(provider_holdings_status) {
            Ok(ProviderReadiness::Ready(_)) => {}
            Ok(ProviderReadiness::NotReady(reason)) => {
                let warning = if let Some(activity_warning) = context.activity_warning.as_ref() {
                    format!(
                        "Holdings sync deferred: {}; activity reference sync needs review: {}",
                        reason, activity_warning
                    )
                } else {
                    format!("Holdings sync deferred: {}", reason)
                };
                if let Err(e) = self
                    .sync_service
                    .finalize_activity_sync_needs_review(
                        job.account_id.clone(),
                        warning.clone(),
                        None,
                    )
                    .await
                {
                    error!(
                        "Failed to mark deferred holdings sync for '{}': {}",
                        job.account_name, e
                    );
                }
                self.progress_reporter.report_progress(
                    SyncProgressPayload::new(
                        &job.account_id,
                        &job.account_name,
                        SyncStatus::NeedsReview,
                    )
                    .with_message(warning),
                );
                summary.accounts_warned += 1;
                return summary;
            }
            Err(err) => {
                error!(
                    "Failed to resolve provider holdings sync status for '{}': {}",
                    job.account_name, err
                );
                let _ = self
                    .sync_service
                    .finalize_activity_sync_failure(
                        job.account_id.clone(),
                        format!("Holdings sync status failed: {}", err),
                        None,
                    )
                    .await;
                summary.accounts_failed += 1;
                return summary;
            }
        }

        let holdings_import_mode = match self
            .sync_service
            .has_broker_imported_holdings_snapshot(&job.account_id)
        {
            Ok(true) => ImportRunMode::Incremental,
            Ok(false) => ImportRunMode::Initial,
            Err(e) => {
                warn!(
                    "Failed to read holdings snapshot state for '{}' before holdings sync: {}",
                    job.account_name, e
                );
                ImportRunMode::Initial
            }
        };

        let holdings_import_run = match self
            .sync_service
            .create_import_run(&job.account_id, holdings_import_mode)
            .await
        {
            Ok(run) => {
                debug!(
                    "Created holdings import run {} for account '{}'",
                    run.id, job.account_name
                );
                Some(run)
            }
            Err(e) => {
                error!(
                    "Failed to create holdings import run for '{}': {}",
                    job.account_name, e
                );
                None
            }
        };
        let holdings_import_run_id = holdings_import_run.as_ref().map(|r| r.id.clone());

        match self
            .sync_account_holdings(
                api_client,
                &job.account_id,
                &job.account_name,
                &job.broker_account_id,
            )
            .await
        {
            Ok((diff, assets_created, new_asset_ids)) => {
                let import_summary = ImportRunSummary {
                    fetched: diff.total_positions as u32,
                    inserted: diff.added_positions as u32,
                    updated: diff.updated_positions as u32,
                    skipped: diff.unchanged_positions as u32,
                    warnings: 0,
                    errors: 0,
                    removed: diff.removed_positions as u32,
                    assets_created: assets_created as u32,
                };

                if let Some(ref run_id) = holdings_import_run_id {
                    let _ = self
                        .sync_service
                        .finalize_import_run(run_id, import_summary, ImportRunStatus::Applied, None)
                        .await;
                }

                summary.accounts_synced += 1;
                summary.positions_upserted += diff.added_positions + diff.updated_positions;
                summary.snapshots_upserted += if diff.snapshot_saved { 1 } else { 0 };
                summary.assets_inserted += assets_created;
                summary.new_asset_ids.extend(new_asset_ids);

                if let Some(warning) = context.activity_warning {
                    let warning_message = format!(
                        "Holdings synced, but activity reference sync needs review: {}",
                        warning
                    );
                    if let Err(e) = self
                        .sync_service
                        .finalize_activity_sync_needs_review(
                            job.account_id.clone(),
                            warning_message.clone(),
                            context.activity_import_run_id,
                        )
                        .await
                    {
                        error!(
                            "Failed to mark activity sync state as needs review for '{}': {}",
                            job.account_name, e
                        );
                    }

                    self.progress_reporter.report_progress(
                        SyncProgressPayload::new(
                            &job.account_id,
                            &job.account_name,
                            SyncStatus::NeedsReview,
                        )
                        .with_message(warning_message),
                    );
                }
            }
            Err(err) => {
                error!(
                    "Failed to sync holdings for '{}': {}",
                    job.account_name, err
                );

                if let Some(ref run_id) = holdings_import_run_id {
                    let _ = self
                        .sync_service
                        .finalize_import_run(
                            run_id,
                            ImportRunSummary::default(),
                            ImportRunStatus::Failed,
                            Some(err.clone()),
                        )
                        .await;
                }

                let _ = self
                    .sync_service
                    .finalize_activity_sync_failure(
                        job.account_id.clone(),
                        format!("Holdings sync failed: {}", err),
                        holdings_import_run_id,
                    )
                    .await;

                self.progress_reporter.report_progress(
                    SyncProgressPayload::new(
                        &job.account_id,
                        &job.account_name,
                        SyncStatus::Failed,
                    )
                    .with_message(err),
                );

                summary.accounts_failed += 1;
            }
        }

        summary
    }

    async fn sync_account_holdings(
        &self,
        api_client: &dyn BrokerApiClient,
        account_id: &str,
        account_name: &str,
        broker_account_id: &str,
    ) -> Result<(HoldingsDiff, usize, Vec<String>), String> {
        info!(
            "Syncing holdings for account '{}' ({})",
            account_name, broker_account_id
        );

        self.progress_reporter.report_progress(
            SyncProgressPayload::new(account_id, account_name, SyncStatus::Syncing)
                .with_message("Fetching holdings from broker...".to_string()),
        );

        let holdings = api_client
            .get_account_holdings(broker_account_id)
            .await
            .map_err(|e| e.to_string())?;

        let positions_count = holdings.positions.as_ref().map(|p| p.len()).unwrap_or(0);
        let option_positions_count = holdings
            .option_positions
            .as_ref()
            .map(|p| p.len())
            .unwrap_or(0);
        let balances_count = holdings.balances.as_ref().map(|b| b.len()).unwrap_or(0);

        info!(
            "Fetched {} positions, {} option positions, and {} balances for '{}'",
            positions_count, option_positions_count, balances_count, account_name
        );

        let (diff, assets_created, new_asset_ids) = self
            .sync_service
            .save_broker_holdings(
                account_id.to_string(),
                holdings.balances.unwrap_or_default(),
                holdings.positions.unwrap_or_default(),
                holdings.option_positions.unwrap_or_default(),
            )
            .await
            .map_err(|e| format!("Failed to save broker holdings: {}", e))?;

        let changed_positions =
            diff.added_positions + diff.updated_positions + diff.removed_positions;
        let summary_message = if changed_positions == 0 {
            format!(
                "No position changes detected ({} positions checked)",
                diff.total_positions
            )
        } else {
            format!(
                "Positions: +{}, {} updated, {} removed",
                diff.added_positions, diff.updated_positions, diff.removed_positions
            )
        };

        self.progress_reporter.report_progress(
            SyncProgressPayload::new(account_id, account_name, SyncStatus::Complete).with_message(
                format!("{} ({} assets created)", summary_message, assets_created),
            ),
        );

        Ok((diff, assets_created, new_asset_ids))
    }
}
