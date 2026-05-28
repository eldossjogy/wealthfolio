ALTER TABLE lots ADD COLUMN original_cost_basis_base TEXT NOT NULL DEFAULT '0';
ALTER TABLE lots ADD COLUMN remaining_cost_basis_base TEXT NOT NULL DEFAULT '0';
ALTER TABLE lots ADD COLUMN fee_allocated_base TEXT NOT NULL DEFAULT '0';
ALTER TABLE lots ADD COLUMN currency TEXT NOT NULL DEFAULT '';
ALTER TABLE lots ADD COLUMN base_currency TEXT NOT NULL DEFAULT '';
ALTER TABLE lots ADD COLUMN fx_rate_to_base TEXT NOT NULL DEFAULT '1';
ALTER TABLE lots ADD COLUMN cost_basis_method TEXT NOT NULL DEFAULT 'FIFO';

ALTER TABLE daily_account_valuation
ADD COLUMN external_flow_source TEXT NOT NULL DEFAULT 'UNKNOWN';

CREATE TABLE account_accounting_settings (
    account_id TEXT PRIMARY KEY NOT NULL,
    cost_basis_method TEXT NOT NULL DEFAULT 'FIFO',
    cost_basis_profile TEXT NOT NULL DEFAULT 'GENERIC',
    pooling_scope TEXT NOT NULL DEFAULT 'ACCOUNT',
    lot_selection_strategy TEXT NULL,
    settings_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

INSERT OR IGNORE INTO account_accounting_settings (
    account_id,
    cost_basis_method,
    cost_basis_profile,
    pooling_scope,
    lot_selection_strategy,
    settings_json,
    created_at,
    updated_at
)
SELECT
    id,
    'FIFO',
    'GENERIC',
    'ACCOUNT',
    NULL,
    '{}',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM accounts;

CREATE TABLE lot_disposals (
    id TEXT PRIMARY KEY NOT NULL,
    lot_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    disposal_activity_id TEXT NOT NULL,
    disposal_date TEXT NOT NULL,
    quantity TEXT NOT NULL,
    proceeds TEXT NOT NULL,
    cost_basis TEXT NOT NULL,
    realized_pnl TEXT NOT NULL,
    proceeds_base TEXT NOT NULL,
    cost_basis_base TEXT NOT NULL,
    realized_pnl_base TEXT NOT NULL,
    currency TEXT NOT NULL,
    base_currency TEXT NOT NULL,
    fx_rate_to_base TEXT NOT NULL,
    cost_basis_method TEXT NOT NULL DEFAULT 'FIFO',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    FOREIGN KEY (lot_id) REFERENCES lots(id) ON DELETE CASCADE,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE,
    FOREIGN KEY (disposal_activity_id) REFERENCES activities(id) ON DELETE CASCADE
);

CREATE INDEX idx_lot_disposals_account_date
    ON lot_disposals(account_id, disposal_date);

CREATE INDEX idx_lot_disposals_asset_date
    ON lot_disposals(asset_id, disposal_date);

CREATE INDEX idx_lot_disposals_activity
    ON lot_disposals(disposal_activity_id);

-- Lots, disposal slices, calculated snapshots, and daily valuations are
-- generated read models. Clear them when this generated schema changes so
-- startup detects missing generated history and backfills from the source
-- activity/import data with one consistent version.
DELETE FROM lot_disposals;
DELETE FROM lots;
DELETE FROM daily_account_valuation;
DELETE FROM holdings_snapshots
WHERE source = 'CALCULATED';
