//! The counters of the module on the home page (feature `rest`; spec 024, REQ-024-14,
//! 15): the rules of the policy that allow and those that deny, by the predicate of the
//! list of the rules.

use kartenn_service::module::{BoxFuture, Contribution, ModuleRead, StatsContributor, StatsScope};
use kartenn_service::port::StoreError;
use kartenn_store_pg::rbac::readable;
use sqlx::PgConnection;
use time::OffsetDateTime;

use crate::module::RULE;

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
            if !(scope.reads)(RULE) {
                return Ok(Contribution::default());
            }
            let at =
                OffsetDateTime::from_unix_timestamp_nanos(i128::from(read.valid_at().0) * 1_000)
                    .map_err(|e| StoreError(e.to_string()))?;
            let conn = read
                .connection()
                .downcast_mut::<PgConnection>()
                .ok_or_else(|| StoreError("not a PostgreSQL connection".into()))?;
            let sql = format!(
                "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($2))
                 SELECT count(*) FILTER (WHERE x.effect = 'allow'),
                        count(*) FILTER (WHERE x.effect = 'deny')
                   FROM policy.policy_rule x
                  WHERE x.valid_period @> $1 AND ($2::jsonb IS NULL OR {})",
                readable(
                    "x.id",
                    &format!("'{RULE}'"),
                    "(SELECT m.domain_id FROM module_object m
                       WHERE m.id = x.id AND m.valid_period @> $1 LIMIT 1)",
                    "NULL::inet",
                    "$1",
                )
            );
            let (allow, deny): (i64, i64) = sqlx::query_as(sqlx::AssertSqlSafe(sql))
                .bind(at)
                .bind(scope.scope)
                .fetch_one(conn)
                .await
                .map_err(|e| StoreError(e.to_string()))?;
            let n = |v: i64| u64::try_from(v).unwrap_or_default();
            Ok(Contribution {
                objects: Vec::new(),
                counters: vec![("rules_allow", n(allow)), ("rules_deny", n(deny))],
            })
        })
    }
}
