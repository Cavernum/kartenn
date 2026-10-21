//! The rules of the module on the final state of a changeset (feature `pg`; design 010
//! « Opérations », table of rules): every version of an object of the catalogue the
//! changeset wrote is read back and checked at the start of its period against the
//! designated axes, the default language and the objects it cites.

use std::collections::BTreeSet;

use kartenn_core::rules::Violation;
use kartenn_service::port::StoreError;
use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::codec;
use crate::model::{
    AxisKey, Condition, Effect, FlowSelector, Key, Label, Pattern, Rule, Service, Signal,
};
use crate::module::ENVIRONMENT;
use crate::pg::store;
use crate::rules::REFERENCE_CLOSED;

/// The axes designated at `at`, `policy.environment` always among them (S22).
async fn designated(
    conn: &mut PgConnection,
    at: OffsetDateTime,
) -> Result<BTreeSet<AxisKey>, StoreError> {
    let keys: Vec<String> =
        sqlx::query_scalar("SELECT property_key FROM policy.policy_axis WHERE valid_period @> $1")
            .bind(at)
            .fetch_all(conn)
            .await
            .map_err(|e| store(&e))?;
    Ok(keys
        .iter()
        .chain(std::iter::once(&ENVIRONMENT.to_owned()))
        .filter_map(|k| AxisKey::parse(k).ok())
        .collect())
}

/// The versions of `table` the changeset wrote: object, start, columns.
async fn written(
    conn: &mut PgConnection,
    table: &str,
    changeset: Uuid,
) -> Result<Vec<(Uuid, OffsetDateTime, Value)>, StoreError> {
    let sql = format!(
        "SELECT id, lower(valid_period), to_jsonb(t) FROM {table} t
          WHERE changeset_id = $1 ORDER BY id, lower(valid_period)"
    );
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(changeset)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))
}

/// The references the changeset wrote to objects that do not live at the start of the
/// citing version (REQ-010-2, 8): `(object, column, cited)`.
async fn dead_references(
    conn: &mut PgConnection,
    changeset: Uuid,
) -> Result<Vec<(Uuid, String, String)>, StoreError> {
    // (citing table, its column, an array of identifiers, the cited table)
    const CITED: &[(&str, &str, &str)] = &[
        ("policy_service", "ARRAY[flow_type_id]", "policy_flow_type"),
        ("policy_rule", "flow_type_ids", "policy_flow_type"),
        ("policy_rule", "service_ids", "policy_service"),
        ("policy_rule", "required_pattern_ids", "policy_pattern"),
        (
            "policy_rule",
            "array_remove(ARRAY[pattern_id], NULL)",
            "policy_pattern",
        ),
        ("policy_condition", "rule_ids", "policy_rule"),
        ("policy_condition", "flow_type_ids", "policy_flow_type"),
    ];
    let mut dead = Vec::new();
    for (from, column, to) in CITED {
        let sql = format!(
            "SELECT r.id, '{to}', x::text FROM policy.{from} r, unnest({column}) x
              WHERE r.changeset_id = $1
                AND NOT EXISTS (SELECT 1 FROM policy.{to} u
                                 WHERE u.id = x AND u.valid_period @> lower(r.valid_period))"
        );
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(changeset)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
        dead.extend(rows);
    }
    // The values of closed lists a rule admits: living values of the list of the axis (or
    // of the list it borrows, REQ-016-2) at the start of the rule.
    let values: Vec<(Uuid, String, String)> = sqlx::query_as(
        "WITH terms AS (
             SELECT r.id, lower(r.valid_period) AS at, e.key AS axis, v AS value
               FROM policy.policy_rule r,
                    LATERAL (SELECT * FROM jsonb_each(r.source)
                             UNION ALL SELECT * FROM jsonb_each(r.destination)) e,
                    jsonb_array_elements_text(e.value) v
              WHERE r.changeset_id = $1)
         SELECT DISTINCT t.id, t.axis, t.value
           FROM terms t
           JOIN public.property_declaration d
             ON d.key = t.axis AND d.valid_period @> t.at AND d.value_type = 'enum'
          WHERE NOT EXISTS (
                SELECT 1 FROM public.property_enum_value v
                  JOIN public.property_declaration o ON o.id = v.property_id
                 WHERE o.key = coalesce(d.type_params->>'values_of', d.key)
                   AND v.key = t.value AND v.valid_period @> t.at)",
    )
    .bind(changeset)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    dead.extend(values);
    Ok(dead)
}

