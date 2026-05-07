use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpendingSettings {
    pub enabled: bool,
    pub account_ids: Vec<String>,
}

impl Default for SpendingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            account_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpendingSettingsUpdate {
    pub enabled: Option<bool>,
    pub account_ids: Option<Vec<String>>,
}
