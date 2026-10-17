//! Retention of the audit journal by criticality (spec 012, REQ-012-16; 012/L22): whole
//! months are dropped once past their retention, only if every event of the month is
//! sealed and delivered to every output; each purge is recorded so that the verification
//! tells it from a deletion. Run with the owner of the schema (the application role may
//! not delete).

use std::time::Duration;

use kartenn_service::StoreError;
use sqlx::PgPool;
use sqlx::types::time::Date;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Retention by criticality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    /// Ordinary reads.
    pub low: Duration,
    /// Writes and operation.
    pub normal: Duration,
    /// Refusals, administration, sensitive reads.
    pub high: Duration,
}

impl Default for Retention {
    /// 90 days, 1 year, 3 years (012/L22).
    fn default() -> Self {
        let day = Duration::from_hours(24);
        Self {
            low: day * 90,
            normal: day * 365,
            high: day * 1095,
        }
    }
}

/// What a purge did for one month of one criticality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Purged {
    /// Criticality.
    pub criticality: String,
    /// First day of the month.
    pub month: Date,
    /// Whether it was dropped; `false` when events were not sealed or delivered yet.
    pub dropped: bool,
}

/// Drops the months past retention whose events are all sealed and delivered to each of
/// `sinks`; reports the months kept back.
///
/// # Errors
///
/// Database errors.
pub async fn purge(
    pool: &PgPool,
    retention: &Retention,
    sinks: &[&str],
) -> Result<Vec<Purged>, StoreError> {
    let mut done = Vec::new();
    for (criticality, keep) in [
        ("low", retention.low),
        ("normal", retention.normal),
        ("high", retention.high),
    ] {
        let seconds = keep.as_secs_f64();
        // Months entirely before now - retention, that still hold events.
        let months: Vec<Date> = sqlx::query_scalar(
            "SELECT DISTINCT date_trunc('month', recorded_at)::date FROM audit.event
             WHERE criticality = $1
               AND date_trunc('month', recorded_at) + interval '1 month'
                   <= now() - make_interval(secs => $2)
             ORDER BY 1",
        )
        .bind(criticality)
        .bind(seconds)
        .fetch_all(pool)
        .await
        .map_err(store_err)?;
        for month in months {
            let pending: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM audit.event e
                 WHERE e.criticality = $1
                   AND e.recorded_at >= $2 AND e.recorded_at < $2 + interval '1 month'
                   AND (NOT EXISTS (SELECT 1 FROM audit.sealed s WHERE s.event_seq = e.seq)
                        OR EXISTS (SELECT 1 FROM unnest($3::text[]) AS k(sink)
                                   WHERE NOT EXISTS (SELECT 1 FROM audit.delivery d
                                                     WHERE d.sink = k.sink AND d.event_seq = e.seq)))",
            )
            .bind(criticality)
            .bind(month)
            .bind(sinks)
            .fetch_one(pool)
            .await
            .map_err(store_err)?;
            let dropped = pending == 0;
            if dropped {
                drop_month(pool, criticality, month).await?;
            }
            done.push(Purged {
                criticality: criticality.to_owned(),
                month,
                dropped,
            });
        }
    }
    Ok(done)
}

/// Drops the events and deliveries of a month (its partitions, or its rows in the default
/// partitions), and records the purge, in one transaction.
async fn drop_month(pool: &PgPool, criticality: &str, month: Date) -> Result<(), StoreError> {
    let mut tx = pool.begin().await.map_err(store_err)?;
    let suffix = format!(
        "{}_{:04}{:02}",
        criticality,
        month.year(),
        u8::from(month.month())
    );
    for table in ["event", "delivery"] {
        let partition = format!("{table}_{suffix}");
        let exists: bool = sqlx::query_scalar("SELECT to_regclass('audit.' || $1) IS NOT NULL")
            .bind(&partition)
            .fetch_one(&mut *tx)
            .await
            .map_err(store_err)?;
        let sql = if exists {
            format!("DROP TABLE audit.{partition}")
        } else {
            format!(
                "DELETE FROM audit.{table}_{criticality}_default
                 WHERE recorded_at >= '{month}' AND recorded_at < date '{month}' + interval '1 month'"
            )
        };
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&mut *tx)
            .await
            .map_err(store_err)?;
    }
    sqlx::query(
        "INSERT INTO audit.purge (criticality, month) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(criticality)
    .bind(month)
    .execute(&mut *tx)
    .await
    .map_err(store_err)?;
    tx.commit().await.map_err(store_err)
}