fn text(row: &Value, column: &str) -> String {
    row.get(column)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn key_of(row: &Value) -> Option<Key> {
    Key::new(&text(row, "key")).ok()
}

/// Keys standing for `n` cited objects: the checks of a single object read only whether
/// a list is empty.
fn stand_ins(row: &Value, column: &str) -> BTreeSet<Key> {
    let n = row
        .get(column)
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    (0..n)
        .filter_map(|i| Key::new(&format!("x{i}")).ok())
        .collect()
}

/// The violations of a version of a rule (REQ-010-8, 29, 32, 33).
fn rule(row: &Value, axes: &BTreeSet<AxisKey>) -> Vec<Violation> {
    let (Some(key), Ok(label)) = (key_of(row), Label::new(&text(row, "label"))) else {
        return Vec::new();
    };
    let selector = |side: &str| {
        row.get(side)
            .and_then(|s| codec::selector(s).ok())
            .map(|(s, _)| s)
            .unwrap_or_default()
    };
    let rule = Rule {
        key,
        label,
        source: selector("source"),
        destination: selector("destination"),
        same: row
            .get("same")
            .and_then(|s| codec::axes(s).ok())
            .unwrap_or_default(),
        flows: FlowSelector {
            flow_types: stand_ins(row, "flow_type_ids"),
            services: stand_ins(row, "service_ids"),
        },
        effect: if text(row, "effect") == "deny" {
            Effect::Deny
        } else {
            Effect::Allow
        },
        required_patterns: stand_ins(row, "required_pattern_ids"),
        pattern: None,
        texts: codec::stored_texts(&row["texts"]),
    };
    rule.check(axes, &codec::default_locale())
        .err()
        .unwrap_or_default()
}

/// The violations and warnings of the catalogue objects the changeset wrote.
pub(crate) async fn catalogue(
    conn: &mut PgConnection,
    changeset: Uuid,
) -> Result<Vec<Violation>, StoreError> {
    let mut found = Vec::new();
    for (id, table, cited) in dead_references(conn, changeset).await? {
        found.push(
            Violation::new(
                &REFERENCE_CLOSED,
                format!("cites {cited} ({table}), which does not live at the start of its period"),
            )
            .with_subjects([id]),
        );
    }
    let default = codec::default_locale();
    for (id, _, row) in written(conn, "policy.policy_service", changeset).await? {
        let entries = codec::entries(&row["entries"]).map(|(e, _)| e);
        if let (Some(key), Ok(label), Ok(entries)) =
            (key_of(&row), Label::new(&text(&row, "label")), entries)
            && let Ok(service) = Service::new(key.clone(), label, &[key], entries)
        {
            found.extend(
                service
                    .warnings()
                    .into_iter()
                    .map(|w| w.with_subjects([id])),
            );
        }
    }
    for (id, at, row) in written(conn, "policy.policy_rule", changeset).await? {
        let axes = designated(conn, at).await?;
        found.extend(rule(&row, &axes).into_iter().map(|v| v.with_subjects([id])));
    }
    for (id, _, row) in written(conn, "policy.policy_pattern", changeset).await? {
        let (Some(key), Ok(label)) = (key_of(&row), Label::new(&text(&row, "label"))) else {
            continue;
        };
        let pattern = Pattern {
            key,
            label,
            texts: codec::stored_texts(&row["texts"]),
        };
        if let Err(v) = pattern.check(&default) {
            found.push(v.with_subjects([id]));
        }
    }
    for (id, at, row) in written(conn, "policy.policy_condition", changeset).await? {
        let axes = designated(conn, at).await?;
        let (Some(key), Ok(kind)) = (
            key_of(&row),
            codec::condition(&text(&row, "kind"), &row["params"]),
        ) else {
            continue;
        };
        let condition = Condition {
            key,
            kind,
            rules: BTreeSet::new(),
            flow_types: BTreeSet::new(),
            texts: codec::stored_texts(&row["texts"]),
        };
        if let Err(violations) = condition.check(&axes, &default) {
            found.extend(violations.into_iter().map(|v| v.with_subjects([id])));
        }
    }
    for (id, at, row) in written(conn, "policy.policy_signal", changeset).await? {
        let axes = designated(conn, at).await?;
        let (Some(key), Ok(kind)) = (
            key_of(&row),
            codec::signal(&text(&row, "kind"), &row["params"]),
        ) else {
            continue;
        };
        let signal = Signal {
            key,
            kind,
            weight: row
                .get("weight")
                .and_then(Value::as_i64)
                .and_then(|w| i32::try_from(w).ok())
                .unwrap_or_default(),
        };
        if let Err(v) = signal.check(&axes) {
            found.push(v.with_subjects([id]));
        }
    }
    Ok(found)
}
