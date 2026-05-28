//! Centralized broker sync orchestrator.
//!
//! This module provides a unified sync implementation that can be used
//! by both Tauri (desktop) and Axum (web) platforms.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};
use log::{debug, error, info, warn};

use super::models::{
    BrokerSyncStatusDetail, HoldingsDiff, NewAccountInfo, SyncActivitiesResponse,
    SyncHoldingsResponse, SyncResult,
};
use super::progress::{SyncProgressPayload, SyncProgressReporter, SyncStatus};
use super::traits::{BrokerApiClient, BrokerSyncServiceTrait};
use crate::broker_ingest::{ImportRunMode, ImportRunStatus, ImportRunSummary};
use wealthfolio_core::accounts::TrackingMode;

/// Configuration for sync operations.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Number of activities to fetch per page.
    pub page_limit: i64,
    /// Maximum number of pages to fetch per account (safety limit).
    pub max_pages: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            page_limit: 1000,
            max_pages: 10_000,
        }
    }
}

#[derive(Debug, Clone)]
struct ActivityQueryWindow {
    start_date: Option<String>,
    end_date: String,
    provider_waterline: String,
    has_local_cursor: bool,
}

#[derive(Debug, Clone, Default)]
struct ActivitySyncOutcome {
    fetched: u32,
    inserted: u32,
    assets_created: u32,
    needs_review: u32,
    new_asset_ids: Vec<String>,
    empty_first_page: bool,
    inconsistent_empty_page: bool,
}

enum ProviderReadiness {
    Ready(NaiveDate),
    NotReady(String),
}

fn parse_provider_sync_date(value: &str) -> Result<NaiveDate, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc).date_naive())
        .or_else(|_| NaiveDate::parse_from_str(value, "%Y-%m-%d"))
        .map_err(|_| format!("Invalid provider sync timestamp '{}'", value))
}

fn resolve_activity_readiness(
    status: Option<&BrokerSyncStatusDetail>,
) -> Result<ProviderReadiness, String> {
    let Some(status) = status else {
        return Ok(ProviderReadiness::NotReady(
            "Provider transaction sync status is unavailable".to_string(),
        ));
    };

    if status.initial_sync_completed == Some(false) {
        return Ok(ProviderReadiness::NotReady(
            "Provider transaction sync is still preparing".to_string(),
        ));
    }

    let Some(last_successful_sync) = status.last_successful_sync.as_deref() else {
        return Ok(ProviderReadiness::NotReady(
            "Provider transaction sync has no successful waterline".to_string(),
        ));
    };

    Ok(ProviderReadiness::Ready(parse_provider_sync_date(
        last_successful_sync,
    )?))
}

fn resolve_holdings_readiness(
    status: Option<&BrokerSyncStatusDetail>,
) -> Result<ProviderReadiness, String> {
    let Some(status) = status else {
        return Ok(ProviderReadiness::NotReady(
            "Provider holdings sync status is unavailable".to_string(),
        ));
    };

    if status.initial_sync_completed == Some(false) {
        return Ok(ProviderReadiness::NotReady(
            "Provider holdings sync is still preparing".to_string(),
        ));
    }

    let Some(last_successful_sync) = status.last_successful_sync.as_deref() else {
        return Ok(ProviderReadiness::NotReady(
            "Provider holdings sync has no successful waterline".to_string(),
        ));
    };

    Ok(ProviderReadiness::Ready(parse_provider_sync_date(
        last_successful_sync,
    )?))
}

fn provider_waterline_precedes_local_cursor(
    local_cursor: Option<&DateTime<Utc>>,
    provider_waterline: NaiveDate,
) -> bool {
    local_cursor
        .map(|cursor| provider_waterline < cursor.date_naive())
        .unwrap_or(false)
}

fn should_advance_activity_cursor(
    outcome: &ActivitySyncOutcome,
    window: &ActivityQueryWindow,
    provider_status: Option<&BrokerSyncStatusDetail>,
) -> bool {
    if outcome.inconsistent_empty_page {
        return false;
    }

    if outcome.fetched > 0 || window.has_local_cursor {
        return true;
    }

    matches!(
        provider_status,
        Some(BrokerSyncStatusDetail {
            initial_sync_completed: Some(true),
            first_transaction_date: None,
            ..
        })
    )
}

/// Orchestrates broker data synchronization.
///
/// This struct encapsulates the sync logic previously duplicated in
/// Tauri commands and Axum handlers. It handles:
/// - Connection syncing
/// - Account syncing (with sync_enabled filtering)
/// - Activity syncing with full pagination support
/// - Progress reporting via a pluggable reporter trait
///
/// # Example
///
/// ```ignore
/// let reporter = Arc::new(TauriProgressReporter::new(app_handle));
/// let orchestrator = SyncOrchestrator::new(sync_service, reporter, SyncConfig::default());
/// let result = orchestrator.sync_all(&api_client).await?;
/// ```
pub struct SyncOrchestrator<P: SyncProgressReporter> {
    sync_service: Arc<dyn BrokerSyncServiceTrait>,
    progress_reporter: Arc<P>,
    config: SyncConfig,
}

