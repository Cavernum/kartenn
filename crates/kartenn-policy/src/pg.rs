//! The storage of the module in PostgreSQL (feature `pg`): the tables of the schema
//! `policy` (migration `0001_policy.sql`), on the connection of the changeset.

use kartenn_core::period::{Instant, Period};
use kartenn_core::rules::{Rule, Violation};
use kartenn_service::port::{StoreError, WriteError};
use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

/// The versioned tables of the module, as the generic writes of the core name them.
pub(crate) mod table {
    pub(crate) const AXIS: &str = "policy.policy_axis";
    pub(crate) const VALUE: &str = "policy.policy_value";
}

/// The filter `AND column = $n` of `columns`, their parameters numbered from `first`.
fn equalities(columns: &[(&str, &str)], first: usize) -> String {
    use std::fmt::Write as _;
    columns
        .iter()
        .enumerate()
        .fold(String::new(), |mut sql, (i, (column, _))| {
            let _ = write!(sql, " AND {column} = ${}", i + first);
            sql
        })
}

/// An instant as PostgreSQL reads it.
fn time(i: Instant) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).expect("an instant")
}

/// The bounds of `period` to bind; a `None` start is the instant of the transaction.
pub(crate) fn bounds(period: Option<Period>) -> (Option<OffsetDateTime>, Option<OffsetDateTime>) {
    period.map_or((None, None), |p| (Some(time(p.start())), p.end().map(time)))
}

pub(crate) fn store(e: &sqlx::Error) -> StoreError {
    StoreError(e.to_string())
}

/// The identifier of the property `key` declared at `at`.
pub(crate) async fn property(
    conn: &mut PgConnection,
    key: &str,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT id FROM public.property_declaration
          WHERE key = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(key)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// The value `value` of the closed list `property` living at `at`.
pub(crate) async fn enum_value(
    conn: &mut PgConnection,
    property: &str,
    value: &str,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT v.id FROM public.property_enum_value v
           JOIN public.property_declaration d ON d.id = v.property_id
          WHERE d.key = $1 AND v.key = $2
            AND d.valid_period @> coalesce($3::timestamptz, now())
            AND v.valid_period @> coalesce($3::timestamptz, now())",
    )
    .bind(property)
    .bind(value)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// The row of `table` living at `at` whose `columns` equal `values`, if any.
pub(crate) async fn find(
    conn: &mut PgConnection,
    table: &str,
    columns: &[(&str, &str)],
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    let filter = equalities(columns, 2);
    let sql = format!(
        "SELECT id FROM {table} WHERE valid_period @> coalesce($1::timestamptz, now()){filter}"
    );
    let mut query = sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).bind(at);
    for (_, value) in columns {
        query = query.bind(*value);
    }
    query.fetch_optional(conn).await.map_err(|e| store(&e))
}

/// Refuses, by `rule`, a row of `table` other than `id` with the same `columns` over
/// `period`: checked before the write, since a violated exclusion would abort the whole
/// transaction of the changeset.
pub(crate) async fn unique(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    columns: &[(&str, &str)],
    period: Option<Period>,
    rule: &'static Rule,
) -> Result<(), WriteError> {
    let filter = equalities(columns, 4);
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM {table} WHERE id <> $1
            AND valid_period && tstzrange(coalesce($2::timestamptz, now()), $3){filter})"
    );
    let (from, to) = bounds(period);
    let mut query = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(from)
        .bind(to);
    for (_, value) in columns {
        query = query.bind(*value);
    }
    let taken: bool = query.fetch_one(conn).await.map_err(|e| store(&e))?;
    if taken {
        let what: Vec<String> = columns.iter().map(|(c, v)| format!("{c} {v}")).collect();
        return Err(WriteError::Rule(
            Violation::new(
                rule,
                format!("{} is taken over the period", what.join(", ")),
            )
            .with_subjects([id]),
        ));
    }
    Ok(())
}

/// Inserts the first version of `row` (its columns, `id` included) in `table` over
/// `period`. The defaults of the table do not apply: `row` gives every column.
pub(crate) async fn insert(
    conn: &mut PgConnection,
    table: &str,
    row: &Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    let sql = format!(
        "INSERT INTO {table} SELECT * FROM jsonb_populate_record(NULL::{table},
             $1::jsonb || jsonb_build_object(
                 'valid_period', tstzrange(coalesce($2::timestamptz, now()), $3),
                 'changeset_id', $4::uuid))"
    );
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(row)
        .bind(from)
        .bind(to)
        .bind(changeset)
        .execute(conn)
        .await
        .map(drop)
        .map_err(|e| WriteError::Store(store(&e)))
}

