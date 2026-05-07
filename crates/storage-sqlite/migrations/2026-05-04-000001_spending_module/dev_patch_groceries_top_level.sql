-- ============================================================================
-- DEV DB PATCH — restructure spending_categories so that Groceries is its own
-- top-level category (matches Monarch / YNAB / Lunch Money / EveryDollar SOTA).
--
-- Changes:
--   1. Removes `cat_food_groceries` (was a child of Food & Dining)
--   2. Adds new top-level `cat_groceries` (key='groceries', sort_order=2)
--   3. Bumps existing top-levels 3-12 → 4-13 to make room
--   4. Re-numbers Food & Dining subcategories 1-4 (down from 5; groceries left)
--
-- Destructive — clears categorization_rules + activity_taxonomy_assignments
-- referencing the dropped category. Acceptable for dev workflow; users will
-- re-import preset rules afterward (their `categoryKey` now reads "groceries").
--
-- Run via:  sqlite3 path/to/wealthfolio.db < dev_patch_groceries_top_level.sql
-- ============================================================================

BEGIN TRANSACTION;

-- 1. Clear any rules pointing at the old cat_food_groceries (their FK is
--    SET NULL but we want a clean re-import, not silently broken rules).
DELETE FROM categorization_rules
  WHERE taxonomy_id = 'spending_categories'
    AND category_id = 'cat_food_groceries';

-- 2. Clear assignments referencing the old grocery category. Most users have
--    already cleared activity_taxonomy_assignments via the earlier dev patch;
--    this is a safety net.
DELETE FROM activity_taxonomy_assignments
  WHERE taxonomy_id = 'spending_categories'
    AND category_id = 'cat_food_groceries';

-- 3. Drop the old subcategory.
DELETE FROM taxonomy_categories
  WHERE id = 'cat_food_groceries';

-- 4. Bump existing top-level sort_orders 3..12 → 4..13 to make room at 2.
UPDATE taxonomy_categories SET sort_order = 13 WHERE id = 'cat_fees';
UPDATE taxonomy_categories SET sort_order = 12 WHERE id = 'cat_gifts';
UPDATE taxonomy_categories SET sort_order = 11 WHERE id = 'cat_travel';
UPDATE taxonomy_categories SET sort_order = 10 WHERE id = 'cat_education';
UPDATE taxonomy_categories SET sort_order =  9 WHERE id = 'cat_personal';
UPDATE taxonomy_categories SET sort_order =  8 WHERE id = 'cat_bills';
UPDATE taxonomy_categories SET sort_order =  7 WHERE id = 'cat_health';
UPDATE taxonomy_categories SET sort_order =  6 WHERE id = 'cat_entertainment';
UPDATE taxonomy_categories SET sort_order =  5 WHERE id = 'cat_shopping';
UPDATE taxonomy_categories SET sort_order =  4 WHERE id = 'cat_transport';
UPDATE taxonomy_categories SET sort_order =  3 WHERE id = 'cat_food';
-- cat_housing stays at 1, cat_other_expense stays at 99.

-- 5. Insert new top-level Groceries at sort_order 2.
INSERT INTO taxonomy_categories
  (id, taxonomy_id, parent_id, name, key, color, icon, sort_order)
VALUES
  ('cat_groceries', 'spending_categories', NULL, 'Groceries', 'groceries',
   '#7CB342', 'ShoppingCart', 2);

-- 6. Re-number Food & Dining subcategories now that Groceries is gone.
UPDATE taxonomy_categories SET sort_order = 1 WHERE id = 'cat_food_restaurants';
UPDATE taxonomy_categories SET sort_order = 2 WHERE id = 'cat_food_coffee';
UPDATE taxonomy_categories SET sort_order = 3 WHERE id = 'cat_food_delivery';
UPDATE taxonomy_categories SET sort_order = 4 WHERE id = 'cat_food_alcohol';

COMMIT;
