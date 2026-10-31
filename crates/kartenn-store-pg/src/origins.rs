//! Materialised origins of heritable properties along the addressing tree (spec 003,
//! task 003-8; ADR 0016, P9), along the entity and site trees (task 003-18), and along the
//! tree of each module (spec 008, REQ-008-6): an object of a module inherits along the
//! parents of `module_object`, whatever the tree of the declaration.
//!
//! The origin of a target over time is computed set-wise: its candidates are itself, the
//! prefixes of its domain that contain it (strictly for a prefix; from its network up for
//! a range or an address) and its domain; the timeline is cut at every bound of their
//! versions and of their assignments; on each segment the nearest candidate carrying an
//! assignment is the origin. Only the segments that differ from the stored ones are
//! written, merged with their unchanged neighbours. The candidates are found going up
//! from each target, or for many targets going down from the objects that carry an
//! assignment (same result, see [`DOWN_FROM`]).

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::properties::InheritTree;
use kartenn_service::port::StoreError;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::write::PgTx;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// A segment of the timeline of one object: origin, start, end (`None`: unbounded).
type Segment = (Uuid, OffsetDateTime, Option<OffsetDateTime>);

/// Targets of a refresh: `$1` objects, `$3` window start, `$4` carrier kinds.
///
/// The window is written out, never taken from a CTE: one cited twice, as it was in
/// [`SEGMENTS_DOWN`] (here and in `a`), is materialised, so a plan made for the
/// parameters does not see its value, estimates `valid_period &&` it at a default
/// selectivity and plans 2·10⁵ targets as about 1 700. [`SEGMENTS_DOWN`] then tested
/// each object below a carrier against every target: about 890 s for the query instead
/// of 2.4 s when the roots of three domains of the reference dataset lose their value
/// (task 003-20). [`SEGMENTS_UP`], which shares these targets, needs it only for
/// uniformity: a CTE cited once is folded into the query.
macro_rules! targets {
    () => {
        "
WITH t AS (
    SELECT o.id, o.domain_id, o.value, o.kind, o.valid_period * tstzrange($3, NULL) AS vp
    FROM addressing_object o
    WHERE o.id = ANY($1) AND o.kind = ANY($4) AND o.valid_period && tstzrange($3, NULL)
),"
    };
}

/// Origin of each target on each segment, from `t` and `hit` (candidates carrying an
/// assignment of `$2`, with their priority and the period of the pair).
macro_rules! segments {
    () => {
        "
pts AS (
    SELECT id AS target, lower(vp) AS p FROM t
    UNION SELECT id, coalesce(upper(vp), 'infinity') FROM t
    UNION SELECT target, lower(vp) FROM hit
    UNION SELECT target, coalesce(upper(vp), 'infinity') FROM hit
),
seg AS (SELECT target, p AS s, lead(p) OVER (PARTITION BY target ORDER BY p) AS e FROM pts)
SELECT DISTINCT ON (g.target, g.s) g.target, h.cand, g.s, nullif(g.e, 'infinity')
FROM seg g
JOIN t ON t.id = g.target AND t.vp @> g.s
JOIN hit h ON h.target = g.target AND h.vp @> g.s
WHERE g.e IS NOT NULL
ORDER BY g.target, g.s, h.prio DESC"
    };
}

