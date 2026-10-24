//! The counters of the module on the home page (feature `rest`; spec 024, REQ-024-14,
//! 15), under the predicate of the list of the flows: `flow.read` on their entity, or
//! their requester (REQ-011-17). The flows themselves are counted here, in place of the
//! count of the core, which knows `flow.read` only; with them, the flows whose evaluation
//! is not compliant and those whose waiver ends within 30 days.

use kartenn_service::module::{BoxFuture, Contribution, ModuleRead, StatsContributor, StatsScope};
use kartenn_service::port::StoreError;
use kartenn_store_pg::rbac::readable;
use sqlx::PgConnection;
use time::OffsetDateTime;

use crate::module::FLOW;

/// The contributor of the module.
#[derive(Debug)]
pub struct Stats;

impl StatsContributor for Stats {
    fn contribute<'a>(
        &'a self,
        read: &'a mut dyn ModuleRead,
        scope: &'a StatsScope<'a>,
    ) -> BoxFuture<'a, Result<Contribution, StoreError>> {
        Box::pin(async move {
            let at =
                OffsetDateTime::from_unix_timestamp_nanos(i128::from(read.valid_at().0) * 1_000)
                    .map_err(|e| StoreError(e.to_string()))?;
            let conn = read
                .connection()
                .downcast_mut::<PgConnection>()
                .ok_or_else(|| StoreError("not a PostgreSQL connection".into()))?;
            let reads = (scope.reads)(FLOW);
            let sql = format!(
                "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($2)),
                      f AS (SELECT x.id FROM flow.flow x
                             WHERE x.valid_period @> $1
                               AND (($3 AND ($2::jsonb IS NULL OR {}))
                                    OR x.requester_id = $4))
                 SELECT (SELECT count(*) FROM f),
                        (SELECT count(*) FROM f JOIN flow.flow_summary m
                             ON m.flow_id = f.id AND m.valid_period @> $1
                          WHERE m.outcome = 'noncompliant'),
                        (SELECT count(*) FROM f JOIN flow.flow_waiver w
                             ON w.flow_id = f.id AND w.valid_period @> $1
                          WHERE upper(w.valid_period) < $1 + interval '30 days')",
                readable(
                    "x.id",
                    &format!("'{FLOW}'"),
                    "(SELECT m.domain_id FROM module_object m
                       WHERE m.id = x.id AND m.valid_period @> $1 LIMIT 1)",
                    "NULL::inet",
                    "$1",
                )
            );
            let (flows, noncompliant, expiring): (i64, i64, i64) =
                sqlx::query_as(sqlx::AssertSqlSafe(sql))
                    .bind(at)
                    .bind(scope.scope)
                    .bind(reads)
                    .bind(scope.identity)
                    .fetch_one(conn)
                    .await
                    .map_err(|e| StoreError(e.to_string()))?;
            if !reads && flows == 0 {
                return Ok(Contribution::default());
            }
            let n = |v: i64| u64::try_from(v).unwrap_or_default();
            Ok(Contribution {
                objects: vec![(FLOW, n(flows))],
                counters: vec![
                    ("noncompliant", n(noncompliant)),
                    ("waivers_expiring", n(expiring)),
                ],
            })
        })
    }
}
