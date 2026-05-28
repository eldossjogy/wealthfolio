CREATE TABLE target_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active', 'archived')),
    scope_type TEXT NOT NULL CHECK (scope_type IN ('all', 'portfolio', 'account')),
    scope_id TEXT,
    taxonomy_id TEXT NOT NULL DEFAULT 'asset_classes',
    base_currency TEXT NOT NULL,

    trigger_type TEXT NOT NULL DEFAULT 'threshold' CHECK (trigger_type IN ('manual', 'threshold')),
    drift_band_bps INTEGER NOT NULL DEFAULT 500 CHECK (drift_band_bps >= 0 AND drift_band_bps <= 10000),
    rebalance_to TEXT NOT NULL DEFAULT 'nearest_band' CHECK (rebalance_to IN ('nearest_band', 'exact_target')),

    allow_sells INTEGER NOT NULL DEFAULT 0,
    min_trade_amount TEXT NOT NULL DEFAULT '0',
    whole_shares_only INTEGER NOT NULL DEFAULT 0,

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE UNIQUE INDEX idx_target_profiles_active_scope
ON target_profiles(scope_type, scope_id)
WHERE status = 'active';

CREATE INDEX idx_target_profiles_scope
ON target_profiles(scope_type, scope_id, status);

CREATE TABLE target_allocation_nodes (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    target_bps INTEGER NOT NULL CHECK (target_bps >= 0 AND target_bps <= 10000),
    is_locked INTEGER NOT NULL DEFAULT 0,
    is_required INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    FOREIGN KEY (profile_id) REFERENCES target_profiles(id) ON DELETE CASCADE,
    UNIQUE(profile_id, category_id)
);

CREATE INDEX idx_target_allocation_nodes_profile
ON target_allocation_nodes(profile_id);

CREATE TABLE rebalance_drafts (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL,
    profile_snapshot_json TEXT NOT NULL,
    input_json TEXT NOT NULL,
    result_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    FOREIGN KEY (profile_id) REFERENCES target_profiles(id) ON DELETE CASCADE
);

CREATE INDEX idx_rebalance_drafts_profile
ON rebalance_drafts(profile_id, created_at);
