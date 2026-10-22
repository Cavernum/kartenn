//! Obligations of properties: storage with their declaration and the objects left
//! without a value (spec 003, REQ-003-16; task 003-10).

use std::collections::BTreeSet;

use kartenn_core::properties::ObligationScope;
use kartenn_core::rules::{self, Violation};
use kartenn_service::port::{StoreError, WriteError};
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
    /// Obligations of a property valid at `at` (transaction time if `None`), of the
    /// origin `origin` (`module` or `admin`), or of both if `None` (REQ-008-28).
    pub(crate) async fn obligations_at(
        &mut self,
        property: Uuid,
        at: Option<OffsetDateTime>,
        origin: Option<&str>,
    ) -> Result<BTreeSet<ObligationScope>, StoreError> {
        let rows: Vec<(String, Option<Uuid>, Option<String>)> = sqlx::query_as(
            "SELECT scope_kind, scope_id, scope_type FROM property_obligation
             WHERE property_id = $1 AND valid_period @> coalesce($2::timestamptz, now())
               AND ($3::text IS NULL OR origin = $3)",
        )
        .bind(property)
        .bind(at)
        .bind(origin)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        rows.into_iter()
            .map(|(kind, id, ty)| decode(&kind, id, ty))
            .collect()
    }

    /// Makes the obligations of the declaration of a property (origin `module`) from
    /// `start` to `end` those of `wanted`: the others end at `start`, the missing ones
    /// begin there. Those of an administrator stay as they are; a scope one of them already
    /// covers is not added twice (REQ-008-28).
    pub(crate) async fn set_obligations(
        &mut self,
        property: Uuid,
        wanted: &BTreeSet<ObligationScope>,
        start: Option<OffsetDateTime>,
        end: Option<OffsetDateTime>,
        changeset: Uuid,
    ) -> Result<(), StoreError> {
        let current = self.obligations_at(property, start, Some("module")).await?;
        let admin = self.obligations_at(property, start, Some("admin")).await?;
        for scope in current.difference(wanted) {
            self.end_obligation(property, scope, "module", start, changeset)
                .await?;
        }
        for scope in wanted.difference(&current) {
            if !admin.contains(scope) {
                self.begin_obligation(property, scope, "module", start, end, changeset)
                    .await?;
            }
        }
        Ok(())
    }

    /// Ends at `start` the obligation `scope` of origin `origin`.
    async fn end_obligation(
        &mut self,
        property: Uuid,
        scope: &ObligationScope,
        origin: &str,
        start: Option<OffsetDateTime>,
        changeset: Uuid,
    ) -> Result<(), StoreError> {
        let (kind, id, ty) = encode(scope);
        sqlx::query(
            "UPDATE property_obligation
             SET valid_period = tstzrange(lower(valid_period), coalesce($5::timestamptz, now())),
                 changeset_id = $6
             WHERE property_id = $1 AND scope_kind = $2 AND scope_id IS NOT DISTINCT FROM $3
               AND scope_type IS NOT DISTINCT FROM $4 AND origin = $7
               AND valid_period @> coalesce($5::timestamptz, now())
               AND lower(valid_period) < coalesce($5::timestamptz, now())",
        )
        .bind(property)
        .bind(kind)
        .bind(id)
        .bind(ty.clone())
        .bind(start)
        .bind(changeset)
        .bind(origin)
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        sqlx::query(
            "DELETE FROM property_obligation
             WHERE property_id = $1 AND scope_kind = $2 AND scope_id IS NOT DISTINCT FROM $3
               AND scope_type IS NOT DISTINCT FROM $4 AND origin = $6
               AND lower(valid_period) >= coalesce($5::timestamptz, now())",
        )
        .bind(property)
        .bind(kind)
        .bind(id)
        .bind(ty)
        .bind(start)
        .bind(origin)
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(())
    }

    /// Begins at `start`, up to `end`, the obligation `scope` of origin `origin`.
    async fn begin_obligation(
        &mut self,
        property: Uuid,
        scope: &ObligationScope,
        origin: &str,
        start: Option<OffsetDateTime>,
        end: Option<OffsetDateTime>,
        changeset: Uuid,
    ) -> Result<(), StoreError> {
        let (kind, id, ty) = encode(scope);
        sqlx::query(
            "INSERT INTO property_obligation
                 (id, property_id, scope_kind, scope_id, scope_type, valid_period, changeset_id, origin)
             VALUES (gen_random_uuid(), $1, $2, $3, $4,
                     tstzrange(coalesce($5::timestamptz, now()), $6::timestamptz), $7, $8)",
        )
        .bind(property)
        .bind(kind)
        .bind(id)
        .bind(ty)
        .bind(start)
        .bind(end)
        .bind(changeset)
        .bind(origin)
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(())
    }

    /// Adds (`required`) or removes an administrator's obligation `scope` on `property`
    /// from `start` to `end` (REQ-008-28): refused when the scope is already required, or,
    /// for a removal, when no administrator required it.
    pub(crate) async fn admin_obligation(
        &mut self,
        property: Uuid,
        scope: &ObligationScope,
        required: bool,
        start: Option<OffsetDateTime>,
        end: Option<OffsetDateTime>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let refused = |message: String| {
            WriteError::Rule(
                Violation::new(&rules::PROP_INCOMPATIBLE_CHANGE, message).with_subjects([property]),
            )
        };
        if required {
            if self
                .obligations_at(property, start, None)
                .await?
                .contains(scope)
            {
                return Err(refused(format!(
                    "{property}: {scope:?} is already required"
                )));
            }
            self.begin_obligation(property, scope, "admin", start, end, changeset)
                .await?;
        } else {
            if !self
                .obligations_at(property, start, Some("admin"))
                .await?
                .contains(scope)
            {
                return Err(refused(format!(
                    "{property}: no administrator requires it in {scope:?}"
                )));
            }
            self.end_obligation(property, scope, "admin", start, changeset)
                .await?;
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
        // The candidates joined from `unnest` when given, rather than `$7 IS NULL OR id =
        // ANY($7)`: under a generic plan that filter read every carrier and searched the
        // array for each (an import of 10⁴ machines checks as many candidates).
        let (from_carrier, among_entities, among_owned) = if candidates.is_some() {
            (
                // each candidate looked up by its identifier (`OFFSET 0` keeps the
                // subquery apart): the statistics of the tables an import fills are stale
                // within it, and a join let the planner scan them once per candidate; the
                // scope inside, so that the tables of other kinds are not probed
                "cand CROSS JOIN LATERAL (
                     SELECT * FROM carrier_object x
                     WHERE x.id = cand.id AND x.kind = ANY($2)
                       AND CASE $4 WHEN 'domain' THEN x.domain_id = $5
                                   WHEN 'object_type' THEN x.kind = $6
                                   ELSE false END
                     OFFSET 0) x",
                "AND sub.id IN (SELECT id FROM cand)",
                "JOIN cand ON cand.id = p.object_id",
            )
        } else {
            ("carrier_object x", "", "")
        };
        // The branches of an entity scope only for one: elsewhere `sub` is empty, but the
        // generic plan still looked up the owner of every candidate before joining it.
        let entity_branches = if kind == "entity" {
            format!(
                "UNION ALL
                 -- the entities of the subtree that carry the property
                 SELECT sub.id, sub.vp FROM sub
                 WHERE 'entity' = ANY($2) {among_entities}
                 UNION ALL
                 -- the objects whose resolved owner is in the subtree
                 SELECT p.object_id, p.valid_period * a.valid_period * sub.vp * x.valid_period
                 FROM property_origin p
                 {among_owned}
                 JOIN property_assignment a ON a.object_id = p.origin_object_id
                     AND a.property_id = p.property_id AND a.kind = 'value'
                     AND a.valid_period && p.valid_period
                 JOIN sub ON sub.id = a.v_uuid AND sub.vp && (p.valid_period * a.valid_period)
                 JOIN carrier_object x ON x.id = p.object_id AND x.kind = ANY($2)
                     AND x.valid_period && (p.valid_period * a.valid_period * sub.vp)
                 WHERE p.property_id IN (SELECT id FROM property_declaration WHERE key = 'core.owner')"
            )
        } else {
            String::new()
        };
        let sql = format!(
            "WITH RECURSIVE w AS (SELECT tstzrange(coalesce($8::timestamptz, now()), NULL) AS r),
             cand AS (SELECT DISTINCT id FROM unnest($7::uuid[]) AS u(id)),
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
                 FROM {from_carrier}, w
                 WHERE x.kind = ANY($2) AND x.valid_period && w.r
                   AND CASE $4 WHEN 'domain' THEN x.domain_id = $5
                               WHEN 'object_type' THEN x.kind = $6
                               ELSE false END
                 {entity_branches}
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
        );
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
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
