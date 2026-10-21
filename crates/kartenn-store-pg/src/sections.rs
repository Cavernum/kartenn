//! Sections of the differential contributed by the modules (spec 008, REQ-008-26; ADR
//! 0026, clause 13): recorded in a temporary table of the transaction, copied to
//! `changeset_diff_section` with the journal of an applied changeset, and read back by
//! pages.

use kartenn_core::period::{Instant, Period};
use kartenn_service::StoreError;
use kartenn_service::module::{DiffRow, DiffSection};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::write::PgTx;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

fn instant(i: Instant) -> Result<OffsetDateTime, StoreError> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).map_err(store_err)
}

fn micros(t: OffsetDateTime) -> Result<Instant, StoreError> {
    i64::try_from(t.unix_timestamp_nanos() / 1_000)
        .map(Instant)
        .map_err(store_err)
}

/// A stored row: its position, then the row.
type Stored = (
    i64,
    Uuid,
    String,
    Option<serde_json::Value>,
    Option<serde_json::Value>,
    Option<OffsetDateTime>,
    Option<OffsetDateTime>,
    bool,
);

fn row((_, object, kind, before, after, start, end, dated): Stored) -> Result<DiffRow, StoreError> {
    let period = match (dated, start) {
        (true, Some(start)) => {
            Some(Period::new(micros(start)?, end.map(micros).transpose()?).map_err(store_err)?)
        }
        _ => None,
    };
    Ok(DiffRow {
        object,
        kind,
        before,
        after,
        period,
    })
}

impl PgTx {
    async fn section_table(&mut self) -> Result<(), StoreError> {
        sqlx::raw_sql(
            "CREATE TEMP TABLE IF NOT EXISTS diff_section (
                 section text NOT NULL, seq bigint GENERATED ALWAYS AS IDENTITY,
                 object_id uuid NOT NULL, kind text NOT NULL, before jsonb, after jsonb,
                 period tstzrange
             ) ON COMMIT DROP",
        )
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(())
    }

    /// Records the rows of `section`, in their order.
    pub(crate) async fn record_section_rows(
        &mut self,
        section: &DiffSection,
    ) -> Result<(), StoreError> {
        self.section_table().await?;
        if section.rows.is_empty() {
            return Ok(());
        }
        let mut objects = Vec::with_capacity(section.rows.len());
        let mut kinds = Vec::with_capacity(section.rows.len());
        let mut befores = Vec::with_capacity(section.rows.len());
        let mut afters = Vec::with_capacity(section.rows.len());
        let mut starts: Vec<Option<OffsetDateTime>> = Vec::with_capacity(section.rows.len());
        let mut ends: Vec<Option<OffsetDateTime>> = Vec::with_capacity(section.rows.len());
        for r in &section.rows {
            objects.push(r.object);
            kinds.push(r.kind.clone());
            befores.push(r.before.clone());
            afters.push(r.after.clone());
            starts.push(r.period.map(|p| instant(p.start())).transpose()?);
            ends.push(r.period.and_then(Period::end).map(instant).transpose()?);
        }
        sqlx::query(
            "INSERT INTO pg_temp.diff_section (section, object_id, kind, before, after, period)
             SELECT $1, o, k, b, a, CASE WHEN s IS NULL THEN NULL ELSE tstzrange(s, e) END
             FROM unnest($2::uuid[], $3::text[], $4::jsonb[], $5::jsonb[],
                         $6::timestamptz[], $7::timestamptz[])
                  WITH ORDINALITY AS r(o, k, b, a, s, e, n)
             ORDER BY n",
        )
        .bind(&section.name)
        .bind(&objects)
        .bind(&kinds)
        .bind(&befores)
        .bind(&afters)
        .bind(&starts)
        .bind(&ends)
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(())
    }

    /// Copies the recorded sections to `changeset_diff_section` (after the changeset row
    /// exists).
    pub(crate) async fn store_sections(&mut self, changeset: Uuid) -> Result<(), StoreError> {
        self.section_table().await?;
        sqlx::query(
            "INSERT INTO changeset_diff_section
                 (changeset_id, section, object_id, kind, before, after, period)
             SELECT $1, section, object_id, kind, before, after, period
             FROM pg_temp.diff_section ORDER BY seq",
        )
        .bind(changeset)
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(())
    }
}

/// The sections of an applied changeset, with their number of rows, by name.
///
/// # Errors
///
/// Database errors.
pub async fn section_counts(
    pool: &PgPool,
    changeset: Uuid,
) -> Result<Vec<(String, i64)>, StoreError> {
    sqlx::query_as(
        "SELECT section, count(*) FROM changeset_diff_section WHERE changeset_id = $1
         GROUP BY section ORDER BY section",
    )
    .bind(changeset)
    .fetch_all(pool)
    .await
    .map_err(store_err)
}

/// Up to `limit` rows of the section `section` of `changeset` after the position `after`,
/// with their positions, in the order the module gave.
///
/// # Errors
///
/// Database errors.
pub async fn section_page(
    pool: &PgPool,
    changeset: Uuid,
    section: &str,
    after: Option<i64>,
    limit: i64,
) -> Result<Vec<(i64, DiffRow)>, StoreError> {
    let rows: Vec<Stored> = sqlx::query_as(
        "SELECT seq, object_id, kind, before, after, lower(period), upper(period),
                period IS NOT NULL
         FROM changeset_diff_section
         WHERE changeset_id = $1 AND section = $2 AND seq > coalesce($3, 0)
         ORDER BY seq LIMIT $4",
    )
    .bind(changeset)
    .bind(section)
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    rows.into_iter()
        .map(|stored| {
            let seq = stored.0;
            row(stored).map(|r| (seq, r))
        })
        .collect()
}
