//! SQLite repository for tax lot rows.

use async_trait::async_trait;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use std::sync::Arc;

use crate::db::{get_connection, WriteHandle};
use crate::errors::StorageError;
use chrono::NaiveDate;
use log::warn;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use wealthfolio_core::errors::Result;
use wealthfolio_core::lots::{LotClosure, LotRecord, LotRepositoryTrait};

// ── Diesel model ──────────────────────────────────────────────────────────────

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::lots)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct LotRecordDB {
    id: String,
    account_id: String,
    asset_id: String,
    open_date: String,
    open_activity_id: Option<String>,
    original_quantity: String,
    remaining_quantity: String,
    cost_per_unit: String,
    original_cost_basis: String,
    remaining_cost_basis: String,
    fee_allocated: String,
    is_closed: i32,
    close_date: Option<String>,
    close_activity_id: Option<String>,
    created_at: String,
    updated_at: String,
    split_ratio: String,
}

impl From<LotRecordDB> for LotRecord {
    fn from(r: LotRecordDB) -> Self {
        LotRecord {
            id: r.id,
            account_id: r.account_id,
            asset_id: r.asset_id,
            open_date: r.open_date,
            open_activity_id: r.open_activity_id,
            original_quantity: r.original_quantity,
            remaining_quantity: r.remaining_quantity,
            cost_per_unit: r.cost_per_unit,
            original_cost_basis: r.original_cost_basis,
            remaining_cost_basis: r.remaining_cost_basis,
            fee_allocated: r.fee_allocated,
            split_ratio: r.split_ratio,
            is_closed: r.is_closed != 0,
            close_date: r.close_date,
            close_activity_id: r.close_activity_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

impl From<&LotRecord> for LotRecordDB {
    fn from(r: &LotRecord) -> Self {
        LotRecordDB {
            id: r.id.clone(),
            account_id: r.account_id.clone(),
            asset_id: r.asset_id.clone(),
            open_date: r.open_date.clone(),
            open_activity_id: r.open_activity_id.clone(),
            original_quantity: r.original_quantity.clone(),
            remaining_quantity: r.remaining_quantity.clone(),
            cost_per_unit: r.cost_per_unit.clone(),
            original_cost_basis: r.original_cost_basis.clone(),
            remaining_cost_basis: r.remaining_cost_basis.clone(),
            fee_allocated: r.fee_allocated.clone(),
            split_ratio: r.split_ratio.clone(),
            is_closed: r.is_closed as i32,
            close_date: r.close_date.clone(),
            close_activity_id: r.close_activity_id.clone(),
            created_at: r.created_at.clone(),
            updated_at: r.updated_at.clone(),
        }
    }
}

// ── Repository ────────────────────────────────────────────────────────────────

pub struct LotsRepository {
    pool: Arc<Pool<ConnectionManager<SqliteConnection>>>,
    writer: WriteHandle,
}

impl LotsRepository {
    pub fn new(pool: Arc<Pool<ConnectionManager<SqliteConnection>>>, writer: WriteHandle) -> Self {
        Self { pool, writer }
    }
}

#[async_trait]
impl LotRepositoryTrait for LotsRepository {
    async fn replace_lots_for_account(&self, account_id: &str, lots: &[LotRecord]) -> Result<()> {
        use crate::schema::lots::dsl;

        let account_id = account_id.to_string();
        let db_lots: Vec<LotRecordDB> = lots.iter().map(LotRecordDB::from).collect();

        self.writer
            .exec(move |conn| {
                diesel::delete(dsl::lots.filter(dsl::account_id.eq(&account_id)))
                    .execute(conn)
                    .map_err(StorageError::from)?;

                if !db_lots.is_empty() {
                    let normalized = filter_and_normalize_lots(conn, db_lots, &account_id)?;
                    if !normalized.is_empty() {
                        diesel::insert_into(dsl::lots)
                            .values(&normalized)
                            .execute(conn)
                            .map_err(StorageError::from)?;
                    }
                }

                Ok(())
            })
            .await
    }

    async fn get_open_lots_for_account(&self, account_id: &str) -> Result<Vec<LotRecord>> {
        use crate::schema::lots::dsl;

        let account_id = account_id.to_string();
        let mut conn = get_connection(&self.pool)?;
        let rows: Vec<LotRecordDB> = dsl::lots
            .filter(dsl::account_id.eq(&account_id))
            .filter(dsl::is_closed.eq(0))
            .load(&mut conn)
            .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(LotRecord::from).collect())
    }

    async fn get_all_open_lots(&self) -> Result<Vec<LotRecord>> {
        use crate::schema::lots::dsl;

        let mut conn = get_connection(&self.pool)?;
        let rows: Vec<LotRecordDB> = dsl::lots
            .filter(dsl::is_closed.eq(0))
            .load(&mut conn)
            .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(LotRecord::from).collect())
    }

    async fn get_lots_as_of_date(
        &self,
        account_ids: &[String],
        date: NaiveDate,
    ) -> Result<Vec<LotRecord>> {
        use crate::schema::lots::dsl;

        let date_str = date.format("%Y-%m-%d").to_string();
        let mut conn = get_connection(&self.pool)?;
        // A lot was active on `date` if it opened on or before that date AND
        // either (a) it is still open, or (b) it closed after that date.
        // The old query used .assume_not_null() on close_date which could drop
        // open lots (NULL > 'x' is NULL in SQL, not TRUE).
        let rows: Vec<LotRecordDB> = dsl::lots
            .filter(dsl::account_id.eq_any(account_ids))
            .filter(dsl::open_date.le(&date_str))
            .filter(
                dsl::is_closed.eq(0).or(dsl::close_date
                    .is_not_null()
                    .and(dsl::close_date.gt(&date_str))),
            )
            .load(&mut conn)
            .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(LotRecord::from).collect())
    }

