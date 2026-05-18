-- Spending Module Migration
-- Adds optional cash-tracking / expense management on top of existing tables.
-- Re-uses the taxonomies system (scope='activity') instead of forking a Categories table.
--
-- Adds:
--   - Columns: taxonomies.scope, taxonomy_categories.icon, activities.event_id
--   - Tables: activity_taxonomy_assignments, event_types, events, categorization_rules,
--             budget_groups, budget_group_assignments, budget_targets,
--             budget_rollover_settings
--   - Seeds: 'Spending Categories' + 'Income Sources' system taxonomies (scope='activity')
--           with full category trees ported from PR #494; 7 default event types
--
-- Notes on intentional non-additions:
--   - `accounts` untouched: account names + spending opt-in list cover checking/savings/credit.
--   - `activities.name` not added: existing `notes` column carries payee/merchant string
--     (rules pattern-match on it; CSV import maps Description → notes).

-- ============================================================================
-- 1. EXTEND TAXONOMIES: scope + icon
-- ============================================================================

ALTER TABLE taxonomies ADD COLUMN scope TEXT NOT NULL DEFAULT 'asset';
CREATE INDEX ix_taxonomies_scope ON taxonomies(scope);

ALTER TABLE taxonomy_categories ADD COLUMN icon TEXT;

-- ============================================================================
-- 2. ACTIVITY_TAXONOMY_ASSIGNMENTS (mirrors asset_taxonomy_assignments)
-- ============================================================================

CREATE TABLE activity_taxonomy_assignments (
    id TEXT NOT NULL PRIMARY KEY,
    activity_id TEXT NOT NULL,
    taxonomy_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    weight INTEGER NOT NULL DEFAULT 10000,
    source TEXT NOT NULL DEFAULT 'manual',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    FOREIGN KEY (activity_id) REFERENCES activities(id) ON DELETE CASCADE,
    FOREIGN KEY (taxonomy_id, category_id) REFERENCES taxonomy_categories(taxonomy_id, id) ON DELETE CASCADE,

    CHECK (weight >= 0 AND weight <= 10000)
);

CREATE INDEX ix_activity_taxonomy_assignments_activity ON activity_taxonomy_assignments(activity_id);
CREATE INDEX ix_activity_taxonomy_assignments_category ON activity_taxonomy_assignments(taxonomy_id, category_id);
CREATE UNIQUE INDEX ix_activity_taxonomy_assignment_unique ON activity_taxonomy_assignments(activity_id, taxonomy_id, category_id);

-- ============================================================================
-- 3. EVENT_TYPES (lookup) + EVENTS
-- ============================================================================