impl<P: SyncProgressReporter> SyncOrchestrator<P> {
    /// Create a new sync orchestrator.
    pub fn new(
        sync_service: Arc<dyn BrokerSyncServiceTrait>,
        progress_reporter: Arc<P>,
        config: SyncConfig,
    ) -> Self {
        Self {
            sync_service,
            progress_reporter,
            config,
        }
    }

    /// Perform a full sync: connections -> accounts -> activities.
    ///
    /// This is the main entry point for broker synchronization.
    /// Always emits sync-start and sync-complete/error events.
    pub async fn sync_all(&self, api_client: &dyn BrokerApiClient) -> Result<SyncResult, String> {
        info!("Starting broker data sync...");
        self.progress_reporter.report_sync_start();

        // Run the sync and ensure we always emit completion event
        let result = self.sync_all_internal(api_client).await;

        match &result {
            Ok(sync_result) => {
                self.progress_reporter.report_sync_complete(sync_result);
            }
            Err(err) => {
                // Create a failed result to emit the error event
                let failed_result = SyncResult {
                    success: false,
                    message: err.clone(),
                    connections_synced: None,
                    accounts_synced: None,
                    activities_synced: None,
                    holdings_synced: None,
                    new_accounts: None,
                };
                self.progress_reporter.report_sync_complete(&failed_result);
            }
        }

        result
    }

    /// Internal sync logic that may fail at any step.
    async fn sync_all_internal(
        &self,
        api_client: &dyn BrokerApiClient,
    ) -> Result<SyncResult, String> {
        // Step 1: Sync connections (platforms)
        let connections = api_client
            .list_connections()
            .await
            .map_err(|e| e.to_string())?;

        let connections_result = self
            .sync_service
            .sync_connections(connections.clone())
            .await
            .map_err(|e| format!("Failed to sync connections: {}", e))?;

        debug!(
            "Connections synced: {} created, {} updated",
            connections_result.platforms_created, connections_result.platforms_updated
        );

        // Step 2: Sync accounts (filter by sync_enabled)
        let authorization_ids: Vec<String> = connections.iter().map(|c| c.id.clone()).collect();
        let all_accounts = api_client
            .list_accounts(if authorization_ids.is_empty() {
                None
            } else {
                Some(authorization_ids)
            })
            .await
            .map_err(|e| e.to_string())?;

        let provider_transaction_statuses: HashMap<String, BrokerSyncStatusDetail> = all_accounts
            .iter()
            .filter_map(|account| {
                Some((
                    account.id.clone()?,
                    account.sync_status.as_ref()?.transactions.clone()?,
                ))
            })
            .collect();
        let provider_holdings_statuses: HashMap<String, BrokerSyncStatusDetail> = all_accounts
            .iter()
            .filter_map(|account| {
                Some((
                    account.id.clone()?,
                    account.sync_status.as_ref()?.holdings.clone()?,
                ))
            })
            .collect();

        // Track sync-enabled broker IDs for data sync
        let sync_enabled_broker_ids: HashSet<String> = all_accounts
            .iter()
            .filter(|a| a.sync_enabled)
            .filter_map(|a| a.id.clone())
            .collect();

        // Only create/update local accounts for sync-enabled broker accounts
        let accounts: Vec<_> = all_accounts
            .into_iter()
            .filter(|a| a.sync_enabled)
            .collect();

        let accounts_result = self
            .sync_service
            .sync_accounts(accounts)
            .await
            .map_err(|e| format!("Failed to sync accounts: {}", e))?;

        info!(
            "Accounts synced: {} created, {} updated, {} skipped",
            accounts_result.created, accounts_result.updated, accounts_result.skipped
        );

        // Step 3: Sync data for all synced accounts based on their tracking mode
        // - TRANSACTIONS mode: sync activities
        // - HOLDINGS mode: sync holdings (positions)
        // - NOT_SET mode: skip (needs user configuration first)
        let (activities_result, holdings_result) = self
            .sync_account_data(
                api_client,
                &sync_enabled_broker_ids,
                &provider_transaction_statuses,
                &provider_holdings_statuses,
            )
            .await?;

        // Build the accounts_needing_setup list - sync-enabled accounts with trackingMode=NOT_SET
        // This ensures the "review" toast appears on every sync until user configures all accounts
        let accounts_needing_setup: Vec<NewAccountInfo> = self
            .sync_service
            .get_synced_accounts()
            .map_err(|e| format!("Failed to get synced accounts: {}", e))?
            .into_iter()
            .filter(|acc| {
                acc.tracking_mode == TrackingMode::NotSet
                    && acc
                        .provider_account_id
                        .as_ref()
                        .is_some_and(|id| sync_enabled_broker_ids.contains(id))
            })
            .map(|acc| NewAccountInfo {
                local_account_id: acc.id.clone(),
                provider_account_id: acc.provider_account_id.unwrap_or_default(),
                default_name: acc.name.clone(),
                currency: acc.currency.clone(),
                institution_name: acc.platform_id.clone(),
            })
            .collect();

        let new_accounts: Option<Vec<NewAccountInfo>> = if accounts_needing_setup.is_empty() {
            None
        } else {
            Some(accounts_needing_setup)
        };

        let total_failed = activities_result.accounts_failed + holdings_result.accounts_failed;
        let total_warnings = activities_result.accounts_warned + holdings_result.accounts_warned;
        let result = SyncResult {
            success: total_failed == 0,
            message: format!(
                "Sync completed. {} accounts created, {} activities synced, {} holdings synced{}{}",
                accounts_result.created,
                activities_result.activities_upserted,
                holdings_result.positions_upserted,
                if total_failed == 0 {
                    ".".to_string()
                } else {
                    format!(" ({} failed).", total_failed)
                },
                if total_warnings == 0 {
                    "".to_string()
                } else {
                    format!(
                        " ({} warning{}).",
                        total_warnings,
                        if total_warnings == 1 { "" } else { "s" }
                    )
                }
            ),
            connections_synced: Some(connections_result),
            accounts_synced: Some(accounts_result),
            activities_synced: Some(activities_result),
            holdings_synced: Some(holdings_result),
            new_accounts,
        };

        Ok(result)
    }

