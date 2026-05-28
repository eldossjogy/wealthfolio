DROP TABLE IF EXISTS account_accounting_settings;
DROP TABLE IF EXISTS lot_disposals;

ALTER TABLE daily_account_valuation DROP COLUMN external_flow_source;

ALTER TABLE lots DROP COLUMN cost_basis_method;
ALTER TABLE lots DROP COLUMN fx_rate_to_base;
ALTER TABLE lots DROP COLUMN base_currency;
ALTER TABLE lots DROP COLUMN currency;
ALTER TABLE lots DROP COLUMN fee_allocated_base;
ALTER TABLE lots DROP COLUMN remaining_cost_basis_base;
ALTER TABLE lots DROP COLUMN original_cost_basis_base;
