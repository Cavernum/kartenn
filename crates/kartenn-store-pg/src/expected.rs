//! What sources of expected values read: a frozen view of an object and the resolved
//! value of a property (spec 003, REQ-003-21 to 24; task 003-13).

use std::collections::BTreeMap;

use kartenn_core::properties::Declaration;
use kartenn_core::values::Value;
use kartenn_service::port::{ObjectView, StoreError};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::diff::value_json;
use crate::properties::{ASSIGNMENT_COLUMNS, AssignmentRow, decode_assignment};
use crate::write::PgTx;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

impl PgTx {
    /// The object at `at` (transaction time if `None`) with the resolved values of its
    /// properties, by key.
    pub(crate) async fn view_of(
        &mut self,
        object: Uuid,
        at: Option<OffsetDateTime>,
    ) -> Result<Option<ObjectView>, StoreError> {
        let row: Option<(String, Uuid, Option<String>)> = sqlx::query_as(
            "SELECT kind, domain_id,
                    CASE kind WHEN 'prefix' THEN value::cidr::text ELSE host(value) END
             FROM addressing_object WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
        )
        .bind(object)
        .bind(at)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(store_err)?;
        let Some((kind, domain, value)) = row else {
            return Ok(None);
        };
        let json = value_json("d.value_type = 'enum'");
        let properties: Vec<(String, Option<String>)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "WITH t AS (SELECT coalesce($2::timestamptz, now()) AS at)
             SELECT d.key, ({json})::text
             FROM t, property_origin p
             JOIN property_assignment a ON a.object_id = p.origin_object_id AND a.property_id = p.property_id
             JOIN property_declaration d ON d.id = p.property_id
             WHERE p.object_id = $1 AND p.valid_period @> t.at AND a.valid_period @> t.at
               AND d.valid_period @> t.at
             UNION ALL
             SELECT d.key, ({json})::text
             FROM t, property_assignment a
             JOIN property_declaration d ON d.id = a.property_id AND d.inherit_tree IS NULL
             WHERE a.object_id = $1 AND a.valid_period @> t.at AND d.valid_period @> t.at"
        )))
        .bind(object)
        .bind(at)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(Some(ObjectView {
            id: object,
            kind,
            domain,
            value,
            properties: properties.into_iter().collect::<BTreeMap<_, _>>(),
        }))
    }

    /// [`Self::view_of`] of each of `objects`, in two reads whatever their number; in the
    /// order of `objects`, those that do not exist at `at` left out. The properties are
    /// read from the objects down, each behind an `OFFSET 0` fence: in the generic plan of
    /// the prepared statement, `object_id = ANY($1)` filtered every assignment of each
    /// property, linearly in the objects (minutes for 10⁴ objects, task 010-19). The own
    /// assignments are read once per object, then joined to their declarations: the
    /// planner otherwise probed each object once per own declaration (0.7 s for 10⁴
    /// objects in a changeset at 10⁶ addresses, 0.28 s since).
    pub(crate) async fn views_of(
        &mut self,
        objects: &[Uuid],
        at: Option<OffsetDateTime>,
    ) -> Result<Vec<ObjectView>, StoreError> {
        if objects.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<(Uuid, String, Uuid, Option<String>)> = sqlx::query_as(
            "SELECT id, kind, domain_id,
                    CASE kind WHEN 'prefix' THEN value::cidr::text ELSE host(value) END
             FROM addressing_object
             WHERE id = ANY($1) AND valid_period @> coalesce($2::timestamptz, now())",
        )
        .bind(objects)
        .bind(at)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        let json = value_json("d.value_type = 'enum'");
        let properties: Vec<(Uuid, String, Option<String>)> =
            sqlx::query_as(sqlx::AssertSqlSafe(format!(
                "WITH t AS (SELECT coalesce($2::timestamptz, now()) AS at),
                 own AS MATERIALIZED (
                     SELECT a.* FROM t, unnest($1::uuid[]) AS o(id)
                     CROSS JOIN LATERAL (
                         SELECT * FROM property_assignment a
                          WHERE a.object_id = o.id AND a.valid_period @> t.at
                         OFFSET 0) a)
                 SELECT p.object_id, d.key, ({json})::text
                 FROM t, unnest($1::uuid[]) AS o(id)
                 JOIN property_origin p ON p.object_id = o.id
                 JOIN property_assignment a ON a.object_id = p.origin_object_id AND a.property_id = p.property_id
                 JOIN property_declaration d ON d.id = p.property_id
                 WHERE p.valid_period @> t.at AND a.valid_period @> t.at
                   AND d.valid_period @> t.at
                 UNION ALL
                 SELECT a.object_id, d.key, ({json})::text
                 FROM t, own a
                 JOIN property_declaration d ON d.id = a.property_id AND d.inherit_tree IS NULL
                 WHERE d.valid_period @> t.at"
            )))
            .bind(objects)
            .bind(at)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
        let mut by_object: BTreeMap<Uuid, BTreeMap<String, Option<String>>> = BTreeMap::new();
        for (object, key, value) in properties {
            by_object.entry(object).or_default().insert(key, value);
        }
        let mut found: BTreeMap<Uuid, (String, Uuid, Option<String>)> = rows
            .into_iter()
            .map(|(id, kind, domain, value)| (id, (kind, domain, value)))
            .collect();
        Ok(objects
            .iter()
            .filter_map(|id| {
                let (kind, domain, value) = found.remove(id)?;
                Some(ObjectView {
                    id: *id,
                    kind,
                    domain,
                    value,
                    properties: by_object.remove(id).unwrap_or_default(),
                })
            })
            .collect())
    }

    /// Resolved value of a property for an object at `at`: the assignment of its origin
    /// (heritable) or its own; `None` when absent or "no value".
    pub(crate) async fn resolved_at(
        &mut self,
        object: Uuid,
        property: Uuid,
        declaration: &Declaration,
        at: Option<OffsetDateTime>,
    ) -> Result<Option<Value>, StoreError> {
        let row: Option<AssignmentRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "WITH t AS (SELECT coalesce($3::timestamptz, now()) AS at)
             SELECT {ASSIGNMENT_COLUMNS} FROM t, property_assignment
             WHERE property_id = $2 AND valid_period @> t.at
               AND object_id = CASE WHEN $4 THEN
                       (SELECT origin_object_id FROM property_origin
                        WHERE object_id = $1 AND property_id = $2 AND valid_period @> t.at)
                   ELSE $1 END"
        )))
        .bind(object)
        .bind(property)
        .bind(at)
        .bind(declaration.inherit.is_some())
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(row
            .map(|r| decode_assignment(declaration.spec.id(), r))
            .transpose()?
            .and_then(|a| a.value))
    }
}