    /// Sync account data for all synced accounts based on their tracking mode.
    /// - TRANSACTIONS mode: sync activities
    /// - HOLDINGS mode: sync holdings (positions)
    /// - NOT_SET mode: skip (needs user configuration first)
    async fn sync_account_data(
        &self,
        api_client: &dyn BrokerApiClient,
        sync_enabled_broker_ids: &HashSet<String>,
        provider_transaction_statuses: &HashMap<String, BrokerSyncStatusDetail>,
        provider_holdings_statuses: &HashMap<String, BrokerSyncStatusDetail>,
    ) -> Result<(SyncActivitiesResponse, SyncHoldingsResponse), String> {
        let synced_accounts = self
            .sync_service
            .get_synced_accounts()
            .map_err(|e| format!("Failed to get synced accounts: {}", e))?;

        let mut activities_summary = SyncActivitiesResponse::default();
        let mut holdings_summary = SyncHoldingsResponse::default();

        for account in synced_accounts {
            let Some(broker_account_id) = account.provider_account_id.clone() else {
                continue;
            };

            // Skip accounts that are not sync-enabled
            if !sync_enabled_broker_ids.contains(&broker_account_id) {
                info!(
                    "Skipping sync for account '{}' (sync disabled)",
                    account.name
                );
                continue;
            }

            let account_id = account.id.clone();
            let account_name = account.name.clone();
            let is_holdings_mode = account.tracking_mode == TrackingMode::Holdings;

            if account.tracking_mode == TrackingMode::NotSet {
                info!(
                    "Skipping sync for account '{}' (trackingMode=NOT_SET)",
                    account_name
                );
                continue;
            }

            // Track reference-only activity issues for HOLDINGS accounts.
            let mut activity_warning: Option<String> = None;

            // Mark sync attempt
            if let Err(err) = self
                .sync_service
                .mark_activity_sync_attempt(account_id.clone())
                .await
                .map_err(|e| format!("Failed to mark activity sync attempt: {}", e))
            {
                error!(
                    "Failed to mark activity sync attempt for '{}': {}",
                    account_name, err
                );
                if is_holdings_mode {
                    let warning = err.to_string();
                    self.progress_reporter.report_progress(
                        SyncProgressPayload::new(
                            &account_id,
                            &account_name,
                            SyncStatus::NeedsReview,
                        )
                        .with_message(format!(
                            "Activity reference sync setup failed; continuing holdings sync: {}",
                            warning
                        )),
                    );
                    activities_summary.accounts_warned += 1;
                    activity_warning = Some(warning);
                } else {
                    activities_summary.accounts_failed += 1;
                    continue;
                }
            }

            let mut activity_import_run_id: Option<String> = None;

            if activity_warning.is_none() {
                let local_activity_state = match self
                    .sync_service
                    .get_activity_sync_state(&account_id)
                {
                    Ok(state) => Some(state),
                    Err(err) => {
                        error!(
                            "Failed to read activity sync state for '{}': {}",
                            account_name, err
                        );
                        if is_holdings_mode {
                            self.progress_reporter.report_progress(
                                SyncProgressPayload::new(
                                    &account_id,
                                    &account_name,
                                    SyncStatus::NeedsReview,
                                )
                                .with_message(format!(
                                    "Activity reference sync state failed; continuing holdings sync: {}",
                                    err
                                )),
                            );
                            activities_summary.accounts_warned += 1;
                            activity_warning = Some(err.to_string());
                            None
                        } else {
                            activities_summary.accounts_failed += 1;
                            continue;
                        }
                    }
                };

                if let Some(local_activity_state) = local_activity_state {
                    let local_cursor = local_activity_state
                        .as_ref()
                        .and_then(|state| state.last_successful_at.as_ref());
                    let provider_activity_status =
                        provider_transaction_statuses.get(&broker_account_id);

                    let activity_waterline = match resolve_activity_readiness(
                        provider_activity_status,
                    ) {
                        Ok(ProviderReadiness::Ready(date))
                            if provider_waterline_precedes_local_cursor(local_cursor, date) =>
                        {
                            let Some(cursor) = local_cursor else {
                                unreachable!("stale provider waterline requires local cursor");
                            };
                            let message = format!(
                                "Activity sync skipped: provider transaction waterline {} is older than local cursor {}",
                                date,
                                cursor.date_naive()
                            );
                            if let Err(e) = self
                                .sync_service
                                .finalize_activity_sync_success(
                                    account_id.clone(),
                                    cursor.to_rfc3339(),
                                    None,
                                )
                                .await
                            {
                                error!(
                                    "Failed to restore activity sync state for '{}': {}",
                                    account_name, e
                                );
                                activities_summary.accounts_failed += 1;
                                if !is_holdings_mode {
                                    continue;
                                }
                                activity_warning = Some(format!(
                                    "Activity sync skipped, but sync state cleanup failed: {}",
                                    e
                                ));
                            } else {
                                self.progress_reporter.report_progress(
                                    SyncProgressPayload::new(
                                        &account_id,
                                        &account_name,
                                        SyncStatus::Complete,
                                    )
                                    .with_message(message),
                                );
                                activities_summary.accounts_synced += 1;
                            }
                            None
                        }
                        Ok(ProviderReadiness::Ready(date)) => Some(date),
                        Ok(ProviderReadiness::NotReady(reason)) => {
                            let warning = format!("Activity sync deferred: {}", reason);
                            if let Err(e) = self
                                .sync_service
                                .finalize_activity_sync_needs_review(
                                    account_id.clone(),
                                    warning.clone(),
                                    None,
                                )
                                .await
                            {
                                error!(
                                    "Failed to mark deferred activity sync for '{}': {}",
                                    account_name, e
                                );
                            }
                            self.progress_reporter.report_progress(
                                SyncProgressPayload::new(
                                    &account_id,
                                    &account_name,
                                    SyncStatus::NeedsReview,
                                )
                                .with_message(warning.clone()),
                            );
                            activities_summary.accounts_warned += 1;
                            activity_warning = Some(warning);
                            None
                        }
                        Err(err) => {
                            error!(
                                "Failed to resolve provider activity sync status for '{}': {}",
                                account_name, err
                            );
                            if is_holdings_mode {
                                let message = format!(
                                    "Activity reference sync status failed; continuing holdings sync: {}",
                                    err
                                );
                                self.progress_reporter.report_progress(
                                    SyncProgressPayload::new(
                                        &account_id,
                                        &account_name,
                                        SyncStatus::NeedsReview,
                                    )
                                    .with_message(message),
                                );
                                activities_summary.accounts_warned += 1;
                                activity_warning = Some(err);
                                None
                            } else {
                                activities_summary.accounts_failed += 1;
                                continue;
                            }
                        }
                    };

                    let query_window = match activity_waterline {
                        Some(waterline) => {
                            match self.compute_activity_query_window(&account_id, waterline) {
                                Ok(window) => Some(window),
                                Err(err) => {
                                    error!(
                                        "Failed to compute activity query window for '{}': {}",
                                        account_name, err
                                    );
                                    if is_holdings_mode {
                                        let message = format!(
                                            "Activity reference query window failed; continuing holdings sync: {}",
                                            err
                                        );
                                        self.progress_reporter.report_progress(
                                            SyncProgressPayload::new(
                                                &account_id,
                                                &account_name,
                                                SyncStatus::NeedsReview,
                                            )
                                            .with_message(message),
                                        );
                                        activities_summary.accounts_warned += 1;
                                        activity_warning = Some(err);
                                        None
                                    } else {
                                        activities_summary.accounts_failed += 1;
                                        continue;
                                    }
                                }
                            }
                        }
                        None => None,
                    };

                    if let Some(query_window) = query_window {
                        let import_mode = if query_window.start_date.is_none() {
                            ImportRunMode::Initial
                        } else {
                            ImportRunMode::Incremental
                        };

                        let import_run = match self
                            .sync_service
                            .create_import_run(&account_id, import_mode)
                            .await
                        {
                            Ok(run) => {
                                debug!(
                                    "Created import run {} for account '{}'",
                                    run.id, account_name
                                );
                                Some(run)
                            }
                            Err(e) => {
                                error!("Failed to create import run for '{}': {}", account_name, e);
                                None
                            }
                        };
                        activity_import_run_id = import_run.as_ref().map(|r| r.id.clone());

                        let window_label = match &query_window.start_date {
                            Some(s) => format!("{} -> {}", s, query_window.end_date),
                            None => format!("ALL -> {}", query_window.end_date),
                        };
                        info!(
                            "Syncing activities for account '{}' ({}): {}",
                            account_name, broker_account_id, window_label
                        );

                        self.progress_reporter.report_progress(
                            SyncProgressPayload::new(
                                &account_id,
                                &account_name,
                                SyncStatus::Syncing,
                            )
                            .with_message(format!("Starting sync: {}", window_label)),
                        );

                        match self
                            .sync_account_activities(
                                api_client,
                                &account_id,
                                &account_name,
                                &broker_account_id,
                                query_window.start_date.as_deref(),
                                Some(query_window.end_date.as_str()),
                                activity_import_run_id.clone(),
                            )
                            .await
                        {
                            Ok(outcome) => {
                                let mut import_status = if outcome.needs_review > 0 {
                                    ImportRunStatus::NeedsReview
                                } else {
                                    ImportRunStatus::Applied
                                };
                                let summary = ImportRunSummary {
                                    fetched: outcome.fetched,
                                    inserted: outcome.inserted,
                                    updated: 0,
                                    skipped: 0,
                                    warnings: outcome.needs_review,
                                    errors: 0,
                                    removed: 0,
                                    assets_created: outcome.assets_created,
                                };

                                let should_advance_cursor = should_advance_activity_cursor(
                                    &outcome,
                                    &query_window,
                                    provider_activity_status,
                                );

                                if should_advance_cursor {
                                    let sync_state_failed = self
                                        .sync_service
                                        .finalize_activity_sync_success(
                                            account_id.clone(),
                                            query_window.provider_waterline.clone(),
                                            activity_import_run_id.clone(),
                                        )
                                        .await
                                        .is_err();

                                    if sync_state_failed {
                                        error!(
                                        "Failed to update activity sync state for '{}', but activities were synced",
                                        account_name
                                    );
                                    }
                                } else {
                                    let warning = if outcome.inconsistent_empty_page {
                                        "Activity sync returned an empty page while provider pagination reported more data"
                                    } else if outcome.empty_first_page {
                                        "Initial activity sync returned no rows even though provider reports transactions may exist"
                                    } else {
                                        "Initial activity sync did not confirm a complete empty history"
                                }
                                .to_string();
                                    if let Err(e) = self
                                        .sync_service
                                        .finalize_activity_sync_needs_review(
                                            account_id.clone(),
                                            warning.clone(),
                                            activity_import_run_id.clone(),
                                        )
                                        .await
                                    {
                                        error!(
                                        "Failed to mark activity sync as needs review for '{}': {}",
                                        account_name, e
                                    );
                                    }
                                    import_status = ImportRunStatus::NeedsReview;
                                    activities_summary.accounts_warned += 1;
                                    activity_warning = Some(warning.clone());
                                    self.progress_reporter.report_progress(
                                        SyncProgressPayload::new(
                                            &account_id,
                                            &account_name,
                                            SyncStatus::NeedsReview,
                                        )
                                        .with_activities_fetched(outcome.fetched as usize)
                                        .with_message(warning),
                                    );
                                }

                                if let Some(ref run_id) = activity_import_run_id {
                                    if outcome.needs_review > 0 {
                                        info!(
                                            "Import run {} has {} activities needing review",
                                            run_id, outcome.needs_review
                                        );
                                    }

                                    let _ = self
                                        .sync_service
                                        .finalize_import_run(run_id, summary, import_status, None)
                                        .await;
                                }

                                if !should_advance_cursor {
                                    if !is_holdings_mode {
                                        continue;
                                    }
                                } else {
                                    let status = if outcome.needs_review > 0 {
                                        SyncStatus::NeedsReview
                                    } else {
                                        SyncStatus::Complete
                                    };
                                    self.progress_reporter.report_progress(
                                        SyncProgressPayload::new(
                                            &account_id,
                                            &account_name,
                                            status,
                                        )
                                        .with_activities_fetched(outcome.fetched as usize)
                                        .with_message(
                                            format!(
                                                "Synced {} activities ({} need review)",
                                                outcome.inserted, outcome.needs_review
                                            ),
                                        ),
                                    );

                                    activities_summary.accounts_synced += 1;
                                }

                                activities_summary.activities_upserted += outcome.inserted as usize;
                                activities_summary.assets_inserted +=
                                    outcome.assets_created as usize;
                                activities_summary
                                    .new_asset_ids
                                    .extend(outcome.new_asset_ids);
                            }
                            Err(err) => {
                                error!("Failed to sync activities for '{}': {}", account_name, err);

                                let _ = self
                                    .sync_service
                                    .finalize_activity_sync_failure(
                                        account_id.clone(),
                                        err.clone(),
                                        activity_import_run_id.clone(),
                                    )
                                    .await;

                                if let Some(ref run_id) = activity_import_run_id {
                                    let summary = ImportRunSummary::default();
                                    let _ = self
                                        .sync_service
                                        .finalize_import_run(
                                            run_id,
                                            summary,
                                            ImportRunStatus::Failed,
                                            Some(err.clone()),
                                        )
                                        .await;
                                }

                                if is_holdings_mode {
                                    self.progress_reporter.report_progress(
                                    SyncProgressPayload::new(
                                        &account_id,
                                        &account_name,
                                        SyncStatus::NeedsReview,
                                    )
                                    .with_message(format!(
                                        "Activity reference sync failed; continuing holdings sync: {}",
                                        err
                                    )),
                                );
                                    activities_summary.accounts_warned += 1;
                                    activity_warning = Some(err);
                                } else {
                                    self.progress_reporter.report_progress(
                                        SyncProgressPayload::new(
                                            &account_id,
                                            &account_name,
                                            SyncStatus::Failed,
                                        )
                                        .with_message(err.clone()),
                                    );
                                    activities_summary.accounts_failed += 1;
                                    continue;
                                }
                            }
                        }
                    }
                }
            }

            if !is_holdings_mode {
                continue;
            }

            match resolve_holdings_readiness(provider_holdings_statuses.get(&broker_account_id)) {
                Ok(ProviderReadiness::Ready(_)) => {}
                Ok(ProviderReadiness::NotReady(reason)) => {
                    let warning = format!("Holdings sync deferred: {}", reason);
                    self.progress_reporter.report_progress(
                        SyncProgressPayload::new(
                            &account_id,
                            &account_name,
                            SyncStatus::NeedsReview,
                        )
                        .with_message(warning),
                    );
                    holdings_summary.accounts_warned += 1;
                    continue;
                }
                Err(err) => {
                    error!(
                        "Failed to resolve provider holdings sync status for '{}': {}",
                        account_name, err
                    );
                    holdings_summary.accounts_failed += 1;
                    continue;
                }
            }

            let holdings_import_mode = match self
                .sync_service
                .has_broker_imported_holdings_snapshot(&account_id)
            {
                Ok(true) => ImportRunMode::Incremental,
                Ok(false) => ImportRunMode::Initial,
                Err(e) => {
                    warn!(
                        "Failed to read holdings snapshot state for '{}' before holdings sync: {}",
                        account_name, e
                    );
                    ImportRunMode::Initial
                }
            };

            let holdings_import_run = match self
                .sync_service
                .create_import_run(&account_id, holdings_import_mode)
                .await
            {
                Ok(run) => {
                    debug!(
                        "Created holdings import run {} for account '{}'",
                        run.id, account_name
                    );
                    Some(run)
                }
                Err(e) => {
                    error!(
                        "Failed to create holdings import run for '{}': {}",
                        account_name, e
                    );
                    None
                }
            };
            let holdings_import_run_id = holdings_import_run.as_ref().map(|r| r.id.clone());

            match self
                .sync_account_holdings(api_client, &account_id, &account_name, &broker_account_id)
                .await
            {
                Ok((diff, assets_created, new_asset_ids)) => {
                    let summary = ImportRunSummary {
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
                            .finalize_import_run(run_id, summary, ImportRunStatus::Applied, None)
                            .await;
                    }

                    holdings_summary.accounts_synced += 1;
                    holdings_summary.positions_upserted +=
                        diff.added_positions + diff.updated_positions;
                    holdings_summary.snapshots_upserted += if diff.snapshot_saved { 1 } else { 0 };
                    holdings_summary.assets_inserted += assets_created;
                    holdings_summary.new_asset_ids.extend(new_asset_ids);

                    if let Some(warning) = activity_warning {
                        let warning_message = format!(
                            "Holdings synced, but activity reference sync needs review: {}",
                            warning
                        );
                        if let Err(e) = self
                            .sync_service
                            .finalize_activity_sync_needs_review(
                                account_id.clone(),
                                warning_message.clone(),
                                activity_import_run_id.clone(),
                            )
                            .await
                        {
                            error!(
                                "Failed to mark activity sync state as needs review for '{}': {}",
                                account_name, e
                            );
                        }

                        self.progress_reporter.report_progress(
                            SyncProgressPayload::new(
                                &account_id,
                                &account_name,
                                SyncStatus::NeedsReview,
                            )
                            .with_message(warning_message),
                        );
                    }
                }
                Err(err) => {
                    error!("Failed to sync holdings for '{}': {}", account_name, err);

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

                    // Holdings is the valuation source for HOLDINGS mode, so this is a hard failure.
                    let _ = self
                        .sync_service
                        .finalize_activity_sync_failure(
                            account_id.clone(),
                            format!("Holdings sync failed: {}", err),
                            holdings_import_run_id.clone(),
                        )
                        .await;

                    self.progress_reporter.report_progress(
                        SyncProgressPayload::new(&account_id, &account_name, SyncStatus::Failed)
                            .with_message(err),
                    );

                    holdings_summary.accounts_failed += 1;
                }
            }
        }

        Ok((activities_summary, holdings_summary))
    }

