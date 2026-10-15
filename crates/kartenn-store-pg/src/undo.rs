//! What an applied changeset wrote, to undo it (spec 004, REQ-004-34, 35; task 004-20).

use kartenn_service::port::{StoreError, Touched, UndoFacts};
use kartenn_service::{ObjectKind, Operation};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Versioned objects a changeset may write, by kind.
const OBJECTS: [(ObjectKind, &str); 6] = [
    (ObjectKind::Domain, "addressing_domain"),
    (ObjectKind::Prefix, "prefix"),
    (ObjectKind::Range, "ip_range"),
    (ObjectKind::Address, "address"),
    (ObjectKind::Entity, "entity"),
    (ObjectKind::Site, "site"),
];

/// Per key of `table` written by `$1` (applied at `$2`): created (nothing asserted
/// before) and the first later changeset that wrote it over `[$3, ∞)`.
fn touched_sql(table: &str, key: &str, extra: &str) -> String {
    format!(
        "WITH k AS (SELECT DISTINCT {key}{extra} FROM {table} WHERE changeset_id = $1
                    UNION SELECT DISTINCT {key}{extra} FROM {table}_history WHERE changeset_id = $1),
              v AS (SELECT {key}, changeset_id, sys_start, NULL::timestamptz AS sys_end, valid_period
                    FROM {table} WHERE ({key}) IN (SELECT {key} FROM k)
                    UNION ALL
                    SELECT {key}, changeset_id, sys_start, sys_end, valid_period
                    FROM {table}_history WHERE ({key}) IN (SELECT {key} FROM k))
         SELECT k.*,
                NOT EXISTS (SELECT 1 FROM v WHERE ({key}) = (k.{key_k}) AND v.sys_start < $2
                                              AND (v.sys_end IS NULL OR v.sys_end >= $2)),
                (SELECT v.changeset_id FROM v WHERE ({key}) = (k.{key_k}) AND v.sys_start > $2
                   AND v.changeset_id <> $1 AND v.valid_period && tstzrange($3, NULL)
                 ORDER BY v.sys_start LIMIT 1)
         FROM k",
        key_k = key.replace(", ", ", k."),
    )
}

fn kind_of(name: &str) -> Result<ObjectKind, StoreError> {
    OBJECTS
        .iter()
        .find(|(k, _)| k.name() == name)
        .map(|(k, _)| *k)
        .ok_or_else(|| store_err(format!("unknown object kind {name}")))
}

/// What the applied changeset `id` wrote; `None` if it is not applied.
pub(crate) async fn facts(pool: &PgPool, id: Uuid) -> Result<Option<UndoFacts>, StoreError> {
    let row: Option<(Option<OffsetDateTime>, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT applied_at, lower(default_period) FROM changeset WHERE id = $1 AND state = 'applied'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(store_err)?;
    let Some((Some(at), Some(default_start))) = row else {
        return Ok(None);
    };
    // The earliest start: the changeset's period, or an operation's own (REQ-004-2).
    let payloads: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT payload FROM changeset_operation WHERE changeset_id = $1")
            .bind(id)
            .fetch_all(pool)
            .await
            .map_err(store_err)?;
    let micros =
        |t: OffsetDateTime| i64::try_from(t.unix_timestamp_nanos() / 1_000).unwrap_or(i64::MIN);
    let start = payloads
        .into_iter()
        .filter_map(|p| serde_json::from_value::<Operation>(p).ok())
        .filter_map(|op| op.own_period())
        .map(|p| p.start)
        .chain(std::iter::once(micros(default_start)))
        .min()
        .unwrap_or_else(|| micros(default_start));
    let from =
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(start) * 1_000).map_err(store_err)?;
    let mut touched = Vec::new();
    for (kind, table) in OBJECTS {
        let rows: Vec<(Uuid, bool, Option<Uuid>)> =
            sqlx::query_as(sqlx::AssertSqlSafe(touched_sql(table, "id", "")))
                .bind(id)
                .bind(at)
                .bind(from)
                .fetch_all(pool)
                .await
                .map_err(store_err)?;
        touched.extend(rows.into_iter().map(|(object, created, later)| Touched {
            kind,
            id: object,
            property: None,
            created,
            later,
        }));
    }
    let rows: Vec<(Uuid, Uuid, String, bool, Option<Uuid>)> =
        sqlx::query_as(sqlx::AssertSqlSafe(touched_sql(
            "property_assignment",
            "object_id, property_id",
            ", object_type",
        )))
        .bind(id)
        .bind(at)
        .bind(from)
        .fetch_all(pool)
        .await
        .map_err(store_err)?;
    for (object, property, kind, created, later) in rows {
        touched.push(Touched {
            kind: kind_of(&kind)?,
            id: object,
            property: Some(property),
            created,
            later,
        });
    }
    let unsupported: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT 'property', id FROM property_declaration WHERE changeset_id = $1
         UNION SELECT 'property', id FROM property_declaration_history WHERE changeset_id = $1
         UNION SELECT 'enum_value', id FROM property_enum_value WHERE changeset_id = $1
         UNION SELECT 'enum_value', id FROM property_enum_value_history WHERE changeset_id = $1
         ORDER BY 1, 2",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    Ok(Some(UndoFacts {
        start,
        touched,
        unsupported,
    }))
}