/// Origin of each target on each segment of `[from, ∞)`, before merging, going up from
/// each target to its candidates.
///
/// Containment is written `>>=` plus `id <>` (prefixes are unique per domain) so that the
/// `GiST` index of `prefix` applies; the assignments are looked up per candidate behind an
/// `OFFSET 0` fence, as the statistics of a property assigned in this very changeset are
/// stale and would otherwise drive a nested loop over every candidate (bench 003-16).
const SEGMENTS_UP: &str = concat!(
    targets!(),
    "
cand AS (
    SELECT t.id AS target, t.id AS cand, 1000 AS prio, t.vp FROM t
    UNION ALL
    SELECT t.id, p.id, masklen(p.prefix), p.valid_period * t.vp
    FROM t JOIN prefix p ON p.domain_id = t.domain_id AND t.kind <> 'domain'
        AND p.valid_period && t.vp
        AND p.prefix >>= t.value AND p.id <> t.id
    UNION ALL
    SELECT t.id, d.id, -1, d.valid_period * t.vp
    FROM t JOIN addressing_domain d ON d.id = t.domain_id AND t.kind <> 'domain'
        AND d.valid_period && t.vp
),
hit AS (
    SELECT c.target, c.cand, c.prio, c.vp * a.valid_period AS vp
    FROM cand c CROSS JOIN LATERAL (
        SELECT a.valid_period FROM property_assignment a
        WHERE a.object_id = c.cand AND a.property_id = $2 AND a.valid_period && c.vp
        OFFSET 0) a
),",
    segments!()
);

/// As [`SEGMENTS_UP`], going down from each object carrying an assignment to the targets
/// below it: one index scan per carrier rather than one per target, but every carrier of
/// the targets' domains is walked whatever the number of targets.
///
/// The objects below a carrier are kept when they are targets by a join on `t`, never by
/// `x.id = ANY($1)`: once the prepared statement runs its generic plan, `$1` is a
/// parameter whose array PostgreSQL does not hash, and the test is linear in the targets
/// for each object walked (more than an hour for 10⁶ targets, task 010-19).
///
/// It is planned for its parameters at each run, never kept prepared: its generic plan,
/// chosen from the sixth run on a connection, plans the array of targets as a few rows
/// (291 s instead of 0.5 s for the 1.4·10⁵ targets of the roots on the reduced bench,
/// 73 s for 2·10⁴ before task 010-19; task 003-20).
const SEGMENTS_DOWN: &str = concat!(
    targets!(),
    "
a AS (
    SELECT a.object_id, a.valid_period * tstzrange($3, NULL) AS vp FROM property_assignment a
    WHERE a.property_id = $2 AND a.valid_period && tstzrange($3, NULL)
),
hit AS (
    SELECT target, cand, prio, vp FROM (
        SELECT t.id AS target, t.id AS cand, 1000 AS prio, t.vp * a.vp AS vp
        FROM t JOIN a ON a.object_id = t.id
        UNION ALL
        SELECT x.id, p.id, masklen(p.prefix), x.valid_period * p.valid_period * a.vp
        FROM a JOIN prefix p ON p.id = a.object_id AND p.valid_period && a.vp
            AND p.domain_id IN (SELECT DISTINCT domain_id FROM t)
        JOIN addressing_object x ON x.domain_id = p.domain_id AND x.value <<= p.prefix
            AND x.id <> p.id AND x.valid_period && a.vp
            AND x.id IN (SELECT id FROM t)
        UNION ALL
        SELECT x.id, d.id, -1, x.valid_period * d.valid_period * a.vp
        FROM a JOIN addressing_domain d ON d.id = a.object_id AND d.valid_period && a.vp
            AND d.id IN (SELECT DISTINCT domain_id FROM t)
        JOIN addressing_object x ON x.domain_id = d.id AND x.kind <> 'domain'
            AND x.valid_period && a.vp AND x.id IN (SELECT id FROM t)
    ) h WHERE NOT isempty(vp)
),",
    segments!()
);

