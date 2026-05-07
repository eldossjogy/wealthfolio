-- Reverse the spending_module migration.
-- Order: drop FK-dependent objects first.

-- Drop budget tables
DROP INDEX IF EXISTS idx_budget_allocations_category;
DROP INDEX IF EXISTS idx_budget_allocations_config;
DROP TABLE IF EXISTS budget_allocations;
DROP TABLE IF EXISTS budget_config;

-- Drop categorization_rules
DROP INDEX IF EXISTS idx_categorization_rules_preset_unique;
DROP INDEX IF EXISTS idx_categorization_rules_activity_type;
DROP INDEX IF EXISTS idx_categorization_rules_is_global;
DROP INDEX IF EXISTS idx_categorization_rules_account;
DROP INDEX IF EXISTS idx_categorization_rules_category;
DROP INDEX IF EXISTS idx_categorization_rules_priority;
DROP TABLE IF EXISTS categorization_rules;

-- Drop activities.event_id
DROP INDEX IF EXISTS idx_activities_event;
ALTER TABLE activities DROP COLUMN event_id;

-- Drop events + event_types
DROP INDEX IF EXISTS idx_events_dates;
DROP INDEX IF EXISTS idx_events_event_type;
DROP TABLE IF EXISTS events;
DROP TABLE IF EXISTS event_types;

-- Drop activity_taxonomy_assignments
DROP INDEX IF EXISTS ix_activity_taxonomy_assignment_unique;
DROP INDEX IF EXISTS ix_activity_taxonomy_assignments_category;
DROP INDEX IF EXISTS ix_activity_taxonomy_assignments_activity;
DROP TABLE IF EXISTS activity_taxonomy_assignments;

-- Drop seeded activity-scope categories + taxonomies
DELETE FROM taxonomy_categories WHERE taxonomy_id IN ('spending_categories', 'income_sources');
DELETE FROM taxonomies WHERE id IN ('spending_categories', 'income_sources');

-- Reverse taxonomy_categories.icon
ALTER TABLE taxonomy_categories DROP COLUMN icon;

-- Reverse taxonomies.scope
DROP INDEX IF EXISTS ix_taxonomies_scope;
ALTER TABLE taxonomies DROP COLUMN scope;
