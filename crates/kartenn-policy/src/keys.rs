//! The natural keys of the objects of the module as a human reads them, the `label` of
//! their summaries (spec 006, REQ-006-42, task 33f2; `ModuleHandler::keys`), read at the
//! dates of a read in one statement for them all. A key names its object alone, never
//! another one: the property an axis designates; the key of a value of an axis, without
//! its axis (an environment is a value of `policy.environment`); the key of a flow type, a
//! service, a pattern, a rule, a condition, a signal or a rule of inference; the role of an
//! exempt role.

use kartenn_core::period::Instant;
use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::pg::{store, time};

/// Each table of the module, in the schema `policy`, and the column of its key.
const KEYS: &[(&str, &str)] = &[
    ("policy_axis", "property_key"),
    ("policy_value", "value_key"),
    ("policy_flow_type", "key"),
    ("policy_service", "key"),
    ("policy_pattern", "key"),
    ("policy_rule", "key"),
    ("policy_condition", "key"),
    ("policy_signal", "key"),
    ("policy_exempt_role", "role_key"),
    ("policy_inference_rule", "key"),
];

/// The rows of `table` among `$3` valid at `$1` as asserted at `$2` (now when null): their
/// identifier and the key `column`.
fn dated(table: &str, column: &str) -> String {
    format!(
        "SELECT id, {column} FROM policy.{table}
          WHERE id = ANY($3) AND valid_period @> $1::timestamptz
            AND ($2::timestamptz IS NULL OR sys_start <= $2::timestamptz)
         UNION ALL
         SELECT id, {column} FROM policy.{table}_history
          WHERE $2::timestamptz IS NOT NULL AND id = ANY($3)
            AND valid_period @> $1::timestamptz
            AND sys_start <= $2::timestamptz AND sys_end > $2::timestamptz"
    )
}

/// The key of each of `objects` that exists at `valid_at` as asserted at `as_of` (now
/// when `None`); an object of another module, or absent then, is left out.
///
/// # Errors
///
/// Database errors.
pub(crate) async fn of(
    conn: &mut PgConnection,
    objects: &[Uuid],
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<(Uuid, String)>, StoreError> {
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    let sql = KEYS
        .iter()
        .map(|(table, column)| dated(table, column))
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(time(valid_at))
        .bind(as_of.map(time))
        .bind(objects)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))
}