    /// Sync holdings for a single account (HOLDINGS tracking mode).
    ///
    /// Fetches current holdings from the broker API and saves as a snapshot.
    /// Returns (position_diff, assets_created, new_asset_ids).
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

        // Emit progress event
        self.progress_reporter.report_progress(
            SyncProgressPayload::new(account_id, account_name, SyncStatus::Syncing)
                .with_message("Fetching holdings from broker...".to_string()),
        );

        // Fetch holdings from broker API
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

        // Save holdings as a snapshot
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

        // Emit completion event
        self.progress_reporter.report_progress(
            SyncProgressPayload::new(account_id, account_name, SyncStatus::Complete).with_message(
                format!("{} ({} assets created)", summary_message, assets_created),
            ),
        );

        Ok((diff, assets_created, new_asset_ids))
    }

    /// Sync activities for a single account with full pagination.
    ///
    /// Returns (fetched, inserted, assets_created, needs_review, new_asset_ids).
    #[allow(clippy::too_many_arguments)]
    async fn sync_account_activities(
        &self,
        api_client: &dyn BrokerApiClient,
        account_id: &str,
        account_name: &str,
        broker_account_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
        import_run_id: Option<String>,
    ) -> Result<ActivitySyncOutcome, String> {
        let mut offset: i64 = 0;
        let limit = self.config.page_limit;
        let mut pages_fetched: usize = 0;
        let mut last_page_first_id: Option<String> = None;
        let mut empty_first_page = false;
        let mut inconsistent_empty_page = false;

        let mut total_fetched: u32 = 0;
        let mut total_inserted: u32 = 0;
        let mut total_assets_created: u32 = 0;
        let mut total_needs_review: u32 = 0;
        let mut all_new_asset_ids: Vec<String> = Vec::new();

        loop {
            // Check max pages limit
            if pages_fetched >= self.config.max_pages {
                return Err(format!(
                    "Pagination exceeded max pages ({}). Aborting.",
                    self.config.max_pages
                ));
            }

            // Fetch page
            let page = api_client
                .get_account_activities(
                    broker_account_id,
                    start_date,
                    end_date,
                    Some(offset),
                    Some(limit),
                )
                .await
                .map_err(|e| e.to_string())?;

            let data = page.data;
            pages_fetched += 1;
            total_fetched += data.len() as u32;

            let page_total = page.pagination.as_ref().and_then(|p| p.total);

            // Emit progress event
            self.progress_reporter.report_progress(
                SyncProgressPayload::new(account_id, account_name, SyncStatus::Syncing)
                    .with_page(pages_fetched)
                    .with_activities_fetched(total_fetched as usize)
                    .with_message(format!(
                        "Fetched {} activities (total: {:?})",
                        total_fetched, page_total
                    )),
            );

            info!(
                "Fetched {} activities for '{}' (offset {}, total {:?})",
                data.len(),
                account_name,
                offset,
                page_total
            );

            if !data.is_empty() {
                // Check for stuck pagination
                if let Some(first_id) = data.first().and_then(|a| a.id.clone()) {
                    if offset > 0 {
                        if let Some(prev) = &last_page_first_id {
                            if prev == &first_id {
                                return Err(
                                    "Pagination appears stuck (same first activity id returned for multiple pages)."
                                        .to_string(),
                                );
                            }
                        }
                    }
                    last_page_first_id = Some(first_id);
                }

                // Upsert activities
                debug!(
                    "Upserting {} activities for account '{}'...",
                    data.len(),
                    account_name
                );

                let (upserted, assets, new_asset_ids, needs_review) = self
                    .sync_service
                    .upsert_account_activities(
                        account_id.to_string(),
                        import_run_id.clone(),
                        data.clone(),
                    )
                    .await
                    .map_err(|e| format!("Failed to upsert activities: {}", e))?;

                info!(
                    "Upserted {} activities, {} assets for '{}' ({} need review)",
                    upserted, assets, account_name, needs_review
                );

                total_inserted += upserted as u32;
                total_assets_created += assets as u32;
                total_needs_review += needs_review as u32;
                all_new_asset_ids.extend(new_asset_ids);
            }

            let received = data.len() as i64;
            let next_offset = offset + received;

            // Avoid spinning forever if API reports more pages but returns no data.
            if received == 0 {
                empty_first_page = pages_fetched == 1;
                inconsistent_empty_page = page.pagination.as_ref().is_some_and(|p| {
                    p.has_more == Some(true) || p.total.is_some_and(|total| offset < total)
                });
                break;
            }

            // Check if there are more pages.
            // Prefer explicit has_more when provided, then fall back to total/limit inference.
            let has_more = match page.pagination.as_ref() {
                Some(p) => match p.has_more {
                    Some(true) => true,
                    Some(false) => false,
                    None => {
                        if let Some(total) = p.total {
                            next_offset < total
                        } else if let Some(page_limit) = p.limit {
                            received >= page_limit
                        } else {
                            received >= limit
                        }
                    }
                },
                None => received >= limit,
            };

            // Advance offset by number of items received
            offset = next_offset;

            if !has_more {
                break;
            }
        }

        Ok(ActivitySyncOutcome {
            fetched: total_fetched,
            inserted: total_inserted,
            assets_created: total_assets_created,
            needs_review: total_needs_review,
            new_asset_ids: all_new_asset_ids,
            empty_first_page,
            inconsistent_empty_page,
        })
    }

    /// Compute the activity query window for incremental sync.
    fn compute_activity_query_window(
        &self,
        account_id: &str,
        provider_waterline: NaiveDate,
    ) -> Result<ActivityQueryWindow, String> {
        let today = Utc::now().date_naive();
        let end_date = provider_waterline.min(today);
        let sync_state = self
            .sync_service
            .get_activity_sync_state(account_id)
            .map_err(|e| format!("Failed to read activity sync state: {}", e))?;

        let local_cursor = sync_state.and_then(|s| s.last_successful_at);
        let has_local_cursor = local_cursor.is_some();
        let start_date = local_cursor
            .map(|dt| dt.date_naive())
            .map(|d| (d - chrono::Days::new(1)).min(end_date))
            .map(|d| d.format("%Y-%m-%d").to_string());

        Ok(ActivityQueryWindow {
            start_date,
            end_date: end_date.format("%Y-%m-%d").to_string(),
            provider_waterline: end_date.format("%Y-%m-%d").to_string(),
            has_local_cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_config_default() {
        let config = SyncConfig::default();
        assert_eq!(config.page_limit, 1000);
        assert_eq!(config.max_pages, 10_000);
    }

    fn transactions_status(
        initial_sync_completed: Option<bool>,
        last_successful_sync: Option<&str>,
        first_transaction_date: Option<&str>,
    ) -> BrokerSyncStatusDetail {
        BrokerSyncStatusDetail {
            initial_sync_completed,
            last_successful_sync: last_successful_sync.map(str::to_string),
            first_transaction_date: first_transaction_date.map(str::to_string),
        }
    }

    fn window(has_local_cursor: bool) -> ActivityQueryWindow {
        ActivityQueryWindow {
            start_date: has_local_cursor.then(|| "2026-05-21".to_string()),
            end_date: "2026-05-22".to_string(),
            provider_waterline: "2026-05-22".to_string(),
            has_local_cursor,
        }
    }

    #[test]
    fn provider_not_ready_defers_first_activity_sync() {
        let status = transactions_status(Some(false), None, None);
        let readiness = resolve_activity_readiness(Some(&status)).unwrap();

        assert!(matches!(readiness, ProviderReadiness::NotReady(_)));
    }

    #[test]
    fn provider_waterline_without_initial_flag_allows_activity_fetch() {
        let status = transactions_status(None, Some("2026-05-22"), None);
        let readiness = resolve_activity_readiness(Some(&status)).unwrap();

        assert!(matches!(
            readiness,
            ProviderReadiness::Ready(date)
                if date == NaiveDate::from_ymd_opt(2026, 5, 22).unwrap()
        ));
    }

    #[test]
    fn provider_waterline_before_local_cursor_is_stale() {
        let local_cursor = DateTime::parse_from_rfc3339("2026-05-22T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let stale_waterline = NaiveDate::from_ymd_opt(2026, 5, 21).unwrap();
        let current_waterline = NaiveDate::from_ymd_opt(2026, 5, 22).unwrap();

        assert!(provider_waterline_precedes_local_cursor(
            Some(&local_cursor),
            stale_waterline
        ));
        assert!(!provider_waterline_precedes_local_cursor(
            Some(&local_cursor),
            current_waterline
        ));
    }

    #[test]
    fn initial_empty_activity_sync_with_first_transaction_does_not_advance_cursor() {
        let status = transactions_status(Some(true), Some("2026-05-22"), Some("2020-01-01"));
        let outcome = ActivitySyncOutcome {
            empty_first_page: true,
            ..ActivitySyncOutcome::default()
        };

        assert!(!should_advance_activity_cursor(
            &outcome,
            &window(false),
            Some(&status)
        ));
    }

    #[test]
    fn initial_empty_activity_sync_with_confirmed_empty_history_advances_cursor() {
        let status = transactions_status(Some(true), Some("2026-05-22"), None);
        let outcome = ActivitySyncOutcome {
            empty_first_page: true,
            ..ActivitySyncOutcome::default()
        };

        assert!(should_advance_activity_cursor(
            &outcome,
            &window(false),
            Some(&status)
        ));
    }

    #[test]
    fn incremental_empty_activity_sync_advances_cursor_to_provider_waterline() {
        let status = transactions_status(Some(true), Some("2026-05-22"), Some("2020-01-01"));
        let outcome = ActivitySyncOutcome {
            empty_first_page: true,
            ..ActivitySyncOutcome::default()
        };

        assert!(should_advance_activity_cursor(
            &outcome,
            &window(true),
            Some(&status)
        ));
    }

    #[test]
    fn inconsistent_empty_activity_page_does_not_advance_cursor() {
        let status = transactions_status(Some(true), Some("2026-05-22"), None);
        let outcome = ActivitySyncOutcome {
            empty_first_page: true,
            inconsistent_empty_page: true,
            ..ActivitySyncOutcome::default()
        };

        assert!(!should_advance_activity_cursor(
            &outcome,
            &window(true),
            Some(&status)
        ));
    }

    #[test]
    fn holdings_not_ready_defers_snapshot_sync() {
        let status = transactions_status(Some(false), None, None);
        let readiness = resolve_holdings_readiness(Some(&status)).unwrap();

        assert!(matches!(readiness, ProviderReadiness::NotReady(_)));
    }
}
