use chrono::{NaiveDate, Utc};
use log::{debug, info};

use super::super::progress::{SyncProgressPayload, SyncProgressReporter, SyncStatus};
use super::super::traits::BrokerApiClient;
use super::{ActivityQueryWindow, ActivitySyncOutcome, SyncOrchestrator};

impl<P: SyncProgressReporter> SyncOrchestrator<P> {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn sync_account_activities(
        &self,
        api_client: &dyn BrokerApiClient,
        account_id: &str,
        account_name: &str,
        broker_account_id: &str,
        provider: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
        import_run_id: Option<String>,
    ) -> Result<ActivitySyncOutcome, String> {
        let limit = self.config.page_limit;
        let mut pages_fetched: usize = 0;
        let mut inconsistent_empty_page = false;

        let mut total_fetched: u32 = 0;
        let mut total_inserted: u32 = 0;
        let mut total_removed: u32 = 0;
        let mut total_assets_created: u32 = 0;
        let mut total_needs_review: u32 = 0;
        let mut all_new_asset_ids: Vec<String> = Vec::new();
        let normalized_provider = provider.trim().to_ascii_lowercase();
        let persist_checkpoint = normalized_provider != "snaptrade";
        let mut checkpoint = if persist_checkpoint {
            self.sync_service
                .get_activity_sync_state(account_id)
                .map_err(|e| format!("Failed to read activity sync checkpoint: {}", e))?
                .and_then(|state| state.checkpoint_json)
        } else {
            None
        };
        let mut final_checkpoint = if persist_checkpoint {
            checkpoint.clone()
        } else {
            None
        };

        loop {
            if pages_fetched >= self.config.max_pages {
                return Err(format!(
                    "Pagination exceeded max pages ({}). Aborting.",
                    self.config.max_pages
                ));
            }

            let previous_checkpoint = checkpoint.clone();
            let page = api_client
                .sync_account_activities(
                    broker_account_id,
                    Some(provider),
                    checkpoint.clone(),
                    start_date,
                    end_date,
                    Some(limit),
                )
                .await
                .map_err(|e| e.to_string())?;

            let data = page.activities;
            let removed = page.removed_activities;
            let removed_count = removed.len();
            checkpoint = page.checkpoint.clone();
            if persist_checkpoint {
                final_checkpoint = page.checkpoint.or(final_checkpoint);
            }
            pages_fetched += 1;
            total_fetched += data.len() as u32;

            self.progress_reporter.report_progress(
                SyncProgressPayload::new(account_id, account_name, SyncStatus::Syncing)
                    .with_page(pages_fetched)
                    .with_activities_fetched(total_fetched as usize)
                    .with_message(format!(
                        "Fetched {} activities, {} removed",
                        total_fetched, total_removed
                    )),
            );

            info!(
                "Fetched {} activities and {} removals for '{}' (page {})",
                data.len(),
                removed_count,
                account_name,
                pages_fetched
            );

            if !data.is_empty() {
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

            if !removed.is_empty() {
                let deleted = self
                    .sync_service
                    .remove_account_activities(account_id.to_string(), removed)
                    .await
                    .map_err(|e| format!("Failed to remove tombstoned activities: {}", e))?;
                total_removed += deleted as u32;
            }

            if data.is_empty() && removed_count == 0 && page.has_more {
                inconsistent_empty_page = true;
                break;
            }

            if !page.has_more {
                break;
            }

            if checkpoint == previous_checkpoint {
                return Err(
                    "Provider activity sync appears stuck (checkpoint did not advance).".to_string(),
                );
            }
        }

        Ok(ActivitySyncOutcome {
            fetched: total_fetched,
            inserted: total_inserted,
            removed: total_removed,
            assets_created: total_assets_created,
            needs_review: total_needs_review,
            new_asset_ids: all_new_asset_ids,
            checkpoint: final_checkpoint,
            inconsistent_empty_page,
        })
    }

    pub(super) fn compute_activity_query_window(
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
            has_local_cursor,
        })
    }
}
