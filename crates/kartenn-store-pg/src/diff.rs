//! Differential of resolved property values (spec 003, REQ-003-19, 20; task 003-9).
//!
//! The values before the transaction are read from the versions it did not write: current
//! versions older than it, and the versions it replaced (history closed at its instant).
//! The entries are gathered in a temporary table of the transaction, counted for the
//! result, and copied to `changeset_diff` with the journal.

use kartenn_service::port::{DiffCount, DiffItem, StoreError};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::write::PgTx;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Resolved value of an assignment row `a` as JSON: closed-list keys when the SQL boolean
/// `closed_list` holds, null for "no value".
pub(crate) fn value_json(closed_list: &str) -> String {
    format!(
        "CASE
    WHEN a.kind = 'cleared' THEN NULL
    WHEN {closed_list} AND a.v_uuid IS NOT NULL THEN
        to_jsonb((SELECT e.key FROM property_enum_value e WHERE e.id = a.v_uuid LIMIT 1))
    WHEN {closed_list} THEN
        (SELECT jsonb_agg(k ORDER BY k) FROM (
            SELECT DISTINCT e.key AS k FROM property_enum_value e WHERE e.id = ANY(a.v_uuids)) x)
    ELSE coalesce(to_jsonb(a.v_uuid), to_jsonb(a.v_uuids), to_jsonb(a.v_text), to_jsonb(a.v_texts),
                  to_jsonb(a.v_int), to_jsonb(a.v_ints), to_jsonb(a.v_bool), to_jsonb(a.v_date),
                  to_jsonb(a.v_dates))
END"
    )
}

/// Parameters: `$1` property, `$2` objects, `$3` window start, `$4` heritable, `$5` closed
/// list, `$6` objects created by the changeset.
fn record_sql() -> String {
    let value = value_json("$5");
    format!(
        "WITH w AS (SELECT tstzrange($3, NULL) AS r),
ob AS (
    SELECT object_id, origin_object_id AS origin, valid_period FROM property_origin
    WHERE $4 AND property_id = $1 AND object_id = ANY($2) AND sys_start < now()
    UNION ALL
    SELECT object_id, origin_object_id, valid_period FROM property_origin_history
    WHERE $4 AND property_id = $1 AND object_id = ANY($2) AND sys_start < now() AND sys_end = now()
    UNION ALL
    SELECT o, o, tstzrange(NULL, NULL) FROM unnest($2::uuid[]) AS o WHERE NOT $4
),
oa AS (
    SELECT object_id, origin_object_id AS origin, valid_period FROM property_origin
    WHERE $4 AND property_id = $1 AND object_id = ANY($2)
    UNION ALL
    SELECT o, o, tstzrange(NULL, NULL) FROM unnest($2::uuid[]) AS o WHERE NOT $4
),
ab AS (
    SELECT * FROM property_assignment WHERE property_id = $1 AND sys_start < now()
    UNION ALL
    SELECT object_id, object_type, property_id, kind, v_uuid, v_uuids, v_text, v_texts, v_int,
           v_ints, v_bool, v_date, v_dates, provenance, valid_period, sys_start, changeset_id
    FROM property_assignment_history
    WHERE property_id = $1 AND sys_start < now() AND sys_end = now()
),
aa AS (SELECT * FROM property_assignment WHERE property_id = $1),
vb AS (
    SELECT ob.object_id, ob.valid_period * a.valid_period AS vp, {value} AS v
    FROM ob JOIN ab a ON a.object_id = ob.origin AND a.valid_period && ob.valid_period
),
va AS (
    SELECT oa.object_id, oa.valid_period * a.valid_period AS vp, {value} AS v
    FROM oa JOIN aa a ON a.object_id = oa.origin AND a.valid_period && oa.valid_period
),
ex AS (
    SELECT id, valid_period * w.r AS vp FROM carrier_object, w
    WHERE id = ANY($2) AND NOT id = ANY($6) AND valid_period && w.r
),
pts AS (
    SELECT id AS o, lower(vp) AS p FROM ex
    UNION SELECT id, coalesce(upper(vp), 'infinity') FROM ex
    UNION SELECT object_id, lower(vp) FROM vb
    UNION SELECT object_id, coalesce(upper(vp), 'infinity') FROM vb
    UNION SELECT object_id, lower(vp) FROM va
    UNION SELECT object_id, coalesce(upper(vp), 'infinity') FROM va
),
seg AS (SELECT o, p AS s, lead(p) OVER (PARTITION BY o ORDER BY p) AS e FROM pts),
cmp AS (
    SELECT g.o, vb.v AS before, va.v AS after
    FROM seg g
    JOIN ex ON ex.id = g.o AND ex.vp @> g.s
    LEFT JOIN vb ON vb.object_id = g.o AND vb.vp @> g.s
    LEFT JOIN va ON va.object_id = g.o AND va.vp @> g.s
    WHERE g.e IS NOT NULL AND g.s >= $3
)
INSERT INTO pg_temp.diff_detail (property_key, object_id, before, after)
SELECT DISTINCT (SELECT key FROM property_declaration WHERE id = $1 LIMIT 1), o, before, after
FROM cmp WHERE before IS DISTINCT FROM after"
    )
}

