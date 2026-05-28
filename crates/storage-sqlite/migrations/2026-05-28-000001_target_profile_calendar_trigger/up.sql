-- Add calendar/combined trigger support to target_profiles.
-- SQLite cannot ALTER a CHECK constraint, so we recreate the table.

PRAGMA foreign_keys = OFF;

CREATE TABLE target_profiles_new (
    id              TEXT    PRIMARY KEY NOT NULL,
    name            TEXT    NOT NULL CHECK (length(trim(name)) > 0),
    status          TEXT    NOT NULL DEFAULT 'draft'
                            CHECK (status IN ('draft', 'active', 'archived')),
    scope_type      TEXT    NOT NULL CHECK (scope_type IN ('all', 'portfolio', 'account')),
    scope_id        TEXT,
    taxonomy_id     TEXT    NOT NULL DEFAULT 'asset_classes',
    base_currency   TEXT    NOT NULL,

    trigger_type    TEXT    NOT NULL DEFAULT 'threshold'
                            CHECK (trigger_type IN ('manual', 'threshold', 'calendar', 'combined')),
    drift_band_bps  INTEGER NOT NULL DEFAULT 500
                            CHECK (drift_band_bps >= 0 AND drift_band_bps <= 10000),
    review_frequency TEXT   CHECK (review_frequency IN ('monthly', 'quarterly', 'semi_annual', 'annual')),
    next_review_date TEXT,

    rebalance_to    TEXT    NOT NULL DEFAULT 'nearest_band'
                            CHECK (rebalance_to IN ('nearest_band', 'exact_target')),
    allow_sells     INTEGER NOT NULL DEFAULT 0,
    min_trade_amount TEXT   NOT NULL DEFAULT '0',
    whole_shares_only INTEGER NOT NULL DEFAULT 0,

    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

INSERT INTO target_profiles_new
SELECT id, name, status, scope_type, scope_id, taxonomy_id, base_currency,
       trigger_type, drift_band_bps, NULL, NULL,
       rebalance_to, allow_sells, min_trade_amount, whole_shares_only,
       created_at, updated_at
FROM target_profiles;

DROP TABLE target_profiles;
ALTER TABLE target_profiles_new RENAME TO target_profiles;

CREATE UNIQUE INDEX idx_target_profiles_active_scope
ON target_profiles(scope_type, scope_id)
WHERE status = 'active';

CREATE INDEX idx_target_profiles_scope
ON target_profiles(scope_type, scope_id, status);

PRAGMA foreign_keys = ON;