/// Origin of each target on each segment of `[from, ∞)` along the stored parents of an
/// entity or site tree (003-18): the candidates are the target, then each parent in turn
/// over the time the link holds, nearest first (a cycle is refused by the core;
/// the depth is bounded all the same).
macro_rules! segments_anchor {
    ($table:literal) => {
        concat!(
            "
WITH RECURSIVE w AS (SELECT tstzrange($3, NULL) AS r),
t AS (
    SELECT o.id, o.valid_period * w.r AS vp FROM ",
            $table,
            " o, w
    WHERE o.id = ANY($1) AND o.valid_period && w.r AND cardinality($4::text[]) >= 0
),
cand(target, cand, depth, vp) AS (
    SELECT t.id, t.id, 0, t.vp FROM t
    UNION ALL
    SELECT c.target, x.parent_id, c.depth + 1, c.vp * x.valid_period
    FROM cand c JOIN ",
            $table,
            " x ON x.id = c.cand AND x.valid_period && c.vp AND x.parent_id IS NOT NULL
    WHERE c.depth < 1000
),
hit AS (
    SELECT c.target, c.cand, 1000 - c.depth AS prio, c.vp * a.valid_period AS vp
    FROM cand c CROSS JOIN LATERAL (
        SELECT a.valid_period FROM property_assignment a
        WHERE a.object_id = c.cand AND a.property_id = $2 AND a.valid_period && c.vp
        OFFSET 0) a
),",
            segments!()
        )
    };
}

const SEGMENTS_ENTITY: &str = segments_anchor!("entity");
const SEGMENTS_SITE: &str = segments_anchor!("site");
const SEGMENTS_MODULE: &str = segments_anchor!("module_object");
const SEGMENTS_VLAN: &str = segments_anchor!("vlan_tree");

/// From this many targets on, [`SEGMENTS_DOWN`] is used: on the reference dataset going
/// up costs about 30 µs per target, going down about 7 µs per object below a carrier of
/// the targets' domains (a domain holds about 6·10⁴ objects).
pub(crate) const DOWN_FROM: usize = 50_000;

/// Merges touching segments of the same origin; `segments` sorted by start.
fn merge(segments: Vec<Segment>) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::with_capacity(segments.len());
    for s in segments {
        match out.last_mut() {
            Some(last) if last.0 == s.0 && last.2 == Some(s.1) => last.2 = s.2,
            _ => out.push(s),
        }
    }
    out
}

/// A stored or computed row: object, origin, start, end.
type Row = (Uuid, Uuid, OffsetDateTime, Option<OffsetDateTime>);

/// Rows to delete (object, start) and to insert, and how many objects changed.
struct Changes {
    changed: usize,
    deleted: Vec<(Uuid, OffsetDateTime)>,
    inserted: Vec<(Uuid, Segment)>,
}

/// Compares the stored rows touching `[from, ∞)` with the rows wanted: the stored part
/// before `from`, then the computed segments, merged.
fn changes(stored: &[Row], computed: Vec<Row>, from: OffsetDateTime) -> Changes {
    let mut before: BTreeMap<Uuid, BTreeSet<Segment>> = BTreeMap::new();
    let mut desired: BTreeMap<Uuid, Vec<Segment>> = BTreeMap::new();
    for &(object, origin, lower, upper) in stored {
        before
            .entry(object)
            .or_default()
            .insert((origin, lower, upper));
        if lower < from {
            let end = upper.map_or(from, |u| u.min(from));
            desired
                .entry(object)
                .or_default()
                .push((origin, lower, Some(end)));
        }
    }
    for (object, origin, lower, upper) in computed {
        desired
            .entry(object)
            .or_default()
            .push((origin, lower, upper));
    }
    let mut out = Changes {
        changed: 0,
        deleted: Vec::new(),
        inserted: Vec::new(),
    };
    let objects: BTreeSet<Uuid> = before.keys().chain(desired.keys()).copied().collect();
    for object in objects {
        let mut wanted = desired.remove(&object).unwrap_or_default();
        wanted.sort_by_key(|s| s.1);
        let wanted: BTreeSet<Segment> = merge(wanted).into_iter().collect();
        let had = before.remove(&object).unwrap_or_default();
        if wanted != had {
            out.changed += 1;
            out.deleted
                .extend(had.difference(&wanted).map(|s| (object, s.1)));
            out.inserted
                .extend(wanted.difference(&had).map(|s| (object, *s)));
        }
    }
    out
}

