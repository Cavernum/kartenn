//! The cache of policy snapshots across databases (REQ-010-16; ADR 0030): two databases
//! served by one process may share a revision, never a policy.

use kartenn_policy::cache::SnapshotCache;
use kartenn_policy::model::Key;
use kartenn_policy::read;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::time::OffsetDateTime;
use sqlx::{Connection, PgConnection, PgPool};

use crate::support::env::{Env, op};

async fn revision(pool: &PgPool) -> u64 {
    read::revision(&mut pool.acquire().await.unwrap())
        .await
        .unwrap()
}

/// Two databases at the same revision, with different policies, read through one cache in
/// one process: each gives its own.
#[kartenn_testing::pg_test]
async fn two_databases_at_one_revision_keep_their_own_policy(pool: PgPool) {
    let master = kartenn_testing::master_options();
    let mut admin = PgConnection::connect_with(&master).await.unwrap();
    let template = kartenn_testing::ensure_template(&mut admin, &kartenn_store_pg::MIGRATOR)
        .await
        .unwrap();
    let name = kartenn_testing::database_name(concat!(module_path!(), "::other"));
    kartenn_testing::clone(&mut admin, &template, &name)
        .await
        .unwrap();
    let other = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(master.clone().database(&name))
        .await
        .unwrap();
    let (a, b) = (Env::new(&pool).await, Env::new(&other).await);
    for (env, key) in [(&a, "file"), (&b, "mail")] {
        env.applied(vec![op(
            "flow_type.create",
            json!({ "key": key, "labels": { "fr": key } }),
        )])
        .await;
    }
    assert_eq!(revision(&pool).await, revision(&other).await);
    let cache = SnapshotCache::new();
    let now = OffsetDateTime::now_utc();
    for (pool, own, foreign) in [
        (&pool, "file", "mail"),
        (&other, "mail", "file"),
        (&pool, "file", "mail"),
    ] {
        let snapshot = cache
            .snapshot(&mut pool.acquire().await.unwrap(), now, None)
            .await
            .unwrap();
        assert!(snapshot.flow_type(&Key::new(own).unwrap()).is_some());
        assert!(snapshot.flow_type(&Key::new(foreign).unwrap()).is_none());
    }
    other.close().await;
    kartenn_testing::drop_database(&mut admin, &name)
        .await
        .unwrap();
}
