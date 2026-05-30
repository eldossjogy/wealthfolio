-- Milestone 2: cash-flow rebalance planner.
-- Saved rebalance plans. JSON snapshot keeps drafts readable even after the target changes.
CREATE TABLE rebalance_drafts (
    id TEXT PRIMARY KEY NOT NULL,
    target_id TEXT NOT NULL,
    target_snapshot_json TEXT NOT NULL,
    input_json TEXT NOT NULL,
    result_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    FOREIGN KEY (target_id) REFERENCES allocation_targets(id) ON DELETE CASCADE
);

CREATE INDEX idx_rebalance_drafts_target
ON rebalance_drafts(target_id);