/// Differential of the heirs of origins whose value changed while no origin moved: the
/// timeline of each origin's value before and after, then the heirs over each segment that
/// changed, from the origin index. Parameters: `$1` property, `$2` origins, `$3` window
/// start, `$4` excluded objects, `$5` closed list.
fn value_sql() -> String {
    let value = value_json("$5");
    format!(
        "WITH w AS (SELECT tstzrange($3, NULL) AS r),
ab AS (
    SELECT * FROM property_assignment
    WHERE property_id = $1 AND object_id = ANY($2) AND sys_start < now()
    UNION ALL
    SELECT object_id, object_type, property_id, kind, v_uuid, v_uuids, v_text, v_texts, v_int,
           v_ints, v_bool, v_date, v_dates, provenance, valid_period, sys_start, changeset_id
    FROM property_assignment_history
    WHERE property_id = $1 AND object_id = ANY($2) AND sys_start < now() AND sys_end = now()
),
aa AS (SELECT * FROM property_assignment WHERE property_id = $1 AND object_id = ANY($2)),
vb AS (
    SELECT a.object_id AS origin, a.valid_period * w.r AS vp, {value} AS v
    FROM ab a, w WHERE a.valid_period && w.r
),
va AS (
    SELECT a.object_id AS origin, a.valid_period * w.r AS vp, {value} AS v
    FROM aa a, w WHERE a.valid_period && w.r
),
pts AS (
    SELECT origin, lower(vp) AS p FROM vb
    UNION SELECT origin, coalesce(upper(vp), 'infinity') FROM vb
    UNION SELECT origin, lower(vp) FROM va
    UNION SELECT origin, coalesce(upper(vp), 'infinity') FROM va
),
seg AS (SELECT origin, p AS s, lead(p) OVER (PARTITION BY origin ORDER BY p) AS e FROM pts),
chg AS (
    SELECT g.origin, tstzrange(g.s, nullif(g.e, 'infinity')) AS sp, vb.v AS before, va.v AS after
    FROM seg g
    LEFT JOIN vb ON vb.origin = g.origin AND vb.vp @> g.s
    LEFT JOIN va ON va.origin = g.origin AND va.vp @> g.s
    WHERE g.e IS NOT NULL
)
INSERT INTO pg_temp.diff_detail (property_key, object_id, before, after)
SELECT DISTINCT (SELECT key FROM property_declaration WHERE id = $1 LIMIT 1), r.object_id,
       c.before, c.after
FROM chg c
JOIN property_origin r ON r.property_id = $1 AND r.origin_object_id = c.origin
    AND r.valid_period && c.sp
WHERE c.before IS DISTINCT FROM c.after AND NOT r.object_id = ANY($4)"
    )
}

