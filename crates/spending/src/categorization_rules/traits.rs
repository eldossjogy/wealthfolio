use anyhow::Result;
use async_trait::async_trait;

use super::model::{CategorizationRule, NewCategorizationRule, UpdateCategorizationRule};

#[async_trait]
pub trait CategorizationRulesRepositoryTrait: Send + Sync {
    async fn list(&self) -> Result<Vec<CategorizationRule>>;
    async fn get(&self, id: &str) -> Result<Option<CategorizationRule>>;
    async fn create(&self, new_rule: NewCategorizationRule) -> Result<CategorizationRule>;
    async fn update(&self, id: &str, patch: UpdateCategorizationRule)
        -> Result<CategorizationRule>;
    /// Replace a bundled preset rule without marking it as user-modified.
    async fn replace_preset_rule(
        &self,
        id: &str,
        rule: NewCategorizationRule,
    ) -> Result<CategorizationRule>;
    async fn delete(&self, id: &str) -> Result<()>;
    /// Remove all rules originating from `preset_id`. Unmodified rules are
    /// deleted; user-modified rules are detached (preset metadata cleared) so
    /// they survive as standalone user rules. Returns `(removed, kept_modified)`.
    async fn remove_preset(&self, preset_id: &str) -> Result<(usize, usize)>;
}
