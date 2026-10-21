//! The storage of the module in PostgreSQL, in the schema `flow` (design 011 « Données »).

use kartenn_core::period::{Instant, Period};
use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

/// The versioned table of the flows.
pub(crate) const FLOW_TABLE: &str = "flow.flow";

/// The versioned tables keyed by `flow_id`, ended with their flow.
const BY_FLOW: &[&str] = &[
    "flow.flow_waiver",
    "flow.flow_endpoint_address",
    "flow.flow_evaluation",
    "flow.flow_summary",
];

/// An instant as PostgreSQL reads it.
fn time(i: Instant) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).expect("an instant")
}

/// The bounds of `period` to bind; a `None` start is the instant of the transaction.
pub(crate) fn bounds(period: Option<Period>) -> (Option<OffsetDateTime>, Option<OffsetDateTime>) {
    period.map_or((None, None), |p| (Some(time(p.start())), p.end().map(time)))
}

fn store(e: &sqlx::Error) -> StoreError {
    StoreError(e.to_string())
}

/// The kind of endpoint a reference of the module points at (`address`,
/// `machines.interface`…), as the flow records it; `None` for another target.
fn endpoint_kind(to: &str) -> Option<&'static str> {
    Some(match to {
        "address" => "address",
        "prefix" => "prefix",
        "machines.interface" => "interface",
        "machines.machine" => "machine",
        _ => return None,
    })
}

/// The flows that name `target`, of the type `to`, as an endpoint and live from `at` on
/// (active or to come, REQ-011-3).
pub(crate) async fn referrers(
    conn: &mut PgConnection,
    to: &str,
    target: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Vec<Uuid>, StoreError> {
    let Some(kind) = endpoint_kind(to) else {
        return Ok(Vec::new());
    };
    sqlx::query_scalar(
        "SELECT DISTINCT id FROM flow.flow
          WHERE ((source_kind = $3 AND source_id = $1)
              OR (destination_kind = $3 AND destination_id = $1))
            AND valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)
          ORDER BY id",
    )
    .bind(target)
    .bind(at)
    .bind(kind)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// Ends from `at` on the rows of the flow `flow` in the tables keyed by `flow_id` (its
/// waiver, its endpoints and evaluations), as the core ends a versioned object: a version
/// that started before is shortened, a later one removed.
pub(crate) async fn close_dependents(
    conn: &mut PgConnection,
    flow: Uuid,
    at: Option<OffsetDateTime>,
    changeset: Uuid,
) -> Result<(), StoreError> {
    for table in BY_FLOW {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE {table}
                SET valid_period = tstzrange(lower(valid_period), coalesce($2::timestamptz, now())),
                    changeset_id = $3
              WHERE flow_id = $1 AND valid_period @> coalesce($2::timestamptz, now())
                AND lower(valid_period) < coalesce($2::timestamptz, now())"
        )))
        .bind(flow)
        .bind(at)
        .bind(changeset)
        .execute(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {table}
              WHERE flow_id = $1 AND lower(valid_period) >= coalesce($2::timestamptz, now())"
        )))
        .bind(flow)
        .bind(at)
        .execute(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    }
    Ok(())
}
