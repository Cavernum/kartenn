//! The policy read from the storage (feature `pg`; design 010 « Instantané de politique »):
//! one query per table at a valid instant, as the transaction sees it after its writes or
//! as it stood before them (the versions the changeset superseded are in the history,
//! ended at the start of the transaction, ADR 0017).
//!
//! Conditions and signals are not read: neither a verdict of principle nor the
//! differential depends on them (REQ-010-18, 36, 37).

use std::collections::BTreeMap;

use kartenn_core::properties::PropertyKey;
use kartenn_service::port::StoreError;
use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::codec;
use crate::model::{
    Axis, AxisValue, Carrier, Effect, FlowSelector, FlowType, Key, Label, Pattern, Rule, Service,
    ValueAttributes,
};
use crate::pg::store;
use crate::snapshot::Parts;

/// Which state of the transaction a snapshot reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    /// As it stood before the changeset's writes.
    Before,
    /// With the changeset's writes.
    After,
}

/// The rows of `table` valid at `at` in `state`, as JSON objects with their `id`.
async fn rows(
    conn: &mut PgConnection,
    table: &str,
    at: OffsetDateTime,
    state: State,
) -> Result<Vec<Value>, StoreError> {
    let sql = match state {
        State::After => format!("SELECT to_jsonb(t) FROM {table} t WHERE valid_period @> $1"),
        // Before: the versions older than the transaction, and those it superseded (moved
        // to the history at its start).
        State::Before => format!(
            "SELECT to_jsonb(t) FROM {table} t
              WHERE valid_period @> $1 AND sys_start < now()
             UNION ALL
             SELECT to_jsonb(h) - 'sys_end' FROM {table}_history h
              WHERE valid_period @> $1 AND sys_start < now() AND sys_end >= now()"
        ),
    };
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(at)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))
}

fn text(row: &Value, column: &str) -> String {
    row.get(column)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn id(row: &Value, column: &str) -> Option<Uuid> {
    row.get(column)
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok())
}

/// The keys of the objects whose identifiers `column` (an array) cites.
fn cited(row: &Value, column: &str, keys: &BTreeMap<Uuid, Key>) -> std::collections::BTreeSet<Key> {
    row.get(column)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter_map(|s| s.parse::<Uuid>().ok())
                .filter_map(|i| keys.get(&i).cloned())
                .collect()
        })
        .unwrap_or_default()
}

/// Key and label of a row; `None` for a row that does not decode (never written so).
fn named(row: &Value) -> Option<(Key, Label)> {
    Some((
        Key::new(&text(row, "key")).ok()?,
        Label::new(&text(row, "label")).ok()?,
    ))
}

