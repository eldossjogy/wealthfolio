-- Repaints the spending-module seed colors (budget groups, spending taxonomy,
-- categories, subcategories, income sources, event types) to match the new
-- design palette + Flexoki 500/600 extras.
--
-- Safe to re-run. Targets rows by their seed id only — user-created groups,
-- categories and event types are untouched, and user-edited colors on seed
-- rows ARE overwritten (intended: this is the whole point of the script).
--
-- Usage (macOS, default install):
--   sqlite3 "$HOME/Library/Application Support/com.teymz.wealthfolio/app.db" \
--     < scripts/repaint-spending-colors.sql
--
-- ALWAYS close Wealthfolio first (SQLite locks) and back up app.db before running.

BEGIN TRANSACTION;

-- Budget groups
UPDATE budget_groups SET color = '#4F6B92' WHERE id = 'budget_group_needs';
UPDATE budget_groups SET color = '#8E7CB3' WHERE id = 'budget_group_wants';
UPDATE budget_groups SET color = '#6B8E54' WHERE id = 'budget_group_savings';
UPDATE budget_groups SET color = '#A35742' WHERE id = 'budget_group_giving';
UPDATE budget_groups SET color = '#B89A4C' WHERE id = 'budget_group_personal';
UPDATE budget_groups SET color = '#9C998E' WHERE id = 'budget_group_other';

-- Taxonomy umbrellas
UPDATE taxonomies SET color = '#B0552E' WHERE id = 'spending_categories';
UPDATE taxonomies SET color = '#5A7A3E' WHERE id = 'income_sources';

-- Spending top-level
UPDATE taxonomy_categories SET color = '#A35742' WHERE id = 'cat_housing';
UPDATE taxonomy_categories SET color = '#5A7A3E' WHERE id = 'cat_groceries';
UPDATE taxonomy_categories SET color = '#B89A4C' WHERE id = 'cat_food';
UPDATE taxonomy_categories SET color = '#7B96C9' WHERE id = 'cat_transport';
UPDATE taxonomy_categories SET color = '#8E7CB3' WHERE id = 'cat_shopping';
UPDATE taxonomy_categories SET color = '#B0552E' WHERE id = 'cat_entertainment';
UPDATE taxonomy_categories SET color = '#6B8E54' WHERE id = 'cat_health';
UPDATE taxonomy_categories SET color = '#4F6B92' WHERE id = 'cat_bills';
UPDATE taxonomy_categories SET color = '#B74583' WHERE id = 'cat_personal';
UPDATE taxonomy_categories SET color = '#24837B' WHERE id = 'cat_education';
UPDATE taxonomy_categories SET color = '#3171B2' WHERE id = 'cat_travel';
UPDATE taxonomy_categories SET color = '#AF3029' WHERE id = 'cat_gifts';
UPDATE taxonomy_categories SET color = '#9C998E' WHERE id = 'cat_fees';
UPDATE taxonomy_categories SET color = '#B6B2A4' WHERE id = 'cat_other_expense';

-- Spending subcategories (inherit parent color)
UPDATE taxonomy_categories SET color = '#A35742' WHERE id LIKE 'cat_housing_%';
UPDATE taxonomy_categories SET color = '#B89A4C' WHERE id LIKE 'cat_food_%';
UPDATE taxonomy_categories SET color = '#7B96C9' WHERE id LIKE 'cat_transport_%';
UPDATE taxonomy_categories SET color = '#8E7CB3' WHERE id LIKE 'cat_shopping_%';
UPDATE taxonomy_categories SET color = '#B0552E' WHERE id LIKE 'cat_entertainment_%';
UPDATE taxonomy_categories SET color = '#6B8E54' WHERE id LIKE 'cat_health_%';
UPDATE taxonomy_categories SET color = '#4F6B92' WHERE id LIKE 'cat_bills_%';
UPDATE taxonomy_categories SET color = '#9C998E' WHERE id LIKE 'cat_fees_%';

-- Income sources (top-level)
UPDATE taxonomy_categories SET color = '#5A7A3E' WHERE id = 'cat_income_employment';
UPDATE taxonomy_categories SET color = '#6B8E54' WHERE id = 'cat_income_selfemploy';
UPDATE taxonomy_categories SET color = '#B89A4C' WHERE id = 'cat_income_investment';
UPDATE taxonomy_categories SET color = '#9C998E' WHERE id = 'cat_income_other';

-- Income subcategories (inherit parent color via known ids)
UPDATE taxonomy_categories SET color = '#5A7A3E'
  WHERE id IN ('cat_income_salary', 'cat_income_bonus', 'cat_income_commission');
UPDATE taxonomy_categories SET color = '#6B8E54'
  WHERE id IN ('cat_income_freelance', 'cat_income_business');
UPDATE taxonomy_categories SET color = '#B89A4C'
  WHERE id IN ('cat_income_dividends', 'cat_income_interest',
               'cat_income_rental', 'cat_income_capital_gains');
UPDATE taxonomy_categories SET color = '#9C998E'
  WHERE id IN ('cat_income_gifts', 'cat_income_refunds',
               'cat_income_reimbursements', 'cat_income_tax_refund');

-- Event types
UPDATE event_types SET color = '#7B96C9' WHERE id = 'event-type-travel';
UPDATE event_types SET color = '#6B8E54' WHERE id = 'event-type-holiday';
UPDATE event_types SET color = '#B89A4C' WHERE id = 'event-type-business';
UPDATE event_types SET color = '#5A7A3E' WHERE id = 'event-type-education';
UPDATE event_types SET color = '#B0552E' WHERE id = 'event-type-medical';
UPDATE event_types SET color = '#8E7CB3' WHERE id = 'event-type-special-occasion';
UPDATE event_types SET color = '#9C998E' WHERE id = 'event-type-other';

COMMIT;

-- Sanity check (printed to stdout when run with sqlite3)
SELECT 'budget_groups   ' AS table_name, COUNT(*) AS updated FROM budget_groups
  WHERE color IN ('#4F6B92','#8E7CB3','#6B8E54','#A35742','#B89A4C','#9C998E')
UNION ALL
SELECT 'event_types     ', COUNT(*) FROM event_types
  WHERE color IN ('#7B96C9','#6B8E54','#B89A4C','#5A7A3E','#B0552E','#8E7CB3','#9C998E');
