//! Obligations of properties: storage with their declaration and the objects left
//! without a value (spec 003, REQ-003-16; task 003-10).

use std::collections::BTreeSet;

use kartenn_core::properties::ObligationScope;
use kartenn_service::port::StoreError;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::write::PgTx;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Columns of a scope: kind, identifier, object type.
fn encode(scope: &ObligationScope) -> (&'static str, Option<Uuid>, Option<String>) {
    match scope {
        ObligationScope::Domain(id) => ("domain", Some(*id), None),
        ObligationScope::Entity(id) => ("entity", Some(*id), None),
        ObligationScope::ObjectType(kind) => ("object_type", None, Some(kind.clone())),
    }
}

fn decode(kind: &str, id: Option<Uuid>, ty: Option<String>) -> Result<ObligationScope, StoreError> {
    match (kind, id, ty) {
        ("domain", Some(id), None) => Ok(ObligationScope::Domain(id)),
        ("entity", Some(id), None) => Ok(ObligationScope::Entity(id)),
        ("object_type", None, Some(ty)) => Ok(ObligationScope::ObjectType(ty)),
        (kind, ..) => Err(store_err(format!("stored obligation of scope {kind}"))),
    }
}

impl PgTx {
    /// Obligations of a property valid at `at` (transaction time if `None`).
    pub(crate) async fn obligations_at(
        &mut self,
        property: Uuid,
        at: Option<OffsetDateTime>,
    ) -> Result<BTreeSet<ObligationScope>, StoreError> {
        let rows: Vec<(String, Option<Uuid>, Option<String>)> = sqlx::query_as(
            "SELECT scope_kind, scope_id, scope_type FROM property_obligation
             WHERE property_id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
        )
        .bind(property)
        .bind(at)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        rows.into_iter()
            .map(|(kind, id, ty)| decode(&kind, id, ty))
            .collect()
    }

    /// Makes the obligations of a property from `start` to `end` those of `wanted`: the
    /// others end at `start`, the missing ones begin there.
    pub(crate) async fn set_obligations(
        &mut self,
        property: Uuid,
        wanted: &BTreeSet<ObligationScope>,
        start: Option<OffsetDateTime>,
        end: Option<OffsetDateTime>,
        changeset: Uuid,
    ) -> Result<(), StoreError> {
        let current = self.obligations_at(property, start).await?;
        for scope in current.difference(wanted) {
            let (kind, id, ty) = encode(scope);
            sqlx::query(
                "UPDATE property_obligation
                 SET valid_period = tstzrange(lower(valid_period), coalesce($5::timestamptz, now())),
                     changeset_id = $6
                 WHERE property_id = $1 AND scope_kind = $2 AND scope_id IS NOT DISTINCT FROM $3
                   AND scope_type IS NOT DISTINCT FROM $4
                   AND valid_period @> coalesce($5::timestamptz, now())
                   AND lower(valid_period) < coalesce($5::timestamptz, now())",
            )
            .bind(property)
            .bind(kind)
            .bind(id)
            .bind(ty.clone())
            .bind(start)
            .bind(changeset)
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
            sqlx::query(
                "DELETE FROM property_obligation
                 WHERE property_id = $1 AND scope_kind = $2 AND scope_id IS NOT DISTINCT FROM $3
                   AND scope_type IS NOT DISTINCT FROM $4
                   AND lower(valid_period) >= coalesce($5::timestamptz, now())",
            )
            .bind(property)
            .bind(kind)
            .bind(id)
            .bind(ty)
            .bind(start)
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
        }
        for scope in wanted.difference(&current) {
            let (kind, id, ty) = encode(scope);
            sqlx::query(
                "INSERT INTO property_obligation (id, property_id, scope_kind, scope_id, scope_type, valid_period, changeset_id)
                 VALUES (gen_random_uuid(), $1, $2, $3, $4,
                         tstzrange(coalesce($5::timestamptz, now()), $6::timestamptz), $7)",
            )
            .bind(property)
            .bind(kind)
            .bind(id)
            .bind(ty)
            .bind(start)
            .bind(end)
            .bind(changeset)
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
        }
        Ok(())
    }

