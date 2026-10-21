//! Dated reads of the objects of the policy (feature `pg`; REQ-010-16, 26; design 010
//! « Instantané de politique »): the objects valid at `valid_at` as asserted at `as_of`,
//! one query per table, gathered into the [`Parts`] of a [`PolicySnapshot`].
//!
//! [`PolicySnapshot`]: crate::snapshot::PolicySnapshot

use std::collections::{BTreeMap, BTreeSet};

use kartenn_service::port::StoreError;
use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::codec;
use crate::model::{
    Axis, AxisKey, AxisValue, Carrier, Condition, Effect, FlowSelector, FlowType, Key, Label,
    Pattern, Rule, Service, Signal, ValueAttributes,
};
use crate::pg::store;
use crate::snapshot::Parts;

/// The tables of the snapshot, in the order they are read.
pub(crate) const TABLES: &[&str] = &[
    "policy_axis",
    "policy_value",
    "policy_flow_type",
    "policy_service",
    "policy_pattern",
    "policy_rule",
    "policy_condition",
    "policy_signal",
];

/// The rows of `policy.<table>` valid at `$1` as asserted at `$2` (now when null), as JSON:
/// the current table, and its history for a past assertion (ADR 0017).
fn dated(table: &str) -> String {
    format!(
        "SELECT to_jsonb(t) FROM policy.{table} t
          WHERE valid_period @> $1::timestamptz
            AND ($2::timestamptz IS NULL OR sys_start <= $2::timestamptz)
         UNION ALL
         SELECT to_jsonb(h) - 'sys_end' FROM policy.{table}_history h
          WHERE $2::timestamptz IS NOT NULL AND valid_period @> $1::timestamptz
            AND sys_start <= $2::timestamptz AND sys_end > $2::timestamptz"
    )
}

async fn rows(
    conn: &mut PgConnection,
    table: &str,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> Result<Vec<Value>, StoreError> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(dated(table)))
        .bind(valid_at)
        .bind(as_of)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))
}

/// A row that does not read back as the object it was checked to be when written.
fn corrupt(table: &str, row: &Value) -> StoreError {
    StoreError(format!(
        "policy.{table}: the row {} does not read back",
        row.get("id").map_or_else(String::new, Value::to_string)
    ))
}

fn text<'a>(row: &'a Value, column: &str) -> &'a str {
    row.get(column).and_then(Value::as_str).unwrap_or_default()
}

fn uuids(row: &Value, column: &str) -> Vec<Uuid> {
    row.get(column)
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().and_then(|s| s.parse().ok()))
                .collect()
        })
        .unwrap_or_default()
}

fn id(row: &Value, column: &str) -> Option<Uuid> {
    row.get(column)
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok())
}

/// The keys of the cited identifiers: a citation of an object that does not live at the
/// date is dropped, as the rules of the module refuse it when written (REQ-008-10).
fn keys(ids: &[Uuid], of: &BTreeMap<Uuid, Key>) -> BTreeSet<Key> {
    ids.iter().filter_map(|id| of.get(id).cloned()).collect()
}

fn key_label(table: &str, row: &Value) -> Result<(Key, Label), StoreError> {
    match (Key::new(text(row, "key")), Label::new(text(row, "label"))) {
        (Ok(key), Ok(label)) => Ok((key, label)),
        _ => Err(corrupt(table, row)),
    }
}

/// The identifiers of `rows` and their keys.
fn ids_to_keys(rows: &[Value]) -> BTreeMap<Uuid, Key> {
    rows.iter()
        .filter_map(|row| Some((id(row, "id")?, Key::new(text(row, "key")).ok()?)))
        .collect()
}

