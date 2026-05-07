-- ============================================================================
-- DEV DB PATCH — bring an existing dev database in line with the in-place
-- updates to migration 2026-05-04 spending_module:
--
--   1. Renamed table activity_rules → categorization_rules.
--   2. Added preset_id, preset_rule_key, preset_version, preset_modified columns.
--
-- Strategy: drop-and-recreate the table. Destructive — any rules already in
-- the dev DB will be lost. Acceptable for dev workflow; users will re-import
-- rules from the preset packs after upgrading.
--
-- Run via:  sqlite3 path/to/wealthfolio.db < dev_patch_preset_columns.sql
-- ============================================================================

BEGIN TRANSACTION;

-- 1. Drop the old table + its indexes (and any orphaned indexes from the new name).
DROP INDEX IF EXISTS idx_activity_rules_priority;
DROP INDEX IF EXISTS idx_activity_rules_category;
DROP INDEX IF EXISTS idx_activity_rules_account;
DROP INDEX IF EXISTS idx_activity_rules_is_global;
DROP INDEX IF EXISTS idx_activity_rules_activity_type;
DROP INDEX IF EXISTS idx_activity_rules_preset_unique;
DROP INDEX IF EXISTS idx_categorization_rules_priority;
DROP INDEX IF EXISTS idx_categorization_rules_category;
DROP INDEX IF EXISTS idx_categorization_rules_account;
DROP INDEX IF EXISTS idx_categorization_rules_is_global;
DROP INDEX IF EXISTS idx_categorization_rules_activity_type;
DROP INDEX IF EXISTS idx_categorization_rules_preset_unique;

DROP TABLE IF EXISTS activity_rules;
DROP TABLE IF EXISTS categorization_rules;

-- 2. Recreate at the canonical (post-rename, post-preset-columns) shape.
CREATE TABLE categorization_rules (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    pattern TEXT NOT NULL,
    match_type TEXT NOT NULL DEFAULT 'contains',  -- 'contains' | 'starts_with' | 'exact' | 'regex'
    taxonomy_id TEXT,
    category_id TEXT,
    activity_type TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    is_global INTEGER NOT NULL DEFAULT 1,
    account_id TEXT REFERENCES accounts(id) ON DELETE CASCADE,
    preset_id TEXT,
    preset_rule_key TEXT,
    preset_version TEXT,
    preset_modified INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    FOREIGN KEY (taxonomy_id, category_id)
      REFERENCES taxonomy_categories(taxonomy_id, id) ON DELETE SET NULL
);

CREATE INDEX idx_categorization_rules_priority      ON categorization_rules(priority DESC);
CREATE INDEX idx_categorization_rules_category      ON categorization_rules(taxonomy_id, category_id);
CREATE INDEX idx_categorization_rules_account       ON categorization_rules(account_id);
CREATE INDEX idx_categorization_rules_is_global     ON categorization_rules(is_global);
CREATE INDEX idx_categorization_rules_activity_type ON categorization_rules(activity_type);
CREATE UNIQUE INDEX idx_categorization_rules_preset_unique
  ON categorization_rules(preset_id, preset_rule_key)
  WHERE preset_id IS NOT NULL;

COMMIT;