impl PgTx {
    /// Start of the window of a changeset: its period start, or transaction time.
    async fn window_start(
        &mut self,
        start: Option<OffsetDateTime>,
    ) -> Result<OffsetDateTime, StoreError> {
        sqlx::query_scalar("SELECT coalesce($1::timestamptz, now())")
            .bind(start)
            .fetch_one(&mut *self.tx)
            .await
            .map_err(store_err)
    }

    /// The origins of `targets` on each segment of `[from, ∞)`, before merging: the objects
    /// of modules climb the tree of their module, the others the tree of the property.
    async fn computed_origins(
        &mut self,
        property: Uuid,
        tree: &InheritTree,
        targets: &[Uuid],
        from: OffsetDateTime,
    ) -> Result<Vec<Row>, StoreError> {
        // Both lookups are planned for the array of targets at each run, never kept
        // prepared: a generic plan may test it row by row over every object of a module or
        // every VLAN (task 003-20), and planning so small a query costs next to nothing.
        let modular: Vec<Uuid> =
            sqlx::query_scalar("SELECT DISTINCT id FROM module_object WHERE id = ANY($1)")
                .persistent(false)
                .bind(targets)
                .fetch_all(&mut *self.tx)
                .await
                .map_err(store_err)?;
        // The VLANs climb the tree of their groups (spec 026), as the objects of a module.
        let vlans: Vec<Uuid> =
            sqlx::query_scalar("SELECT DISTINCT id FROM vlan_tree WHERE id = ANY($1)")
                .persistent(false)
                .bind(targets)
                .fetch_all(&mut *self.tx)
                .await
                .map_err(store_err)?;
        let others: Vec<Uuid> = if modular.is_empty() && vlans.is_empty() {
            targets.to_vec()
        } else {
            let apart: BTreeSet<Uuid> = modular.iter().chain(&vlans).copied().collect();
            targets
                .iter()
                .filter(|t| !apart.contains(t))
                .copied()
                .collect()
        };
        let carriers: Vec<String> = tree.kinds().iter().map(|k| (*k).to_owned()).collect();
        let mut computed: Vec<Row> = Vec::new();
        // Each query with whether it is kept prepared: not the way down (see SEGMENTS_DOWN).
        let (segments, prepared) = match tree {
            InheritTree::Entity => (SEGMENTS_ENTITY, true),
            InheritTree::Site => (SEGMENTS_SITE, true),
            InheritTree::Module(_) => (SEGMENTS_MODULE, true),
            InheritTree::Addressing if others.len() >= self.down_from => (SEGMENTS_DOWN, false),
            InheritTree::Addressing => (SEGMENTS_UP, true),
        };
        for (segments, prepared, objects) in [
            (segments, prepared, &others),
            (SEGMENTS_MODULE, true, &modular),
            (SEGMENTS_VLAN, true, &vlans),
        ] {
            if objects.is_empty() {
                continue;
            }
            computed.extend(
                sqlx::query_as::<_, Row>(segments)
                    .persistent(prepared)
                    .bind(objects)
                    .bind(property)
                    .bind(from)
                    .bind(&carriers)
                    .fetch_all(&mut *self.tx)
                    .await
                    .map_err(store_err)?,
            );
        }
        Ok(computed)
    }

