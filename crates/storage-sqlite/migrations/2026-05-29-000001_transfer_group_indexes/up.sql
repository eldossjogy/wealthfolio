CREATE INDEX IF NOT EXISTS ix_activities_source_group_id
ON activities(source_group_id)
WHERE source_group_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS ix_activities_transfer_scope
ON activities(account_id, activity_date, status)
WHERE COALESCE(activity_type_override, activity_type) IN ('TRANSFER_IN', 'TRANSFER_OUT');
