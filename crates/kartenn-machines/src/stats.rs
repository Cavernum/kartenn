//! The counters of the module on the home page (feature `rest`; spec 024, REQ-024-14,
//! 15): the physical and the virtual machines (virtual or containers) the caller reads,
//! by the predicate of the list of the machines.

use kartenn_service::module::{BoxFuture, Contribution, ModuleRead, StatsContributor, StatsScope};
use kartenn_service::port::StoreError;
use kartenn_store_pg::rbac::readable;
use sqlx::PgConnection;

use crate::module::MACHINE;

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
            if !(scope.reads)(MACHINE) {
                return Ok(Contribution::default());
            }
            let at = crate::pg::time(read.valid_at());
            let conn = read
                .connection()
                .downcast_mut::<PgConnection>()
                .ok_or_else(|| StoreError("not a PostgreSQL connection".into()))?;
            let sql = format!(
                "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($2))
                 SELECT count(*) FILTER (WHERE x.kind = 'physical'),
                        count(*) FILTER (WHERE x.kind IN ('virtual', 'container'))
                   FROM machines.machine x
                  WHERE x.valid_period @> $1 AND ($2::jsonb IS NULL OR {})",
                readable(
                    "x.id",
                    &format!("'{MACHINE}'"),
                    "NULL::uuid",
                    "NULL::inet",
                    "$1"
                )
            );
            let (physical, virtual_): (i64, i64) = sqlx::query_as(sqlx::AssertSqlSafe(sql))
                .bind(at)
                .bind(scope.scope)
                .fetch_one(conn)
                .await
                .map_err(|e| StoreError(e.to_string()))?;
            Ok(Contribution {
                objects: Vec::new(),
                counters: vec![
                    ("physical", u64::try_from(physical).unwrap_or_default()),
                    ("virtual", u64::try_from(virtual_).unwrap_or_default()),
                ],
            })
        })
    }
}