    async fn get_all_lots_for_account(&self, account_id: &str) -> Result<Vec<LotRecord>> {
        use crate::schema::lots::dsl;

        let account_id = account_id.to_string();
        let mut conn = get_connection(&self.pool)?;
        let rows: Vec<LotRecordDB> = dsl::lots
            .filter(dsl::account_id.eq(&account_id))
            .load(&mut conn)
            .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(LotRecord::from).collect())
    }

    async fn get_lots_for_asset(&self, asset_id: &str) -> Result<Vec<LotRecord>> {
        use crate::schema::lots::dsl;

        let asset_id = asset_id.to_string();
        let mut conn = get_connection(&self.pool)?;
        let rows: Vec<LotRecordDB> = dsl::lots
            .filter(dsl::asset_id.eq(&asset_id))
            .order(dsl::open_date.asc())
            .load(&mut conn)
            .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(LotRecord::from).collect())
    }

    async fn get_all_lots(&self) -> Result<Vec<LotRecord>> {
        use crate::schema::lots::dsl;

        let mut conn = get_connection(&self.pool)?;
        let rows: Vec<LotRecordDB> = dsl::lots.load(&mut conn).map_err(StorageError::from)?;
        Ok(rows.into_iter().map(LotRecord::from).collect())
    }

    async fn sync_lots_for_account(
        &self,
        account_id: &str,
        open_lots: &[LotRecord],
        closures: &[LotClosure],
    ) -> Result<()> {
        use crate::schema::lots::dsl;

        let account_id = account_id.to_string();
        let db_lots: Vec<LotRecordDB> = open_lots.iter().map(LotRecordDB::from).collect();
        let closures: Vec<LotClosure> = closures.to_vec();

        self.writer
            .exec(move |conn| {
                // Normalize open-lot batch: drop rows whose asset_id no longer
                // exists (FK would reject them), and null out
                // open_activity_id when it points to an activity row that
                // doesn't exist (compiler-generated synthetic IDs like
                // `drip-1:buy`, or activities since deleted).
                let normalized_lots = filter_and_normalize_lots(conn, db_lots, &account_id)?;

                // Upsert open lots one at a time (SQLite Diesel doesn't support
                // batch ON CONFLICT)
                for lot in &normalized_lots {
                    diesel::insert_into(dsl::lots)
                        .values(lot)
                        .on_conflict(dsl::id)
                        .do_update()
                        .set((
                            dsl::original_quantity
                                .eq(diesel::upsert::excluded(dsl::original_quantity)),
                            dsl::remaining_quantity
                                .eq(diesel::upsert::excluded(dsl::remaining_quantity)),
                            dsl::cost_per_unit.eq(diesel::upsert::excluded(dsl::cost_per_unit)),
                            dsl::original_cost_basis
                                .eq(diesel::upsert::excluded(dsl::original_cost_basis)),
                            dsl::remaining_cost_basis
                                .eq(diesel::upsert::excluded(dsl::remaining_cost_basis)),
                            dsl::split_ratio.eq(diesel::upsert::excluded(dsl::split_ratio)),
                            dsl::is_closed.eq(diesel::upsert::excluded(dsl::is_closed)),
                            dsl::close_date.eq(diesel::upsert::excluded(dsl::close_date)),
                            dsl::close_activity_id
                                .eq(diesel::upsert::excluded(dsl::close_activity_id)),
                            dsl::updated_at.eq(diesel::upsert::excluded(dsl::updated_at)),
                        ))
                        .execute(conn)
                        .map_err(StorageError::from)?;
                }

                // Build closure records, then normalize them through the same
                // filter so missing-asset closures are dropped and synthetic
                // open/close_activity_ids are nulled out.
                let now = chrono::Utc::now()
                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                    .to_string();
                let closure_lots: Vec<LotRecordDB> = closures
                    .iter()
                    .map(|closure| LotRecordDB {
                        id: closure.lot_id.clone(),
                        account_id: closure.account_id.clone(),
                        asset_id: closure.asset_id.clone(),
                        open_date: closure.open_date.clone(),
                        open_activity_id: closure.open_activity_id.clone(),
                        original_quantity: closure.original_quantity.clone(),
                        remaining_quantity: "0".to_string(),
                        cost_per_unit: closure.cost_per_unit.clone(),
                        original_cost_basis: closure.original_cost_basis.clone(),
                        // Closure means the lot was fully consumed in one pass — no
                        // remaining basis. The disposed amount is captured in the
                        // separate disposal record (when lot_disposals lands).
                        remaining_cost_basis: "0".to_string(),
                        fee_allocated: closure.fee_allocated.clone(),
                        split_ratio: closure.split_ratio.clone(),
                        is_closed: 1,
                        close_date: Some(closure.close_date.clone()),
                        close_activity_id: closure.close_activity_id.clone(),
                        created_at: now.clone(),
                        updated_at: now.clone(),
                    })
                    .collect();
                let normalized_closures =
                    filter_and_normalize_lots(conn, closure_lots, &account_id)?;
                for closed_lot in &normalized_closures {
                    diesel::insert_into(dsl::lots)
                        .values(closed_lot)
                        .on_conflict(dsl::id)
                        .do_update()
                        .set((
                            dsl::is_closed.eq(1),
                            dsl::close_date.eq(diesel::upsert::excluded(dsl::close_date)),
                            dsl::close_activity_id
                                .eq(diesel::upsert::excluded(dsl::close_activity_id)),
                            dsl::remaining_quantity.eq("0"),
                            dsl::updated_at.eq(diesel::upsert::excluded(dsl::updated_at)),
                        ))
                        .execute(conn)
                        .map_err(StorageError::from)?;
                }

                // Delete orphaned lots for this account that weren't produced by
                // this recalculation. Orphans arise when activities are deleted
                // (FK SET NULL on open_activity_id) and a subsequent rebuild
                // creates new lots with new IDs, leaving the old ones behind.
                //
                // Safety: when both open_lots and closures are empty we do NOT
                // wipe the account's lots. An empty result here means the
                // calculation either failed mid-flight (e.g. an activity with a
                // bad asset_id errored) or it ran for an account with no
                // activities at all. In the latter case, callers that legitimately
                // want to clear an account use `replace_lots_for_account(id, &[])`
                // explicitly. In the former case, wiping would destroy correct
                // data — leaving the existing rows stale until the next successful
                // recalc is the safer outcome.
                let known_ids: Vec<&str> = normalized_lots
                    .iter()
                    .map(|l| l.id.as_str())
                    .chain(closures.iter().map(|c| c.lot_id.as_str()))
                    .collect();

                if known_ids.is_empty() {
                    // Check whether existing lots would have been destroyed by the
                    // old wipe-all behaviour and warn loudly so the underlying
                    // calculator failure can be investigated.
                    let existing: i64 = dsl::lots
                        .filter(dsl::account_id.eq(&account_id))
                        .count()
                        .get_result(conn)
                        .map_err(StorageError::from)?;
                    if existing > 0 {
                        log::warn!(
                            "sync_lots_for_account: skipping orphan cleanup for account {} \
                             because the recalculation produced no lots and no closures. \
                             {} existing lot row(s) preserved. This usually indicates a \
                             holdings_calculator error during recalc; check the logs for \
                             'Failed to process activity' or 'Invalid asset_id' messages. \
                             Use replace_lots_for_account explicitly if a wipe is intended.",
                            account_id,
                            existing
                        );
                    }
                } else {
                    // Delete orphaned OPEN lots not produced by this pass.
                    // Closed lots (is_closed=1) are mostly preserved because a
                    // subsequent recalc may not reproduce closures
                    // (take_disposed_lots drains the closure list on first
                    // call). The exception is closed lots whose
                    // open_activity_id is NULL — those can only have come from
                    // an activity that was deleted (FK SET NULL) and a
                    // subsequent rebuild that wrote a new lot under a new id.
                    // Without this sweep, every wipe+reimport accumulates a
                    // fresh duplicate of every single-pass-closed lot.
                    diesel::delete(
                        dsl::lots
                            .filter(dsl::account_id.eq(&account_id))
                            .filter(dsl::is_closed.eq(0))
                            .filter(diesel::dsl::not(dsl::id.eq_any(&known_ids))),
                    )
                    .execute(conn)
                    .map_err(StorageError::from)?;

                    diesel::delete(
                        dsl::lots
                            .filter(dsl::account_id.eq(&account_id))
                            .filter(dsl::is_closed.eq(1))
                            .filter(dsl::open_activity_id.is_null())
                            .filter(diesel::dsl::not(dsl::id.eq_any(&known_ids))),
                    )
                    .execute(conn)
                    .map_err(StorageError::from)?;
                }

                Ok(())
            })
            .await
    }

    async fn get_open_position_quantities(&self) -> Result<HashMap<String, Decimal>> {
        // Quantities are stored as TEXT for Decimal precision, so SUM() in
        // SQLite would force a lossy REAL cast. Fetch only the two columns
        // we need (avoiding the full LotRecordDB row) and sum as Decimal in
        // Rust to keep precision intact.
        use crate::schema::lots::dsl;

        let mut conn = get_connection(&self.pool)?;
        let rows: Vec<(String, String)> = dsl::lots
            .filter(dsl::is_closed.eq(0))
            .select((dsl::asset_id, dsl::remaining_quantity))
            .load(&mut conn)
            .map_err(StorageError::from)?;

        let mut quantities: HashMap<String, Decimal> = HashMap::new();
        for (asset_id, remaining) in rows {
            let qty = remaining.parse::<Decimal>().unwrap_or(Decimal::ZERO);
            *quantities.entry(asset_id).or_default() += qty;
        }
        Ok(quantities)
    }

    fn count_lots(&self) -> Result<i64> {
        use crate::schema::lots::dsl;
        use diesel::dsl::count_star;

        let mut conn = get_connection(&self.pool)?;
        let n: i64 = dsl::lots
            .select(count_star())
            .first(&mut conn)
            .map_err(StorageError::from)?;
        Ok(n)
    }
}

