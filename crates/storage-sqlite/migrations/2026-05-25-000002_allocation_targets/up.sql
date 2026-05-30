CREATE TABLE target_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active', 'archived')),
    scope_type TEXT NOT NULL CHECK (scope_type IN ('all', 'portfolio', 'account')),
    scope_id TEXT,
    taxonomy_id TEXT NOT NULL DEFAULT 'asset_classes',

    trigger_type TEXT NOT NULL DEFAULT 'threshold' CHECK (trigger_type IN ('manual', 'threshold')),
    drift_band_bps INTEGER NOT NULL DEFAULT 500 CHECK (drift_band_bps >= 0 AND drift_band_bps <= 10000),

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),

    CHECK (
        (scope_type = 'all' AND scope_id IS NULL) OR
        (scope_type IN ('account', 'portfolio') AND scope_id IS NOT NULL)
    )
);

-- COALESCE fixes SQLite NULL-in-unique-index: multiple NULL scope_ids would all match
CREATE UNIQUE INDEX idx_target_profiles_active_scope
ON target_profiles(scope_type, COALESCE(scope_id, ''))
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
