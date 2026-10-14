//! Materialised origins of heritable properties along the addressing tree (spec 003,
//! task 003-8; ADR 0016, P9).
//!
//! The origin of a target over time is computed set-wise: its candidates are itself, the
//! prefixes of its domain that contain it (strictly for a prefix; from its network up for
//! a range or an address) and its domain; the timeline is cut at every bound of their
//! versions and of their assignments; on each segment the nearest candidate carrying an
//! assignment is the origin. Only the segments that differ from the stored ones are
//! written, merged with their unchanged neighbours.

use std::collections::{BTreeMap, BTreeSet};

use kartenn_service::port::StoreError;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::write::PgTx;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// A segment of the timeline of one object: origin, start, end (`None`: unbounded).
type Segment = (Uuid, OffsetDateTime, Option<OffsetDateTime>);

/// Origin of each target on each segment of `[from, ∞)`, before merging.
const SEGMENTS: &str = "
WITH w AS (SELECT tstzrange($3, NULL) AS r),
t AS (
    SELECT o.id, o.domain_id, o.value, o.kind, o.valid_period * w.r AS vp
    FROM addressing_object o, w
    WHERE o.id = ANY($1) AND o.kind = ANY($4) AND o.valid_period && w.r
),
cand AS (
    SELECT t.id AS target, t.id AS cand, 1000 AS prio, t.vp FROM t
    UNION ALL
    SELECT t.id, p.id, masklen(p.prefix), p.valid_period * t.vp
    FROM t JOIN prefix p ON p.domain_id = t.domain_id AND t.kind <> 'domain'
        AND p.valid_period && t.vp
        AND CASE WHEN t.kind = 'prefix' THEN p.prefix::inet >> t.value
                 ELSE p.prefix::inet >>= t.value END
    UNION ALL
    SELECT t.id, d.id, -1, d.valid_period * t.vp
    FROM t JOIN addressing_domain d ON d.id = t.domain_id AND t.kind <> 'domain'
        AND d.valid_period && t.vp
),
hit AS (
    SELECT c.target, c.cand, c.prio, c.vp * a.valid_period AS vp
    FROM cand c JOIN property_assignment a
        ON a.object_id = c.cand AND a.property_id = $2 AND a.valid_period && c.vp
),
pts AS (
    SELECT id AS target, lower(vp) AS p FROM t
    UNION SELECT id, coalesce(upper(vp), 'infinity') FROM t
    UNION SELECT target, lower(vp) FROM hit
    UNION SELECT target, coalesce(upper(vp), 'infinity') FROM hit
),
seg AS (SELECT target, p AS s, lead(p) OVER (PARTITION BY target ORDER BY p) AS e FROM pts)
SELECT DISTINCT ON (g.target, g.s) g.target, h.cand, g.s, nullif(g.e, 'infinity')
FROM seg g JOIN hit h ON h.target = g.target AND h.vp @> g.s
WHERE g.e IS NOT NULL AND EXISTS (SELECT 1 FROM t WHERE t.id = g.target AND t.vp @> g.s)
ORDER BY g.target, g.s, h.prio DESC";

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

    /// Recomputes and writes the origins of `targets` from `start` on (see the module
    /// documentation); returns the number of objects whose stored rows changed.
    pub(crate) async fn refresh_origins_from(
        &mut self,
        property: Uuid,
        carriers: &[String],
        targets: &[Uuid],
        start: Option<OffsetDateTime>,
        changeset: Uuid,
    ) -> Result<usize, StoreError> {
        if targets.is_empty() {
            return Ok(0);
        }
        let from = self.window_start(start).await?;
        let computed: Vec<Row> = sqlx::query_as(SEGMENTS)
            .bind(targets)
            .bind(property)
            .bind(from)
            .bind(carriers)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
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