/// An axis from its row.
fn axis(r: &Value) -> Option<Axis> {
    Some(Axis {
        key: PropertyKey::parse(&text(r, "property_key")).ok()?,
        carrier: if text(r, "carrier") == "machine" {
            Carrier::Machine
        } else {
            Carrier::Tree
        },
        inference: r.get("inference").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// The attributes of a value from its row.
fn value(r: &Value) -> Option<(PropertyKey, AxisValue, ValueAttributes)> {
    Some((
        PropertyKey::parse(&text(r, "property_key")).ok()?,
        AxisValue::new(text(r, "value_key")),
        ValueAttributes {
            transverse: r
                .get("transverse")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            rank: r
                .get("rank")
                .and_then(Value::as_i64)
                .and_then(|v| i32::try_from(v).ok()),
        },
    ))
}

/// The keys of the cited objects of a rule, by identifier.
struct Cited {
    flow_types: BTreeMap<Uuid, Key>,
    services: BTreeMap<Uuid, Key>,
    patterns: BTreeMap<Uuid, Key>,
}

/// A rule from its row, its citations resolved to keys.
fn rule(r: &Value, keys: &Cited) -> Option<Rule> {
    let (key, label) = named(r)?;
    let selector = |side: &str| {
        r.get(side)
            .and_then(|s| codec::selector(s).ok())
            .map(|(s, _)| s)
            .unwrap_or_default()
    };
    Some(Rule {
        key,
        label,
        source: selector("source"),
        destination: selector("destination"),
        same: r
            .get("same")
            .and_then(|s| codec::axes(s).ok())
            .unwrap_or_default(),
        flows: FlowSelector {
            flow_types: cited(r, "flow_type_ids", &keys.flow_types),
            services: cited(r, "service_ids", &keys.services),
        },
        effect: if text(r, "effect") == "deny" {
            Effect::Deny
        } else {
            Effect::Allow
        },
        required_patterns: cited(r, "required_pattern_ids", &keys.patterns),
        pattern: id(r, "pattern_id").and_then(|i| keys.patterns.get(&i).cloned()),
        texts: codec::stored_texts(&r["texts"]),
    })
}

/// The services valid at `at` in `state`, and their keys by identifier.
async fn services(
    conn: &mut PgConnection,
    at: OffsetDateTime,
    state: State,
    flow_types: &BTreeMap<Uuid, Key>,
) -> Result<(Vec<Service>, BTreeMap<Uuid, Key>), StoreError> {
    let mut keys = BTreeMap::new();
    let mut services = Vec::new();
    for r in rows(conn, "policy.policy_service", at, state).await? {
        let Some((key, label)) = named(&r) else {
            continue;
        };
        let flow_type: Vec<Key> = id(&r, "flow_type_id")
            .and_then(|i| flow_types.get(&i).cloned())
            .into_iter()
            .collect();
        let Ok((entries, _)) = codec::entries(&r["entries"]) else {
            continue;
        };
        if let (Some(i), Ok(service)) =
            (id(&r, "id"), Service::new(key, label, &flow_type, entries))
        {
            keys.insert(i, service.key.clone());
            services.push(service);
        }
    }
    Ok((services, keys))
}

/// The objects of the policy valid at `at`, in `state`.
pub(crate) async fn parts(
    conn: &mut PgConnection,
    at: OffsetDateTime,
    state: State,
) -> Result<Parts, StoreError> {
    let revision: i64 = sqlx::query_scalar("SELECT policy FROM policy.policy_revision")
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| store(&e))?
        .unwrap_or_default();
    let axes = rows(conn, "policy.policy_axis", at, state)
        .await?
        .iter()
        .filter_map(axis)
        .collect();
    let values = rows(conn, "policy.policy_value", at, state)
        .await?
        .iter()
        .filter_map(value)
        .collect();
    let mut flow_type_keys = BTreeMap::new();
    let mut flow_types = Vec::new();
    for r in rows(conn, "policy.policy_flow_type", at, state).await? {
        if let (Some(i), Some((key, label))) = (id(&r, "id"), named(&r)) {
            flow_type_keys.insert(i, key.clone());
            flow_types.push(FlowType { key, label });
        }
    }
    let (services, service_keys) = services(conn, at, state, &flow_type_keys).await?;
    let mut pattern_keys = BTreeMap::new();
    let mut patterns = Vec::new();
    for r in rows(conn, "policy.policy_pattern", at, state).await? {
        if let (Some(i), Some((key, label))) = (id(&r, "id"), named(&r)) {
            pattern_keys.insert(i, key.clone());
            patterns.push(Pattern {
                key,
                label,
                texts: codec::stored_texts(&r["texts"]),
            });
        }
    }
    let keys = Cited {
        flow_types: flow_type_keys,
        services: service_keys,
        patterns: pattern_keys,
    };
    let rules = rows(conn, "policy.policy_rule", at, state)
        .await?
        .iter()
        .filter_map(|r| rule(r, &keys))
        .collect();
    Ok(Parts {
        revision: u64::try_from(revision).unwrap_or_default(),
        default_locale: codec::default_locale(),
        axes,
        values,
        flow_types,
        services,
        rules,
        patterns,
        conditions: Vec::new(),
        signals: Vec::new(),
    })
}
