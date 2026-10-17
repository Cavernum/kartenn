//! Floors of the passes that hand each row once (migration 0027): a pass reads from its
//! floor up, so that its cost follows its backlog, not the size of the history.

use sqlx::PgConnection;

/// How often a pass reads from zero, as an SQL interval.
const SWEEP: &str = "1 hour";

/// The rows of a pass: their table (aliased `e`), their number, and the predicate of a row
/// not handled yet, which may use `$1` for the name of the consumer.
pub(crate) struct Rows {
    pub(crate) table: &'static str,
    pub(crate) seq: &'static str,
    pub(crate) unhandled: &'static str,
}

/// The floor of `pass` for this pass, raised when it safely can; `0` once an hour (the
/// sweep). The caller holds the lock of the pass, in the transaction `tx`.
///
/// # Errors
///
/// Database errors.
pub(crate) async fn floor(
    tx: &mut PgConnection,
    pass: &str,
    consumer: &str,
    rows: &Rows,
) -> sqlx::Result<i64> {
    let state = format!(
        "SELECT floor, pending_max,
                coalesce(pg_snapshot_xmin(pg_current_snapshot()) >= pending_xmax, false),
                swept_at < now() - interval '{SWEEP}'
         FROM pass_floor WHERE pass = $1"
    );
    let mut row: Option<(i64, Option<i64>, bool, bool)> =
        sqlx::query_as(sqlx::AssertSqlSafe(state.clone()))
            .bind(pass)
            .fetch_optional(&mut *tx)
            .await?;
    if row.is_none() {
        // The first pass: inserted only then, so that an idle pass writes nothing.
        sqlx::query("INSERT INTO pass_floor (pass) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(pass)
            .execute(&mut *tx)
            .await?;
        row = sqlx::query_as(sqlx::AssertSqlSafe(state))
            .bind(pass)
            .fetch_optional(&mut *tx)
            .await?;
    }
    let (mut floor, pending_max, settled, sweep) = row.unwrap_or((0, None, false, false));
    let Rows {
        table,
        seq,
        unhandled,
    } = rows;
    if let (true, Some(max)) = (settled, pending_max) {
        // Every number up to `max` is final: the first one not handled, or past them all.
        let first: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT min(e.{seq}) FROM {table} e
             WHERE $1::text IS NOT NULL AND e.{seq} >= $2 AND e.{seq} <= $3 AND {unhandled}"
        )))
        .bind(consumer)
        .bind(floor)
        .bind(max)
        .fetch_one(&mut *tx)
        .await?;
        floor = floor.max(first.unwrap_or(max + 1));
        sqlx::query(
            "UPDATE pass_floor SET floor = $2, pending_max = NULL, pending_xmax = NULL
             WHERE pass = $1",
        )
        .bind(pass)
        .bind(floor)
        .execute(&mut *tx)
        .await?;
    }
    if settled || pending_max.is_none() {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE pass_floor SET pending_max = (SELECT max({seq}) FROM {table}),
                                   pending_xmax = pg_snapshot_xmax(pg_current_snapshot())
             WHERE pass = $1"
        )))
        .bind(pass)
        .execute(&mut *tx)
        .await?;
    }
    if sweep {
        sqlx::query("UPDATE pass_floor SET swept_at = now() WHERE pass = $1")
            .bind(pass)
            .execute(&mut *tx)
            .await?;
        return Ok(0);
    }
    Ok(floor)
}