/// Applies `patch` (columns and their new values) to the versions of `id` in `table`
/// over `period` (REQ-004-14).
pub(crate) async fn patch(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    patch: &Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    let versions: i32 = sqlx::query_scalar(
        "SELECT public.kartenn_portion($1::regclass, $2,
                                       coalesce($3::timestamptz, now()), $4, $5::jsonb, $6)",
    )
    .bind(table)
    .bind(id)
    .bind(from)
    .bind(to)
    .bind(patch)
    .bind(changeset)
    .fetch_one(conn)
    .await
    .map_err(|e| WriteError::Store(store(&e)))?;
    if versions == 0 {
        return Err(WriteError::NoVersion(id));
    }
    Ok(())
}

/// Raises the revision `counter` (`policy` or `inference`) once per changeset (REQ-010-26,
/// S17); the new revision when this write is the first of the changeset to raise it.
pub(crate) async fn bump(
    conn: &mut PgConnection,
    counter: &str,
    changeset: Uuid,
) -> Result<Option<i64>, StoreError> {
    let sql = format!(
        "UPDATE policy.policy_revision SET {counter} = {counter} + 1, {counter}_by = $1
          WHERE {counter}_by IS DISTINCT FROM $1 RETURNING {counter}"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(changeset)
        .fetch_optional(conn)
        .await
        .map_err(|e| store(&e))
}

/// The objects of the module living from `at` on that reference `target`, of type `to`
/// (REQ-008-10): the axes of a property, the attributes of a closed-list value.
pub(crate) async fn referrers(
    conn: &mut PgConnection,
    from: &str,
    to: &str,
    target: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Vec<Uuid>, StoreError> {
    let sql = match (from, to) {
        (crate::module::AXIS, "property") => {
            "SELECT DISTINCT a.id FROM policy.policy_axis a
               JOIN public.property_declaration d ON d.key = a.property_key
              WHERE d.id = $1
                AND a.valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)
              ORDER BY a.id"
        }
        (crate::module::VALUE, "enum_value") => {
            "SELECT DISTINCT p.id FROM policy.policy_value p
               JOIN public.property_enum_value v ON v.key = p.value_key
               JOIN public.property_declaration d ON d.id = v.property_id
                                                 AND d.key = p.property_key
              WHERE v.id = $1
                AND p.valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)
              ORDER BY p.id"
        }
        _ => return Ok(Vec::new()),
    };
    sqlx::query_scalar(sql)
        .bind(target)
        .bind(at)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))
}

/// The axes the changeset wrote that are not valid (REQ-010-29), with why: a property
/// absent at the start of the version, of another type than a closed list or an
/// external reference, or not carried by what the carrier of the axis needs.
pub(crate) async fn invalid_axes(
    conn: &mut PgConnection,
    changeset: Uuid,
) -> Result<Vec<(Uuid, String)>, StoreError> {
    sqlx::query_as(
        "SELECT a.id,
                CASE
                  WHEN d.id IS NULL THEN 'the property ' || a.property_key || ' is not declared'
                  WHEN d.value_type NOT IN ('enum', 'external_ref') THEN
                    'the property ' || a.property_key || ' is neither a closed list nor an external reference'
                  WHEN a.carrier = 'tree' AND NOT d.carriers && ARRAY['domain', 'prefix', 'address'] THEN
                    'the property ' || a.property_key || ' is not carried by the addressing tree'
                  ELSE 'the property ' || a.property_key || ' is not carried by the machines'
                END
           FROM policy.policy_axis a
           LEFT JOIN public.property_declaration d
                  ON d.key = a.property_key AND d.valid_period @> lower(a.valid_period)
          WHERE a.changeset_id = $1
            AND (d.id IS NULL
                 OR d.value_type NOT IN ('enum', 'external_ref')
                 OR (a.carrier = 'tree' AND NOT d.carriers && ARRAY['domain', 'prefix', 'address'])
                 OR (a.carrier = 'machine' AND NOT 'machines.machine' = ANY (d.carriers)))
          ORDER BY a.id",
    )
    .bind(changeset)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The attributes the changeset wrote that describe no living value of an axis
/// (REQ-010-2, 30): the property is neither `policy.environment` nor a designated axis at
/// the start of the version, or the value is not a living value of its closed list.
pub(crate) async fn unknown_values(
    conn: &mut PgConnection,
    changeset: Uuid,
) -> Result<Vec<(Uuid, String)>, StoreError> {
    sqlx::query_as(
        "SELECT p.id, p.property_key || '/' || p.value_key
           FROM policy.policy_value p
          WHERE p.changeset_id = $1
            AND (NOT (p.property_key = 'policy.environment'
                      OR EXISTS (SELECT 1 FROM policy.policy_axis a
                                  WHERE a.property_key = p.property_key
                                    AND a.valid_period @> lower(p.valid_period)))
                 OR NOT EXISTS (
                      SELECT 1 FROM public.property_enum_value v
                        JOIN public.property_declaration d ON d.id = v.property_id
                       WHERE d.key = p.property_key AND v.key = p.value_key
                         AND v.valid_period @> lower(p.valid_period)))
          ORDER BY p.id",
    )
    .bind(changeset)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}
