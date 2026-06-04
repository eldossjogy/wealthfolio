CREATE TABLE allocation_targets (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    scope_type TEXT NOT NULL CHECK (scope_type IN ('all', 'portfolio', 'account')),
    scope_id TEXT,
    taxonomy_id TEXT NOT NULL DEFAULT 'asset_classes',

    trigger_type TEXT NOT NULL DEFAULT 'threshold' CHECK (trigger_type IN ('manual', 'threshold')),
    drift_band_bps INTEGER NOT NULL DEFAULT 500 CHECK (drift_band_bps >= 0 AND drift_band_bps <= 10000),
    rebalance_goal TEXT NOT NULL DEFAULT 'nearest_band'
        CHECK (rebalance_goal IN ('nearest_band', 'exact_target')),
    min_trade_amount TEXT NOT NULL DEFAULT '0',
    whole_shares_only INTEGER NOT NULL DEFAULT 0,
    allow_sells INTEGER NOT NULL DEFAULT 0,

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    archived_at TEXT,

    CHECK (
        (scope_type = 'all' AND scope_id IS NULL) OR
        (scope_type IN ('account', 'portfolio') AND scope_id IS NOT NULL)
    )
);

CREATE INDEX idx_allocation_targets_scope
ON allocation_targets(scope_type, scope_id, archived_at);

CREATE TABLE allocation_target_weights (
    id TEXT PRIMARY KEY NOT NULL,
    target_id TEXT NOT NULL,
    taxonomy_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    target_bps INTEGER NOT NULL CHECK (target_bps >= 0 AND target_bps <= 10000),
    is_locked INTEGER NOT NULL DEFAULT 0,
    is_required INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    FOREIGN KEY (target_id) REFERENCES allocation_targets(id) ON DELETE CASCADE,
    FOREIGN KEY (category_id, taxonomy_id) REFERENCES taxonomy_categories(id, taxonomy_id) ON DELETE RESTRICT,
    UNIQUE(target_id, taxonomy_id, category_id)
);

CREATE INDEX idx_allocation_target_weights_target
ON allocation_target_weights(target_id);

CREATE TRIGGER allocation_targets_taxonomy_update
BEFORE UPDATE OF taxonomy_id ON allocation_targets
FOR EACH ROW
WHEN OLD.taxonomy_id <> NEW.taxonomy_id
    AND EXISTS (
        SELECT 1 FROM allocation_target_weights
        WHERE target_id = OLD.id
    )
BEGIN
    SELECT RAISE(ABORT, 'allocation_targets.taxonomy_id cannot change while weights exist');
END;

CREATE TRIGGER allocation_target_weights_taxonomy_insert
BEFORE INSERT ON allocation_target_weights
FOR EACH ROW
WHEN (SELECT taxonomy_id FROM allocation_targets WHERE id = NEW.target_id) <> NEW.taxonomy_id
BEGIN
    SELECT RAISE(ABORT, 'allocation_target_weights.taxonomy_id must match allocation_targets.taxonomy_id');
END;

CREATE TRIGGER allocation_target_weights_taxonomy_update
BEFORE UPDATE OF target_id, taxonomy_id ON allocation_target_weights
FOR EACH ROW
WHEN (SELECT taxonomy_id FROM allocation_targets WHERE id = NEW.target_id) <> NEW.taxonomy_id
BEGIN
    SELECT RAISE(ABORT, 'allocation_target_weights.taxonomy_id must match allocation_targets.taxonomy_id');
END;
