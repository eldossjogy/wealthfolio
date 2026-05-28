-- Revert: remove review_frequency / next_review_date and restore original CHECK.

PRAGMA foreign_keys = OFF;

CREATE TABLE target_profiles_old (
    id              TEXT    PRIMARY KEY NOT NULL,
    name            TEXT    NOT NULL CHECK (length(trim(name)) > 0),
    status          TEXT    NOT NULL DEFAULT 'draft'
                            CHECK (status IN ('draft', 'active', 'archived')),
    scope_type      TEXT    NOT NULL CHECK (scope_type IN ('all', 'portfolio', 'account')),
    scope_id        TEXT,
    taxonomy_id     TEXT    NOT NULL DEFAULT 'asset_classes',
    base_currency   TEXT    NOT NULL,

    trigger_type    TEXT    NOT NULL DEFAULT 'threshold'
                            CHECK (trigger_type IN ('manual', 'threshold')),
    drift_band_bps  INTEGER NOT NULL DEFAULT 500
                            CHECK (drift_band_bps >= 0 AND drift_band_bps <= 10000),
    rebalance_to    TEXT    NOT NULL DEFAULT 'nearest_band'
                            CHECK (rebalance_to IN ('nearest_band', 'exact_target')),
    allow_sells     INTEGER NOT NULL DEFAULT 0,
    min_trade_amount TEXT   NOT NULL DEFAULT '0',
    whole_shares_only INTEGER NOT NULL DEFAULT 0,

    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

INSERT INTO target_profiles_old
SELECT id, name, status, scope_type, scope_id, taxonomy_id, base_currency,
       CASE WHEN trigger_type IN ('calendar','combined') THEN 'threshold' ELSE trigger_type END,
       drift_band_bps, rebalance_to, allow_sells, min_trade_amount, whole_shares_only,
       created_at, updated_at
FROM target_profiles;

DROP TABLE target_profiles;
ALTER TABLE target_profiles_old RENAME TO target_profiles;

CREATE UNIQUE INDEX idx_target_profiles_active_scope
ON target_profiles(scope_type, scope_id)
WHERE status = 'active';

CREATE INDEX idx_target_profiles_scope
ON target_profiles(scope_type, scope_id, status);

PRAGMA foreign_keys = ON;