    /// Carrier objects of `scope` (among `candidates` if given) without a resolved value
    /// at some instant from `start` on: no origin, or an origin carrying "no value". An
    /// object is in the scope of an entity while its resolved owner (`core.owner`) is the
    /// entity or one of its descendants, an entity while it is one of them (003-18).
    pub(crate) async fn without_value(
        &mut self,
        property: Uuid,
        carriers: &[String],
        heritable: bool,
        scope: &ObligationScope,
        candidates: Option<&[Uuid]>,
        start: Option<OffsetDateTime>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (kind, id, ty) = encode(scope);
        sqlx::query_scalar(
            "WITH RECURSIVE w AS (SELECT tstzrange(coalesce($8::timestamptz, now()), NULL) AS r),
             -- scope entity: the entity and its descendants, over the time of the link
             sub(id, vp, depth) AS (
                 SELECT e.id, e.valid_period * w.r, 0 FROM entity e, w
                 WHERE $4 = 'entity' AND e.id = $5 AND e.valid_period && w.r
                 UNION ALL
                 SELECT c.id, c.valid_period * sub.vp, sub.depth + 1
                 FROM sub JOIN entity c ON c.parent_id = sub.id AND c.valid_period && sub.vp
                 WHERE sub.depth < 1000
             ),
             scoped AS (
                 SELECT x.id, x.valid_period * w.r AS vp
                 FROM carrier_object x, w
                 WHERE x.kind = ANY($2) AND x.valid_period && w.r
                   AND ($7::uuid[] IS NULL OR x.id = ANY($7))
                   AND CASE $4 WHEN 'domain' THEN x.domain_id = $5
                               WHEN 'object_type' THEN x.kind = $6
                               ELSE false END
                 UNION ALL
                 -- the entities of the subtree that carry the property
                 SELECT sub.id, sub.vp FROM sub
                 WHERE 'entity' = ANY($2) AND ($7::uuid[] IS NULL OR sub.id = ANY($7))
                 UNION ALL
                 -- the objects whose resolved owner is in the subtree
                 SELECT p.object_id, p.valid_period * a.valid_period * sub.vp * x.valid_period
                 FROM property_origin p
                 JOIN property_assignment a ON a.object_id = p.origin_object_id
                     AND a.property_id = p.property_id AND a.kind = 'value'
                     AND a.valid_period && p.valid_period
                 JOIN sub ON sub.id = a.v_uuid AND sub.vp && (p.valid_period * a.valid_period)
                 JOIN carrier_object x ON x.id = p.object_id AND x.kind = ANY($2)
                     AND x.valid_period && (p.valid_period * a.valid_period * sub.vp)
                 WHERE p.property_id IN (SELECT id FROM property_declaration WHERE key = 'core.owner')
                   AND ($7::uuid[] IS NULL OR p.object_id = ANY($7))
             ),
             o AS (
                 SELECT id, range_agg(vp) AS life FROM scoped WHERE NOT isempty(vp) GROUP BY id
             ),
             v AS (
                 SELECT p.object_id AS id, p.valid_period * a.valid_period AS vp
                 FROM property_origin p
                 JOIN property_assignment a ON a.object_id = p.origin_object_id
                     AND a.property_id = $1 AND a.kind = 'value' AND a.valid_period && p.valid_period
                 WHERE $3 AND p.property_id = $1 AND p.object_id IN (SELECT id FROM o)
                 UNION ALL
                 SELECT a.object_id, a.valid_period FROM property_assignment a
                 WHERE NOT $3 AND a.property_id = $1 AND a.kind = 'value'
                   AND a.object_id IN (SELECT id FROM o)
             ),
             c AS (SELECT id, range_agg(vp) AS covered FROM v GROUP BY id)
             SELECT o.id FROM o LEFT JOIN c ON c.id = o.id
             WHERE c.covered IS NULL OR NOT c.covered @> o.life
             ORDER BY o.id",
        )
        .bind(property)
        .bind(carriers)
        .bind(heritable)
        .bind(kind)
        .bind(id)
        .bind(ty)
        .bind(candidates)
        .bind(start)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)
    }
}