CREATE TABLE event_types (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    color TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE events (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    event_type_id TEXT NOT NULL REFERENCES event_types(id) ON DELETE RESTRICT,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_events_event_type ON events(event_type_id);
CREATE INDEX idx_events_dates ON events(start_date, end_date);

-- ============================================================================
-- 4. ACTIVITIES.EVENT_ID (FK to events)
-- ============================================================================

ALTER TABLE activities ADD COLUMN event_id TEXT REFERENCES events(id) ON DELETE SET NULL;
CREATE INDEX idx_activities_event ON activities(event_id);

-- ============================================================================
-- 5. CATEGORIZATION_RULES (auto-categorization on create / import)
--    References taxonomy_categories via composite FK.
-- ============================================================================

CREATE TABLE categorization_rules (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    pattern TEXT NOT NULL,
    match_type TEXT NOT NULL DEFAULT 'contains',  -- 'contains' | 'starts_with' | 'exact' | 'regex'
    taxonomy_id TEXT,
    category_id TEXT,
    activity_type TEXT,                            -- optional: BUY | WITHDRAWAL | DEPOSIT | ...
    priority INTEGER NOT NULL DEFAULT 0,
    is_global INTEGER NOT NULL DEFAULT 1,
    account_id TEXT REFERENCES accounts(id) ON DELETE CASCADE,
    -- Preset provenance (NULL for user-created rules):
    --   preset_id        — short slug of the source preset, e.g. "ca", "us"
    --   preset_rule_key  — stable per-rule key used for diff/update across versions
    --   preset_version   — preset version installed for this rule
    --   preset_modified  — flips to 1 when the user edits a preset-sourced rule, so
    --                      future updates ask before overwriting
    preset_id TEXT,
    preset_rule_key TEXT,
    preset_version TEXT,
    preset_modified INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    FOREIGN KEY (taxonomy_id, category_id) REFERENCES taxonomy_categories(taxonomy_id, id) ON DELETE SET NULL
);

CREATE INDEX idx_categorization_rules_priority ON categorization_rules(priority DESC);
CREATE INDEX idx_categorization_rules_category ON categorization_rules(taxonomy_id, category_id);
CREATE INDEX idx_categorization_rules_account ON categorization_rules(account_id);
CREATE INDEX idx_categorization_rules_is_global ON categorization_rules(is_global);
CREATE INDEX idx_categorization_rules_activity_type ON categorization_rules(activity_type);
-- Used by the preset update flow to look up "the user's installed copy of preset rule X".
-- NULL preset_id rows (user-created) are excluded from the unique index.
CREATE UNIQUE INDEX idx_categorization_rules_preset_unique
  ON categorization_rules(preset_id, preset_rule_key)
  WHERE preset_id IS NOT NULL;

-- ============================================================================
-- 6. BUDGET tables
-- ============================================================================

CREATE TABLE budget_groups (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    key TEXT NOT NULL UNIQUE,
    color TEXT,
    icon TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    is_system INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (is_system IN (0, 1))
);

CREATE INDEX idx_budget_groups_sort ON budget_groups(sort_order);

CREATE TABLE budget_group_assignments (
    id TEXT NOT NULL PRIMARY KEY,
    group_id TEXT NOT NULL,
    taxonomy_id TEXT NOT NULL DEFAULT 'spending_categories',
    category_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    FOREIGN KEY (group_id) REFERENCES budget_groups(id) ON DELETE CASCADE,
    FOREIGN KEY (taxonomy_id, category_id) REFERENCES taxonomy_categories(taxonomy_id, id) ON DELETE CASCADE,

    CHECK (taxonomy_id = 'spending_categories'),
    UNIQUE (taxonomy_id, category_id)
);

CREATE INDEX idx_budget_group_assignments_group ON budget_group_assignments(group_id);
CREATE INDEX idx_budget_group_assignments_category ON budget_group_assignments(taxonomy_id, category_id);

CREATE TABLE budget_targets (
    id TEXT NOT NULL PRIMARY KEY,
    period_key TEXT NOT NULL,
    target_type TEXT NOT NULL,
    taxonomy_id TEXT,
    category_id TEXT,
    group_id TEXT,
    amount TEXT NOT NULL DEFAULT '0',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    FOREIGN KEY (taxonomy_id, category_id) REFERENCES taxonomy_categories(taxonomy_id, id) ON DELETE CASCADE,
    FOREIGN KEY (group_id) REFERENCES budget_groups(id) ON DELETE CASCADE,

    CHECK (target_type IN ('category', 'group_buffer')),
    CHECK (
      period_key = 'default'
      OR period_key GLOB '[0-9][0-9][0-9][0-9]-0[1-9]'
      OR period_key GLOB '[0-9][0-9][0-9][0-9]-1[0-2]'
    ),
    CHECK (
      (target_type = 'category' AND taxonomy_id IS NOT NULL AND category_id IS NOT NULL AND group_id IS NULL)
      OR
      (target_type = 'group_buffer' AND taxonomy_id IS NULL AND category_id IS NULL AND group_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_budget_targets_category_unique
  ON budget_targets(period_key, taxonomy_id, category_id)
  WHERE target_type = 'category';
CREATE UNIQUE INDEX idx_budget_targets_group_buffer_unique
  ON budget_targets(period_key, group_id)
  WHERE target_type = 'group_buffer';
CREATE INDEX idx_budget_targets_period ON budget_targets(period_key);
CREATE INDEX idx_budget_targets_category ON budget_targets(taxonomy_id, category_id);
CREATE INDEX idx_budget_targets_group ON budget_targets(group_id);

CREATE TABLE budget_rollover_settings (
    id TEXT NOT NULL PRIMARY KEY,
    target_type TEXT NOT NULL,
    taxonomy_id TEXT,
    category_id TEXT,
    group_id TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    start_month TEXT NOT NULL,
    starting_balance TEXT NOT NULL DEFAULT '0',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    FOREIGN KEY (taxonomy_id, category_id) REFERENCES taxonomy_categories(taxonomy_id, id) ON DELETE CASCADE,
    FOREIGN KEY (group_id) REFERENCES budget_groups(id) ON DELETE CASCADE,

    CHECK (target_type IN ('category', 'group')),
    CHECK (enabled IN (0, 1)),
    CHECK (
      start_month GLOB '[0-9][0-9][0-9][0-9]-0[1-9]'
      OR start_month GLOB '[0-9][0-9][0-9][0-9]-1[0-2]'
    ),
    CHECK (
      (target_type = 'category' AND taxonomy_id = 'spending_categories' AND category_id IS NOT NULL AND group_id IS NULL)
      OR
      (target_type = 'group' AND taxonomy_id IS NULL AND category_id IS NULL AND group_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_budget_rollover_settings_category_unique
  ON budget_rollover_settings(taxonomy_id, category_id)
  WHERE target_type = 'category';
CREATE UNIQUE INDEX idx_budget_rollover_settings_group_unique
  ON budget_rollover_settings(group_id)
  WHERE target_type = 'group';
CREATE INDEX idx_budget_rollover_settings_category ON budget_rollover_settings(taxonomy_id, category_id);
CREATE INDEX idx_budget_rollover_settings_group ON budget_rollover_settings(group_id);

-- ============================================================================
-- 7. SEED: BUDGET GROUPS + SYSTEM TAXONOMIES with scope='activity'
-- ============================================================================

-- Color palette: muted/earthy Flexoki-adjacent hues lifted from the spending
-- tracker design. Categories without a direct design counterpart fall back to
-- Flexoki 500/600 shades (see apps/frontend/src/globals.css).
INSERT INTO budget_groups (id, name, key, color, icon, sort_order, is_system) VALUES
  ('budget_group_needs',    'Needs',    'needs',    '#4F6B92', 'Home',          1, 1),
  ('budget_group_wants',    'Wants',    'wants',    '#8E7CB3', 'Sparkles',      2, 1),
  ('budget_group_savings',  'Savings',  'savings',  '#6B8E54', 'PiggyBank',     3, 1),
  ('budget_group_giving',   'Giving',   'giving',   '#A35742', 'Gift',          4, 1),
  ('budget_group_personal', 'Personal', 'personal', '#B89A4C', 'User',          5, 1),
  ('budget_group_other',    'Other',    'other',    '#9C998E', 'MoreHorizontal',99, 1);

INSERT INTO taxonomies (id, name, color, description, is_system, is_single_select, sort_order, scope)
VALUES
  ('spending_categories', 'Spending Categories', '#B0552E',
   'Hierarchical expense categories (Food, Transport, Housing, …) used to classify cash withdrawals and outgoing transfers.',
   1, 1, 200, 'activity'),
  ('income_sources', 'Income Sources', '#5A7A3E',
   'Income classification (Salary, Dividends, Rental, …) used to classify cash deposits.',
   1, 1, 210, 'activity');

-- ----------------------------------------------------------------------------
-- 9a. Spending Categories: TOP LEVEL (parents)
-- ----------------------------------------------------------------------------

INSERT INTO taxonomy_categories (id, taxonomy_id, parent_id, name, key, color, icon, sort_order) VALUES
  ('cat_housing',         'spending_categories', NULL, 'Housing',          'housing',         '#A35742', 'Home',           1),
  ('cat_groceries',       'spending_categories', NULL, 'Groceries',        'groceries',       '#5A7A3E', 'ShoppingCart',   2),
  ('cat_food',            'spending_categories', NULL, 'Food & Dining',    'food',            '#B89A4C', 'UtensilsCrossed',3),
  ('cat_transport',       'spending_categories', NULL, 'Transportation',   'transport',       '#7B96C9', 'Car',            4),
  ('cat_shopping',        'spending_categories', NULL, 'Shopping',         'shopping',        '#8E7CB3', 'ShoppingBag',    5),
  ('cat_entertainment',   'spending_categories', NULL, 'Entertainment',    'entertainment',   '#B0552E', 'Film',           6),
  ('cat_health',          'spending_categories', NULL, 'Health & Wellness','health',          '#6B8E54', 'Heart',          7),
  ('cat_bills',           'spending_categories', NULL, 'Bills & Utilities','bills',           '#4F6B92', 'FileText',       8),
  ('cat_personal',        'spending_categories', NULL, 'Personal Care',    'personal',        '#B74583', 'User',           9),
  ('cat_education',       'spending_categories', NULL, 'Education',        'education',       '#24837B', 'GraduationCap', 10),
  ('cat_travel',          'spending_categories', NULL, 'Travel',           'travel',          '#3171B2', 'Plane',         11),
  ('cat_gifts',           'spending_categories', NULL, 'Gifts & Donations','gifts',           '#AF3029', 'Gift',          12),
  ('cat_fees',            'spending_categories', NULL, 'Fees & Charges',   'fees',            '#9C998E', 'CreditCard',    13),
  ('cat_other_expense',   'spending_categories', NULL, 'Other Expenses',   'other_expense',   '#B6B2A4', 'MoreHorizontal',99);

INSERT INTO budget_group_assignments (id, group_id, taxonomy_id, category_id) VALUES
  ('bga_cat_housing',       'budget_group_needs',    'spending_categories', 'cat_housing'),
  ('bga_cat_groceries',     'budget_group_needs',    'spending_categories', 'cat_groceries'),
  ('bga_cat_transport',     'budget_group_needs',    'spending_categories', 'cat_transport'),
  ('bga_cat_health',        'budget_group_needs',    'spending_categories', 'cat_health'),
  ('bga_cat_bills',         'budget_group_needs',    'spending_categories', 'cat_bills'),
  ('bga_cat_fees',          'budget_group_needs',    'spending_categories', 'cat_fees'),
  ('bga_cat_education',     'budget_group_needs',    'spending_categories', 'cat_education'),
  ('bga_cat_food',          'budget_group_wants',    'spending_categories', 'cat_food'),
  ('bga_cat_shopping',      'budget_group_wants',    'spending_categories', 'cat_shopping'),
  ('bga_cat_entertainment', 'budget_group_wants',    'spending_categories', 'cat_entertainment'),
  ('bga_cat_travel',        'budget_group_wants',    'spending_categories', 'cat_travel'),
  ('bga_cat_gifts',         'budget_group_giving',   'spending_categories', 'cat_gifts'),
  ('bga_cat_personal',      'budget_group_personal', 'spending_categories', 'cat_personal'),
  ('bga_cat_other_expense', 'budget_group_other',    'spending_categories', 'cat_other_expense');

-- ----------------------------------------------------------------------------
-- 9b. Spending Categories: SUBCATEGORIES
-- ----------------------------------------------------------------------------

-- Housing
INSERT INTO taxonomy_categories (id, taxonomy_id, parent_id, name, key, color, icon, sort_order) VALUES
  ('cat_housing_rent',        'spending_categories', 'cat_housing', 'Rent/Mortgage',         'housing_rent',        '#A35742', 'Home',     1),
  ('cat_housing_utilities',   'spending_categories', 'cat_housing', 'Utilities',             'housing_utilities',   '#A35742', 'Lightbulb',2),
  ('cat_housing_insurance',   'spending_categories', 'cat_housing', 'Home Insurance',        'housing_insurance',   '#A35742', 'Shield',   3),
  ('cat_housing_maintenance', 'spending_categories', 'cat_housing', 'Maintenance & Repairs', 'housing_maintenance', '#A35742', 'Wrench',   4),
  ('cat_housing_furnishing',  'spending_categories', 'cat_housing', 'Furnishing',            'housing_furnishing',  '#A35742', 'Sofa',     5);

-- Food & Dining (eating out — Groceries lifted to its own top-level since
-- modern PFM apps universally treat it as separate from "dining out".)
INSERT INTO taxonomy_categories (id, taxonomy_id, parent_id, name, key, color, icon, sort_order) VALUES
  ('cat_food_restaurants', 'spending_categories', 'cat_food', 'Restaurants',     'food_restaurants', '#B89A4C', 'UtensilsCrossed', 1),
  ('cat_food_coffee',      'spending_categories', 'cat_food', 'Coffee Shops',    'food_coffee',      '#B89A4C', 'Coffee',          2),
  ('cat_food_delivery',    'spending_categories', 'cat_food', 'Food Delivery',   'food_delivery',    '#B89A4C', 'Truck',           3),
  ('cat_food_alcohol',     'spending_categories', 'cat_food', 'Bars & Alcohol',  'food_alcohol',     '#B89A4C', 'Wine',            4);

-- Transportation
INSERT INTO taxonomy_categories (id, taxonomy_id, parent_id, name, key, color, icon, sort_order) VALUES
  ('cat_transport_gas',         'spending_categories', 'cat_transport', 'Gas & Fuel',       'transport_gas',         '#7B96C9', 'Fuel',          1),
  ('cat_transport_parking',     'spending_categories', 'cat_transport', 'Parking',          'transport_parking',     '#7B96C9', 'ParkingCircle', 2),
  ('cat_transport_public',      'spending_categories', 'cat_transport', 'Public Transit',   'transport_public',      '#7B96C9', 'Train',         3),
  ('cat_transport_rideshare',   'spending_categories', 'cat_transport', 'Rideshare & Taxi', 'transport_rideshare',   '#7B96C9', 'Car',           4),
  ('cat_transport_maintenance', 'spending_categories', 'cat_transport', 'Car Maintenance',  'transport_maintenance', '#7B96C9', 'Wrench',        5),
  ('cat_transport_insurance',   'spending_categories', 'cat_transport', 'Auto Insurance',   'transport_insurance',   '#7B96C9', 'Shield',        6);

-- Shopping
INSERT INTO taxonomy_categories (id, taxonomy_id, parent_id, name, key, color, icon, sort_order) VALUES
  ('cat_shopping_clothing',    'spending_categories', 'cat_shopping', 'Clothing',        'shopping_clothing',    '#8E7CB3', 'Shirt',      1),
  ('cat_shopping_electronics', 'spending_categories', 'cat_shopping', 'Electronics',     'shopping_electronics', '#8E7CB3', 'Smartphone', 2),
  ('cat_shopping_home',        'spending_categories', 'cat_shopping', 'Home Goods',      'shopping_home',        '#8E7CB3', 'Home',       3),
  ('cat_shopping_online',      'spending_categories', 'cat_shopping', 'Online Shopping', 'shopping_online',      '#8E7CB3', 'Globe',      4);

-- Entertainment
INSERT INTO taxonomy_categories (id, taxonomy_id, parent_id, name, key, color, icon, sort_order) VALUES
  ('cat_entertainment_streaming', 'spending_categories', 'cat_entertainment', 'Streaming Services',  'entertainment_streaming', '#B0552E', 'Tv',       1),
  ('cat_entertainment_movies',    'spending_categories', 'cat_entertainment', 'Movies & Events',     'entertainment_movies',    '#B0552E', 'Film',     2),
  ('cat_entertainment_games',     'spending_categories', 'cat_entertainment', 'Games & Apps',        'entertainment_games',     '#B0552E', 'Gamepad2', 3),
  ('cat_entertainment_hobbies',   'spending_categories', 'cat_entertainment', 'Hobbies',             'entertainment_hobbies',   '#B0552E', 'Palette',  4),
  ('cat_entertainment_sports',    'spending_categories', 'cat_entertainment', 'Sports & Recreation', 'entertainment_sports',    '#B0552E', 'Dumbbell', 5);

-- Health & Wellness
INSERT INTO taxonomy_categories (id, taxonomy_id, parent_id, name, key, color, icon, sort_order) VALUES
  ('cat_health_medical',   'spending_categories', 'cat_health', 'Medical',          'health_medical',   '#6B8E54', 'Stethoscope', 1),
  ('cat_health_pharmacy',  'spending_categories', 'cat_health', 'Pharmacy',         'health_pharmacy',  '#6B8E54', 'Pill',        2),
  ('cat_health_dental',    'spending_categories', 'cat_health', 'Dental',           'health_dental',    '#6B8E54', 'Smile',       3),
  ('cat_health_vision',    'spending_categories', 'cat_health', 'Vision',           'health_vision',    '#6B8E54', 'Eye',         4),
  ('cat_health_fitness',   'spending_categories', 'cat_health', 'Gym & Fitness',    'health_fitness',   '#6B8E54', 'Dumbbell',    5),
  ('cat_health_insurance', 'spending_categories', 'cat_health', 'Health Insurance', 'health_insurance', '#6B8E54', 'Shield',      6);

-- Bills & Utilities
INSERT INTO taxonomy_categories (id, taxonomy_id, parent_id, name, key, color, icon, sort_order) VALUES
  ('cat_bills_phone',         'spending_categories', 'cat_bills', 'Phone',               'bills_phone',         '#4F6B92', 'Smartphone', 1),
  ('cat_bills_internet',      'spending_categories', 'cat_bills', 'Internet',            'bills_internet',      '#4F6B92', 'Wifi',       2),
  ('cat_bills_subscriptions', 'spending_categories', 'cat_bills', 'Subscriptions',       'bills_subscriptions', '#4F6B92', 'Calendar',   3),
  ('cat_bills_software',      'spending_categories', 'cat_bills', 'Software & Services', 'bills_software',      '#4F6B92', 'Code',       4);

-- Fees & Charges
INSERT INTO taxonomy_categories (id, taxonomy_id, parent_id, name, key, color, icon, sort_order) VALUES
  ('cat_fees_bank',     'spending_categories', 'cat_fees', 'Bank Fees',        'fees_bank',     '#9C998E', 'Building',    1),
  ('cat_fees_atm',      'spending_categories', 'cat_fees', 'ATM Fees',         'fees_atm',      '#9C998E', 'Banknote',    2),
  ('cat_fees_interest', 'spending_categories', 'cat_fees', 'Interest Charges', 'fees_interest', '#9C998E', 'Percent',     3),
  ('cat_fees_late',     'spending_categories', 'cat_fees', 'Late Fees',        'fees_late',     '#9C998E', 'AlertCircle', 4);

-- ----------------------------------------------------------------------------
-- 9c. Income Sources: TOP LEVEL (parents)
-- ----------------------------------------------------------------------------

INSERT INTO taxonomy_categories (id, taxonomy_id, parent_id, name, key, color, icon, sort_order) VALUES
  ('cat_income_employment',  'income_sources', NULL, 'Employment',        'income_employment',  '#5A7A3E', 'Briefcase',  1),
  ('cat_income_selfemploy',  'income_sources', NULL, 'Self-Employment',   'income_selfemploy',  '#6B8E54', 'User',       2),
  ('cat_income_investment',  'income_sources', NULL, 'Investment Income', 'income_investment',  '#B89A4C', 'TrendingUp', 3),
  ('cat_income_other',       'income_sources', NULL, 'Other Income',      'income_other',       '#9C998E', 'DollarSign', 4);

-- Income subcategories
INSERT INTO taxonomy_categories (id, taxonomy_id, parent_id, name, key, color, icon, sort_order) VALUES
  ('cat_income_salary',         'income_sources', 'cat_income_employment', 'Salary',         'income_salary',         '#5A7A3E', 'Briefcase',  1),
  ('cat_income_bonus',          'income_sources', 'cat_income_employment', 'Bonus',          'income_bonus',          '#5A7A3E', 'Award',      2),
  ('cat_income_commission',     'income_sources', 'cat_income_employment', 'Commission',     'income_commission',     '#5A7A3E', 'Target',     3),
  ('cat_income_freelance',      'income_sources', 'cat_income_selfemploy', 'Freelance',      'income_freelance',      '#6B8E54', 'Laptop',     1),
  ('cat_income_business',       'income_sources', 'cat_income_selfemploy', 'Business Income','income_business',       '#6B8E54', 'Building',   2),
  ('cat_income_dividends',      'income_sources', 'cat_income_investment', 'Dividends',      'income_dividends',      '#B89A4C', 'PiggyBank',  1),
  ('cat_income_interest',       'income_sources', 'cat_income_investment', 'Interest',       'income_interest',       '#B89A4C', 'Percent',    2),
  ('cat_income_rental',         'income_sources', 'cat_income_investment', 'Rental Income',  'income_rental',         '#B89A4C', 'Home',       3),
  ('cat_income_capital_gains',  'income_sources', 'cat_income_investment', 'Capital Gains',  'income_capital_gains',  '#B89A4C', 'TrendingUp', 4),
  ('cat_income_gifts',          'income_sources', 'cat_income_other',      'Gifts Received', 'income_gifts',          '#9C998E', 'Gift',       1),
  ('cat_income_refunds',        'income_sources', 'cat_income_other',      'Refunds',        'income_refunds',        '#9C998E', 'RotateCcw',  2),
  ('cat_income_reimbursements', 'income_sources', 'cat_income_other',      'Reimbursements', 'income_reimbursements', '#9C998E', 'Receipt',    3),
  ('cat_income_tax_refund',     'income_sources', 'cat_income_other',      'Tax Refund',     'income_tax_refund',     '#9C998E', 'FileText',   4);

-- ============================================================================
-- 8. SEED: DEFAULT EVENT TYPES
-- ============================================================================

INSERT INTO event_types (id, name, color) VALUES
  ('event-type-travel',           'Travel',           '#7B96C9'),
  ('event-type-holiday',          'Holiday',          '#6B8E54'),
  ('event-type-business',         'Business',         '#B89A4C'),
  ('event-type-education',        'Education',        '#5A7A3E'),
  ('event-type-medical',          'Medical',          '#B0552E'),
  ('event-type-special-occasion', 'Special Occasion', '#8E7CB3'),
  ('event-type-other',            'Other',            '#9C998E');