impl PgTx {
    async fn diff_table(&mut self) -> Result<(), StoreError> {
        sqlx::raw_sql(
            "CREATE TEMP TABLE IF NOT EXISTS diff_detail (
                 property_key text NOT NULL, object_id uuid NOT NULL, before jsonb, after jsonb
             ) ON COMMIT DROP",
        )
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(())
    }

    /// Window start and whether the property is a closed list.
    async fn diff_context(
        &mut self,
        property: Uuid,
        start: Option<OffsetDateTime>,
    ) -> Result<(OffsetDateTime, bool), StoreError> {
        let from: OffsetDateTime = sqlx::query_scalar("SELECT coalesce($1::timestamptz, now())")
            .bind(start)
            .fetch_one(&mut *self.tx)
            .await
            .map_err(store_err)?;
        let closed_list: bool = sqlx::query_scalar(
            "SELECT coalesce(bool_or(value_type = 'enum'), false) FROM property_declaration WHERE id = $1",
        )
        .bind(property)
        .fetch_one(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok((from, closed_list))
    }

    /// Records the differential of the heirs of `origins` (see [`value_sql`]).
    pub(crate) async fn record_value_diff_from(
        &mut self,
        property: Uuid,
        origins: &[Uuid],
        excluded: &[Uuid],
        start: Option<OffsetDateTime>,
    ) -> Result<(), StoreError> {
        self.diff_table().await?;
        if origins.is_empty() {
            return Ok(());
        }
        let (from, closed_list) = self.diff_context(property, start).await?;
        sqlx::query(sqlx::AssertSqlSafe(value_sql()))
            .bind(property)
            .bind(origins)
            .bind(from)
            .bind(excluded)
            .bind(closed_list)
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
        Ok(())
    }

    /// Records the differential of one property over `objects` (module documentation).
    pub(crate) async fn record_diff_from(
        &mut self,
        property: Uuid,
        objects: &[Uuid],
        heritable: bool,
        excluded: &[Uuid],
        start: Option<OffsetDateTime>,
    ) -> Result<(), StoreError> {
        self.diff_table().await?;
        if objects.is_empty() {
            return Ok(());
        }
        let (from, closed_list) = self.diff_context(property, start).await?;
        // Equality joins only: hash and merge joins scale with the volume, whereas the
        // statistics of a property written in this changeset are stale and lead the
        // planner into nested loops over every origin (bench 003-16).
        sqlx::raw_sql("SET LOCAL enable_nestloop = off")
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
        sqlx::query(sqlx::AssertSqlSafe(record_sql()))
            .bind(property)
            .bind(objects)
            .bind(from)
            .bind(heritable)
            .bind(closed_list)
            .bind(excluded)
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
        sqlx::raw_sql("SET LOCAL enable_nestloop TO DEFAULT")
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
        Ok(())
    }

    /// Counts by property and change.
    pub(crate) async fn diff_counts(&mut self) -> Result<Vec<DiffCount>, StoreError> {
        self.diff_table().await?;
        let rows: Vec<(String, Option<String>, Option<String>, i64)> = sqlx::query_as(
            "SELECT property_key, before::text, after::text, count(DISTINCT object_id)
             FROM pg_temp.diff_detail GROUP BY 1, 2, 3 ORDER BY 1, 2 NULLS FIRST, 3 NULLS FIRST",
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(rows
            .into_iter()
            .map(|(property, before, after, count)| DiffCount {
                property,
                before,
                after,
                count: u64::try_from(count).unwrap_or_default(),
            })
            .collect())
    }

    /// First entries, in a stable order.
    pub(crate) async fn diff_first(&mut self, limit: i64) -> Result<Vec<DiffItem>, StoreError> {
        self.diff_table().await?;
        let rows: Vec<(String, Uuid, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT property_key, object_id, before::text, after::text FROM pg_temp.diff_detail
             ORDER BY property_key, object_id, before::text, after::text LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(rows
            .into_iter()
            .map(|(property, object, before, after)| DiffItem {
                property,
                object,
                before,
                after,
            })
            .collect())
    }

    /// Copies the recorded detail to `changeset_diff` (after the changeset row exists).
    pub(crate) async fn store_diff(&mut self, changeset: Uuid) -> Result<(), StoreError> {
        self.diff_table().await?;
        sqlx::query(
            "INSERT INTO changeset_diff (changeset_id, property_key, object_id, before, after)
             SELECT $1, property_key, object_id, before, after FROM pg_temp.diff_detail
             ORDER BY property_key, object_id, before::text, after::text",
        )
        .bind(changeset)
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(())
    }
}

impl PgTx {
    /// Objects of the recorded differential for a property key.
    pub(crate) async fn diff_objects_of(
        &mut self,
        property: &str,
    ) -> Result<Vec<Uuid>, StoreError> {
        self.diff_table().await?;
        sqlx::query_scalar(
            "SELECT DISTINCT object_id FROM pg_temp.diff_detail WHERE property_key = $1 ORDER BY 1",
        )
        .bind(property)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)
    }
}