/// Returns the subset of `candidate_ids` that exist in `assets`. Used by lot
/// write paths to drop records whose asset has been deleted while the legacy
/// positions JSON still referenced it — without this filter, the lots table's
/// `asset_id` FK would reject the whole batch.
fn existing_asset_ids<'a, I>(
    conn: &mut SqliteConnection,
    candidate_ids: I,
) -> std::result::Result<HashSet<String>, StorageError>
where
    I: IntoIterator<Item = &'a str>,
{
    use crate::schema::assets::dsl as a;

    let unique: HashSet<String> = candidate_ids.into_iter().map(|s| s.to_string()).collect();
    if unique.is_empty() {
        return Ok(HashSet::new());
    }
    let needles: Vec<String> = unique.iter().cloned().collect();
    let found: Vec<String> = a::assets
        .select(a::id)
        .filter(a::id.eq_any(&needles))
        .load(conn)
        .map_err(StorageError::from)?;
    Ok(found.into_iter().collect())
}

/// Mirror of `existing_asset_ids` for the `activities` table. Used to validate
/// `lots.open_activity_id` / `lots.close_activity_id` before write — the
/// activity-compiler can emit synthetic IDs (e.g. `drip-1:buy`) that look like
/// real IDs but don't have rows in `activities`, and a user can delete an
/// activity at any time. Storing such an ID would violate the FK constraint
/// and abort the whole batch.
fn existing_activity_ids<'a, I>(
    conn: &mut SqliteConnection,
    candidate_ids: I,
) -> std::result::Result<HashSet<String>, StorageError>
where
    I: IntoIterator<Item = &'a str>,
{
    use crate::schema::activities::dsl as ac;

    let unique: HashSet<String> = candidate_ids.into_iter().map(|s| s.to_string()).collect();
    if unique.is_empty() {
        return Ok(HashSet::new());
    }
    let needles: Vec<String> = unique.iter().cloned().collect();
    let found: Vec<String> = ac::activities
        .select(ac::id)
        .filter(ac::id.eq_any(&needles))
        .load(conn)
        .map_err(StorageError::from)?;
    Ok(found.into_iter().collect())
}