    /// Recomputes and writes the origins of `targets` from `start` on (see the module
    /// documentation); returns the number of objects whose stored rows changed.
    pub(crate) async fn refresh_origins_from(
        &mut self,
        property: Uuid,
        tree: &InheritTree,
        targets: &[Uuid],
        start: Option<OffsetDateTime>,
        changeset: Uuid,
    ) -> Result<usize, StoreError> {
        if targets.is_empty() {
            return Ok(0);
        }
        let from = self.window_start(start).await?;
        let computed = self.computed_origins(property, tree, targets, from).await?;
        let stored: Vec<Row> = sqlx::query_as(
            "SELECT object_id, origin_object_id, lower(valid_period), upper(valid_period)
             FROM property_origin
             WHERE property_id = $1 AND object_id = ANY($2) AND origin_object_id IS NOT NULL
               AND (valid_period && tstzrange($3, NULL) OR upper(valid_period) = $3)",
        )
        .bind(property)
        .bind(targets)
        .bind(from)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;

        let Changes {
            changed,
            deleted,
            inserted,
        } = changes(&stored, computed, from);
        if !deleted.is_empty() {
            let (o, l): (Vec<Uuid>, Vec<OffsetDateTime>) = deleted.into_iter().unzip();
            sqlx::query(
                "DELETE FROM property_origin p USING unnest($2::uuid[], $3::timestamptz[]) AS d(o, l)
                 WHERE p.property_id = $1 AND p.object_id = d.o AND lower(p.valid_period) = d.l",
            )
            .bind(property)
            .bind(o)
            .bind(l)
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
        }
        if !inserted.is_empty() {
            let mut o = Vec::with_capacity(inserted.len());
            let mut g = Vec::with_capacity(inserted.len());
            let mut l = Vec::with_capacity(inserted.len());
            let mut u: Vec<Option<OffsetDateTime>> = Vec::with_capacity(inserted.len());
            for (object, (origin, lower, upper)) in inserted {
                o.push(object);
                g.push(origin);
                l.push(lower);
                u.push(upper);
            }
            sqlx::query(
                "INSERT INTO property_origin (object_id, property_id, origin_object_id, valid_period, changeset_id)
                 SELECT n.o, $1, n.g, tstzrange(n.l, n.u), $6
                 FROM unnest($2::uuid[], $3::uuid[], $4::timestamptz[], $5::timestamptz[]) AS n(o, g, l, u)",
            )
            .bind(property)
            .bind(o)
            .bind(g)
            .bind(l)
            .bind(u)
            .bind(changeset)
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
        }
        Ok(changed)
    }

    /// Ends every origin of a property from `start` on.
    pub(crate) async fn drop_origins_from(
        &mut self,
        property: Uuid,
        start: Option<OffsetDateTime>,
        changeset: Uuid,
    ) -> Result<(), StoreError> {
        let from = self.window_start(start).await?;
        sqlx::query(
            "UPDATE property_origin SET valid_period = tstzrange(lower(valid_period), $2), changeset_id = $3
             WHERE property_id = $1 AND valid_period @> $2 AND lower(valid_period) < $2",
        )
        .bind(property)
        .bind(from)
        .bind(changeset)
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        sqlx::query(
            "DELETE FROM property_origin WHERE property_id = $1 AND lower(valid_period) >= $2",
        )
        .bind(property)
        .bind(from)
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_queries_planned_for_their_arrays_see_the_window() {
        // A window named by a CTE, whatever its name, is `… AS (SELECT tstzrange(…`.
        for sql in [SEGMENTS_UP, SEGMENTS_DOWN] {
            assert!(
                !sql.contains("SELECT tstzrange("),
                "window behind a CTE in:\n{sql}"
            );
            assert!(sql.contains("&& tstzrange($3, NULL)"));
        }
    }

    #[test]
    fn touching_segments_of_one_origin_merge() {
        let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let t = |d: i64| {
            OffsetDateTime::from_unix_timestamp(1_900_000_000 + d * 86_400).expect("valid instant")
        };
        let merged = merge(vec![
            (a, t(0), Some(t(1))),
            (a, t(1), Some(t(2))),
            (b, t(2), Some(t(3))),
            (a, t(3), None),
        ]);
        assert_eq!(
            merged,
            vec![
                (a, t(0), Some(t(2))),
                (b, t(2), Some(t(3))),
                (a, t(3), None)
            ]
        );
    }
}
