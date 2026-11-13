//! The dated relations the reads of the module select from (tasks 019-8a, 8b; spec 006,
//! REQ-006-7), and the criteria of a list in its journal (REQ-012-18), as the 009 writes
//! them (`kartenn_machines::rest`).

use kartenn_rest::kit::{Moment, Page};
use serde_json::{Value, json};

use crate::observations::rfc3339;

/// The rows of `table` (a versioned table of the module) valid at `$1`, as asserted at
/// `$2` (now when null), each of its `columns`: the relation of a dated read.
pub(super) fn dated(table: &str, columns: &str) -> String {
    format!(
        "(SELECT {columns} FROM {table}
           WHERE valid_period @> $1::timestamptz
             AND ($2::timestamptz IS NULL OR sys_start <= $2::timestamptz)
          UNION ALL
          SELECT {columns} FROM {table}_history
           WHERE $2::timestamptz IS NOT NULL AND valid_period @> $1::timestamptz
             AND sys_start <= $2::timestamptz AND sys_end > $2::timestamptz)"
    )
}

/// The relation of a list: [`dated`], or for the current assertions (`$2` null) the
/// current table alone, whose index then serves the order of a page: the union with the
/// history, even emptied by `$2`, makes the planner sort every row kept (spec 018, task
/// 018-16).
pub(super) fn listed(table: &str, columns: &str, current: bool) -> String {
    if current {
        format!(
            "(SELECT {columns} FROM {table}
               WHERE valid_period @> $1::timestamptz AND $2::timestamptz IS NULL)"
        )
    } else {
        dated(table, columns)
    }
}

/// [`dated`] restricted by `condition` in each branch: correlated, it keeps the read on
/// the indexes of `condition` even in a generic plan, where a join on the whole relation
/// would scan every row valid at `$1`.
pub(super) fn dated_where(table: &str, columns: &str, condition: &str) -> String {
    format!(
        "(SELECT {columns} FROM {table}
           WHERE {condition} AND valid_period @> $1::timestamptz
             AND ($2::timestamptz IS NULL OR sys_start <= $2::timestamptz)
          UNION ALL
          SELECT {columns} FROM {table}_history
           WHERE {condition} AND $2::timestamptz IS NOT NULL
             AND valid_period @> $1::timestamptz
             AND sys_start <= $2::timestamptz AND sys_end > $2::timestamptz)"
    )
}

/// What a list of `collection` journals of its question: its filters and its dates.
pub(super) fn criteria(collection: &str, page: &Page, moment: &Moment) -> Value {
    json!({
        "collection": collection,
        "filters": page.filters,
        "valid_at": rfc3339(moment.valid_at),
        "as_of": rfc3339(moment.as_of()),
    })
}
