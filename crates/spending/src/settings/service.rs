use std::sync::Arc;

use anyhow::Result;

use super::model::{SpendingSettings, SpendingSettingsUpdate};
use super::traits::SpendingSettingsRepositoryTrait;
use super::{SETTING_KEY_ACCOUNT_IDS, SETTING_KEY_ENABLED};

pub struct SpendingSettingsService {
    repo: Arc<dyn SpendingSettingsRepositoryTrait>,
}

impl SpendingSettingsService {
    pub fn new(repo: Arc<dyn SpendingSettingsRepositoryTrait>) -> Self {
        Self { repo }
    }

    pub async fn get(&self) -> Result<SpendingSettings> {
        let enabled = self
            .repo
            .get_setting(SETTING_KEY_ENABLED)
            .await?
            .map(|s| s == "true")
            .unwrap_or(false);

        let account_ids = self
            .repo
            .get_setting(SETTING_KEY_ACCOUNT_IDS)
            .await?
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .unwrap_or_default();

        Ok(SpendingSettings {
            enabled,
            account_ids,
        })
    }

    pub async fn update(&self, patch: SpendingSettingsUpdate) -> Result<SpendingSettings> {
        if let Some(enabled) = patch.enabled {
            self.repo
                .set_setting(SETTING_KEY_ENABLED, if enabled { "true" } else { "false" })
                .await?;
        }
        if let Some(ids) = patch.account_ids {
            let json = serde_json::to_string(&ids)?;
            self.repo
                .set_setting(SETTING_KEY_ACCOUNT_IDS, &json)
                .await?;
        }
        self.get().await
    }

    /// Auto-include a new CASH account in the spending-tracked set when spending
    /// is enabled. No-op when disabled.
    pub async fn auto_include_account(&self, account_id: &str) -> Result<()> {
        let mut current = self.get().await?;
        if !current.enabled {
            return Ok(());
        }
        if !current.account_ids.iter().any(|id| id == account_id) {
            current.account_ids.push(account_id.to_string());
            let json = serde_json::to_string(&current.account_ids)?;
            self.repo
                .set_setting(SETTING_KEY_ACCOUNT_IDS, &json)
                .await?;
        }
        Ok(())
    }
}