/// Pre-write normalization for a batch of `LotRecordDB`:
///
///   * Drop rows whose `asset_id` is missing from `assets` (with a warn log).
///   * Null out `open_activity_id` / `close_activity_id` when they point at
///     activities that don't exist (with a warn log). The compiler emits
///     synthetic IDs like `<parent>:buy` for DRIP / staking / dividend-in-kind
///     legs; the lot record is still real and useful, just can't claim
///     provenance through the FK.
///
/// Returns the normalized batch ready for insert/upsert.
fn filter_and_normalize_lots(
    conn: &mut SqliteConnection,
    lots: Vec<LotRecordDB>,
    account_id: &str,
) -> std::result::Result<Vec<LotRecordDB>, StorageError> {
    if lots.is_empty() {
        return Ok(lots);
    }
    let valid_assets = existing_asset_ids(conn, lots.iter().map(|l| l.asset_id.as_str()))?;
    let valid_activities = existing_activity_ids(
        conn,
        lots.iter()
            .filter_map(|l| l.open_activity_id.as_deref())
            .chain(lots.iter().filter_map(|l| l.close_activity_id.as_deref())),
    )?;

    let mut out = Vec::with_capacity(lots.len());
    for mut lot in lots {
        if !valid_assets.contains(lot.asset_id.as_str()) {
            warn!(
                "Dropping lot {} for missing asset {} (account {})",
                lot.id, lot.asset_id, account_id
            );
            continue;
        }
        if let Some(ref aid) = lot.open_activity_id {
            if !valid_activities.contains(aid.as_str()) {
                warn!(
                    "Lot {} references unknown open_activity_id {} (account {}); persisting as NULL",
                    lot.id, aid, account_id
                );
                lot.open_activity_id = None;
            }
        }
        if let Some(ref aid) = lot.close_activity_id {
            if !valid_activities.contains(aid.as_str()) {
                warn!(
                    "Lot {} references unknown close_activity_id {} (account {}); persisting as NULL",
                    lot.id, aid, account_id
                );
                lot.close_activity_id = None;
            }
        }
        out.push(lot);
    }
    Ok(out)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_pool, run_migrations, write_actor::spawn_writer};
    use tempfile::tempdir;

    async fn setup() -> (
        LotsRepository,
        Arc<Pool<ConnectionManager<SqliteConnection>>>,
        tempfile::TempDir,
    ) {
        std::env::set_var("CONNECT_API_URL", "http://test.local");
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db").to_string_lossy().to_string();
        run_migrations(&db_path).unwrap();
        let pool = create_pool(&db_path).unwrap();
        let writer = spawn_writer((*pool).clone()).unwrap();
        let repo = LotsRepository::new(Arc::clone(&pool), writer);
        (repo, pool, dir)
    }

    fn insert_account(pool: &Arc<Pool<ConnectionManager<SqliteConnection>>>, id: &str) {
        let mut conn = get_connection(pool).unwrap();
        diesel::sql_query(format!(
            "INSERT INTO accounts (id, name, account_type, currency, is_default, is_active, \
             created_at, updated_at, tracking_mode, is_archived) \
             VALUES ('{}', 'Test', 'REGULAR', 'USD', 0, 1, datetime('now'), datetime('now'), 'TRANSACTIONS', 0)",
            id
        ))
        .execute(&mut conn)
        .unwrap();
    }

    fn insert_asset(pool: &Arc<Pool<ConnectionManager<SqliteConnection>>>, id: &str) {
        let mut conn = get_connection(pool).unwrap();
        diesel::sql_query(format!(
            "INSERT INTO assets (id, kind, is_active, quote_mode, quote_ccy, created_at, updated_at) \
             VALUES ('{}', 'INVESTMENT', 1, 'MARKET', 'USD', datetime('now'), datetime('now'))",
            id
        ))
        .execute(&mut conn)
        .unwrap();
    }

    fn make_lot_record(id: &str, account_id: &str, asset_id: &str, qty: &str) -> LotRecord {
        LotRecord {
            id: id.to_string(),
            account_id: account_id.to_string(),
            asset_id: asset_id.to_string(),
            open_date: "2024-01-15".to_string(),
            open_activity_id: None,
            original_quantity: qty.to_string(),
            remaining_quantity: qty.to_string(),
            cost_per_unit: "150".to_string(),
            original_cost_basis: "15000".to_string(),
            remaining_cost_basis: "15000".to_string(),
            fee_allocated: "0".to_string(),
            split_ratio: "1".to_string(),
            is_closed: false,
            close_date: None,
            close_activity_id: None,
            created_at: "2024-01-15T00:00:00.000Z".to_string(),
            updated_at: "2024-01-15T00:00:00.000Z".to_string(),
        }
    }

    #[tokio::test]
    async fn replace_inserts_and_replaces_lots_for_account() {
        let (repo, pool, _dir) = setup().await;
        insert_account(&pool, "acc1");
        insert_asset(&pool, "AAPL");
        insert_asset(&pool, "MSFT");

        // Insert 3 lots: 2 AAPL, 1 MSFT
        let initial = vec![
            make_lot_record("l1", "acc1", "AAPL", "50"),
            make_lot_record("l2", "acc1", "AAPL", "30"),
            make_lot_record("l3", "acc1", "MSFT", "100"),
        ];
        repo.replace_lots_for_account("acc1", &initial)
            .await
            .unwrap();
        assert_eq!(repo.count_lots().unwrap(), 3);

        // Replace with 2 different lots
        let replacement = vec![
            make_lot_record("l4", "acc1", "AAPL", "80"),
            make_lot_record("l5", "acc1", "MSFT", "60"),
        ];
        repo.replace_lots_for_account("acc1", &replacement)
            .await
            .unwrap();
        assert_eq!(repo.count_lots().unwrap(), 2);

        // Old IDs must be gone
        let mut conn = get_connection(&pool).unwrap();
        let ids: Vec<String> = crate::schema::lots::dsl::lots
            .select(crate::schema::lots::dsl::id)
            .load(&mut conn)
            .unwrap();
        assert!(!ids.contains(&"l1".to_string()));
        assert!(ids.contains(&"l4".to_string()));
        assert!(ids.contains(&"l5".to_string()));
    }

    #[tokio::test]
    async fn replace_only_affects_target_account() {
        let (repo, pool, _dir) = setup().await;
        insert_account(&pool, "acc1");
        insert_account(&pool, "acc2");
        insert_asset(&pool, "AAPL");
        insert_asset(&pool, "LQD");

        repo.replace_lots_for_account(
            "acc1",
            &[
                make_lot_record("a1", "acc1", "AAPL", "50"),
                make_lot_record("a2", "acc1", "AAPL", "30"),
            ],
        )
        .await
        .unwrap();

        repo.replace_lots_for_account(
            "acc2",
            &[
                make_lot_record("b1", "acc2", "LQD", "100"),
                make_lot_record("b2", "acc2", "LQD", "50"),
                make_lot_record("b3", "acc2", "LQD", "25"),
            ],
        )
        .await
        .unwrap();

        assert_eq!(repo.count_lots().unwrap(), 5);

        // Replace only acc1; acc2 must be untouched
        repo.replace_lots_for_account("acc1", &[make_lot_record("a3", "acc1", "AAPL", "80")])
            .await
            .unwrap();

        assert_eq!(repo.count_lots().unwrap(), 4); // 1 (acc1) + 3 (acc2)

        let mut conn = get_connection(&pool).unwrap();
        let acc2_count: i64 = crate::schema::lots::dsl::lots
            .filter(crate::schema::lots::dsl::account_id.eq("acc2"))
            .select(diesel::dsl::count_star())
            .first(&mut conn)
            .unwrap();
        assert_eq!(acc2_count, 3);
    }

    #[tokio::test]
    async fn replace_with_empty_slice_clears_account() {
        let (repo, pool, _dir) = setup().await;
        insert_account(&pool, "acc1");
        insert_asset(&pool, "AAPL");

        repo.replace_lots_for_account("acc1", &[make_lot_record("l1", "acc1", "AAPL", "50")])
            .await
            .unwrap();
        assert_eq!(repo.count_lots().unwrap(), 1);

        repo.replace_lots_for_account("acc1", &[]).await.unwrap();
        assert_eq!(repo.count_lots().unwrap(), 0);
    }

    /// Regression test for the FK-violation bug where compiler-generated
    /// synthetic activity IDs (e.g. `drip-1:buy` for DRIP/staking/dividend-
    /// in-kind BUY legs) would cause `replace_lots_for_account` /
    /// `sync_lots_for_account` to abort with a FK violation on
    /// `lots.open_activity_id`.
    ///
    /// The storage layer should detect that the referenced activity doesn't
    /// exist, null out the FK column with a warning, and still persist the
    /// rest of the lot data.
    #[tokio::test]
    async fn synthetic_open_activity_id_is_nulled_out() {
        let (repo, pool, _dir) = setup().await;
        insert_account(&pool, "acc1");
        insert_asset(&pool, "AAPL");

        // `drip-1:buy` is not in the activities table.
        let mut lot = make_lot_record("lot-drip", "acc1", "AAPL", "10");
        lot.open_activity_id = Some("drip-1:buy".to_string());

        // Should succeed (no FK violation), with the open_activity_id stored as NULL.
        repo.replace_lots_for_account("acc1", &[lot]).await.unwrap();

        let stored = repo.get_open_lots_for_account("acc1").await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id, "lot-drip");
        assert!(
            stored[0].open_activity_id.is_none(),
            "synthetic activity_id {:?} should be normalized to NULL",
            stored[0].open_activity_id
        );
    }

    /// Regression test for the data-corruption bug where a recalculation that
    /// failed (e.g. activity with empty asset_id) would produce empty results,
    /// and `sync_lots_for_account` would then wipe every lot for the account.
    ///
    /// `sync_lots_for_account` must NEVER delete an account's existing lots
    /// when it is given empty inputs. Callers that legitimately want to clear
    /// an account use `replace_lots_for_account(id, &[])` explicitly.
    #[tokio::test]
    async fn sync_with_empty_inputs_does_not_wipe_existing_lots() {
        let (repo, pool, _dir) = setup().await;
        insert_account(&pool, "acc1");
        insert_asset(&pool, "AAPL");
        insert_asset(&pool, "MSFT");

        // Seed the account with some lots.
        let initial = vec![
            make_lot_record("l1", "acc1", "AAPL", "50"),
            make_lot_record("l2", "acc1", "AAPL", "30"),
            make_lot_record("l3", "acc1", "MSFT", "100"),
        ];
        repo.sync_lots_for_account("acc1", &initial, &[])
            .await
            .unwrap();
        assert_eq!(repo.count_lots().unwrap(), 3);

        // Simulate a failed recalc that produced nothing for this account.
        // The function MUST NOT wipe the existing lots.
        repo.sync_lots_for_account("acc1", &[], &[]).await.unwrap();
        assert_eq!(
            repo.count_lots().unwrap(),
            3,
            "sync_lots_for_account with empty inputs must preserve existing lots \
             (failed recalcs would otherwise wipe correct data)"
        );

        // Verify the actual rows are unchanged.
        let lots = repo.get_all_lots_for_account("acc1").await.unwrap();
        assert_eq!(lots.len(), 3);
        let ids: Vec<&str> = lots.iter().map(|l| l.id.as_str()).collect();
        assert!(ids.contains(&"l1"));
        assert!(ids.contains(&"l2"));
        assert!(ids.contains(&"l3"));

        // Sanity check: the explicit wipe path still works for the legitimate
        // "account drained" case.
        repo.replace_lots_for_account("acc1", &[]).await.unwrap();
        assert_eq!(repo.count_lots().unwrap(), 0);
    }

    /// `sync_lots_for_account` should still remove orphaned lots when given a
    /// non-empty new set (the partial-replacement case for incremental recalc).
    #[tokio::test]
    async fn sync_with_partial_inputs_removes_orphans() {
        let (repo, pool, _dir) = setup().await;
        insert_account(&pool, "acc1");
        insert_asset(&pool, "AAPL");
        insert_asset(&pool, "MSFT");

        let initial = vec![
            make_lot_record("l1", "acc1", "AAPL", "50"),
            make_lot_record("l2", "acc1", "AAPL", "30"),
            make_lot_record("l3", "acc1", "MSFT", "100"),
        ];
        repo.sync_lots_for_account("acc1", &initial, &[])
            .await
            .unwrap();
        assert_eq!(repo.count_lots().unwrap(), 3);

        // Recalc keeps only l1; l2 and l3 should be orphaned and removed.
        let kept = vec![make_lot_record("l1", "acc1", "AAPL", "50")];
        repo.sync_lots_for_account("acc1", &kept, &[])
            .await
            .unwrap();
        assert_eq!(repo.count_lots().unwrap(), 1);

        let lots = repo.get_all_lots_for_account("acc1").await.unwrap();
        assert_eq!(lots.len(), 1);
        assert_eq!(lots[0].id, "l1");
    }
}