/// The objects of the policy valid at `valid_at` as asserted at `as_of` (now when `None`),
/// with `revision` as their provenance (REQ-010-16, 26).
///
/// # Errors
/// A failure of the database, or a stored row that does not read back.
#[allow(clippy::too_many_lines)] // one conversion per table, read in one pass
pub async fn parts(
    conn: &mut PgConnection,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    revision: u64,
) -> Result<Parts, StoreError> {
    let mut read = BTreeMap::new();
    for table in TABLES {
        read.insert(*table, rows(conn, table, valid_at, as_of).await?);
    }
    let axes = read["policy_axis"]
        .iter()
        .map(|row| {
            let carrier = match text(row, "carrier") {
                "tree" => Carrier::Tree,
                "machine" => Carrier::Machine,
                _ => return Err(corrupt("policy_axis", row)),
            };
            Ok(Axis {
                key: AxisKey::parse(text(row, "property_key"))
                    .map_err(|_| corrupt("policy_axis", row))?,
                carrier,
                inference: row.get("inference").and_then(Value::as_bool) == Some(true),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let values = read["policy_value"]
        .iter()
        .map(|row| {
            Ok((
                AxisKey::parse(text(row, "property_key"))
                    .map_err(|_| corrupt("policy_value", row))?,
                AxisValue::new(text(row, "value_key")),
                ValueAttributes {
                    transverse: row.get("transverse").and_then(Value::as_bool) == Some(true),
                    rank: row
                        .get("rank")
                        .and_then(Value::as_i64)
                        .and_then(|r| i32::try_from(r).ok()),
                },
            ))
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let flow_type_keys = ids_to_keys(&read["policy_flow_type"]);
    let flow_types = read["policy_flow_type"]
        .iter()
        .map(|row| {
            let (key, label) = key_label("policy_flow_type", row)?;
            Ok(FlowType { key, label })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let service_keys = ids_to_keys(&read["policy_service"]);
    let services = read["policy_service"]
        .iter()
        .filter_map(|row| {
            // A service whose flow type does not live at the date is dropped with it.
            let flow_type = id(row, "flow_type_id").and_then(|t| flow_type_keys.get(&t))?;
            Some((row, flow_type.clone()))
        })
        .map(|(row, flow_type)| {
            let (key, label) = key_label("policy_service", row)?;
            let (entries, _) =
                codec::entries(&row["entries"]).map_err(|_| corrupt("policy_service", row))?;
            Service::new(key, label, &[flow_type], entries)
                .map_err(|_| corrupt("policy_service", row))
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let pattern_keys = ids_to_keys(&read["policy_pattern"]);
    let patterns = read["policy_pattern"]
        .iter()
        .map(|row| {
            let (key, label) = key_label("policy_pattern", row)?;
            Ok(Pattern {
                key,
                label,
                texts: codec::stored_texts(&row["texts"]),
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let rule_keys = ids_to_keys(&read["policy_rule"]);
    let rules = read["policy_rule"]
        .iter()
        .map(|row| {
            let (key, label) = key_label("policy_rule", row)?;
            let selector = |side: &str| {
                codec::selector(&row[side])
                    .map(|(s, _)| s)
                    .map_err(|_| corrupt("policy_rule", row))
            };
            Ok(Rule {
                key,
                label,
                source: selector("source")?,
                destination: selector("destination")?,
                same: codec::axes(&row["same"]).map_err(|_| corrupt("policy_rule", row))?,
                flows: FlowSelector {
                    flow_types: keys(&uuids(row, "flow_type_ids"), &flow_type_keys),
                    services: keys(&uuids(row, "service_ids"), &service_keys),
                },
                effect: if text(row, "effect") == "deny" {
                    Effect::Deny
                } else {
                    Effect::Allow
                },
                required_patterns: keys(&uuids(row, "required_pattern_ids"), &pattern_keys),
                pattern: id(row, "pattern_id").and_then(|p| pattern_keys.get(&p).cloned()),
                texts: codec::stored_texts(&row["texts"]),
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let conditions = read["policy_condition"]
        .iter()
        .map(|row| {
            let key = Key::new(text(row, "key")).map_err(|_| corrupt("policy_condition", row))?;
            let kind = codec::condition(text(row, "kind"), &row["params"])
                .map_err(|_| corrupt("policy_condition", row))?;
            Ok(Condition {
                key,
                kind,
                rules: keys(&uuids(row, "rule_ids"), &rule_keys),
                flow_types: keys(&uuids(row, "flow_type_ids"), &flow_type_keys),
                texts: codec::stored_texts(&row["texts"]),
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let signals = read["policy_signal"]
        .iter()
        .map(|row| {
            let key = Key::new(text(row, "key")).map_err(|_| corrupt("policy_signal", row))?;
            let kind = codec::signal(text(row, "kind"), &row["params"])
                .map_err(|_| corrupt("policy_signal", row))?;
            Ok(Signal {
                key,
                kind,
                weight: row
                    .get("weight")
                    .and_then(Value::as_i64)
                    .and_then(|w| i32::try_from(w).ok())
                    .unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    Ok(Parts {
        revision,
        default_locale: codec::default_locale(),
        axes,
        values,
        flow_types,
        services,
        rules,
        patterns,
        conditions,
        signals,
    })
}

/// The revision of the policy now (REQ-010-26, S17): one row.
///
/// # Errors
/// A failure of the database.
pub async fn revision(conn: &mut PgConnection) -> Result<u64, StoreError> {
    let revision: i64 = sqlx::query_scalar("SELECT policy FROM policy.policy_revision")
        .fetch_one(conn)
        .await
        .map_err(|e| store(&e))?;
    Ok(u64::try_from(revision).unwrap_or_default())
}

/// The span of valid time around `valid_at` over which the objects asserted at `as_of` do
/// not change: the last start or end of a version at or before `valid_at`, and the first
/// after it (`None`: unbounded). A snapshot read at `valid_at` holds over this span.
///
/// # Errors
/// A failure of the database.
pub async fn stable_span(
    conn: &mut PgConnection,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> Result<(Option<OffsetDateTime>, Option<OffsetDateTime>), StoreError> {
    let asserted: Vec<String> = TABLES
        .iter()
        .map(|table| {
            format!(
                "SELECT valid_period FROM policy.{table}
                  WHERE $2::timestamptz IS NULL OR sys_start <= $2::timestamptz
                 UNION ALL
                 SELECT valid_period FROM policy.{table}_history
                  WHERE $2::timestamptz IS NOT NULL
                    AND sys_start <= $2::timestamptz AND sys_end > $2::timestamptz"
            )
        })
        .collect();
    let sql = format!(
        "WITH periods AS ({}),
              bounds AS (SELECT lower(valid_period) AS b FROM periods
                         UNION SELECT upper(valid_period) FROM periods)
         SELECT (SELECT max(b) FROM bounds WHERE b <= $1::timestamptz),
                (SELECT min(b) FROM bounds WHERE b > $1::timestamptz)",
        asserted.join(" UNION ALL ")
    );
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(valid_at)
        .bind(as_of)
        .fetch_one(conn)
        .await
        .map_err(|e| store(&e))
}
