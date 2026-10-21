//! Dated reads of the policy and the cache of snapshots (spec 010, REQ-010-16, 26; task
//! 010-12).

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use kartenn_policy::cache::SnapshotCache;
use kartenn_policy::evaluate::{Endpoint, Requested, Verdict, evaluate};
use kartenn_policy::model::{AxisKey, AxisValue, Key};
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::support::env::{Env, op};

fn days(n: u64) -> Duration {
    Duration::from_secs(n * 86_400)
}

/// The environments `prod` and `dev`, a flow type `admin` and its service `ssh`.
async fn catalogue(env: &Env) {
    env.applied(vec![
        op("environment.create", json!({ "key": "prod" })),
        op("environment.create", json!({ "key": "dev" })),
        op(
            "flow_type.create",
            json!({ "local": 1, "key": "admin", "label": "Administration" }),
        ),
        op(
            "service.create",
            json!({ "key": "ssh", "label": "SSH", "flow_type": { "local": 1 },
                    "entries": [{ "transport": "tcp", "ports": 22 }] }),
        ),
    ])
    .await;
}

async fn flow_type(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("SELECT id FROM policy.policy_flow_type WHERE key = 'admin'")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The clock of the database, between two changesets.
async fn now(pool: &PgPool) -> OffsetDateTime {
    sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(pool)
        .await
        .unwrap()
}

fn endpoint(environment: &str) -> Endpoint {
    Endpoint {
        values: BTreeMap::from([(
            AxisKey::parse("policy.environment").unwrap(),
            BTreeSet::from([AxisValue::new(environment)]),
        )]),
        width: 1,
        whole_domain: false,
    }
}

/// The verdict of `prod` to `dev` for `ssh` in the policy at (`valid_at`, `as_of`).
async fn verdict(
    cache: &SnapshotCache,
    pool: &PgPool,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> Verdict {
    let mut conn = pool.acquire().await.unwrap();
    let snapshot = cache.snapshot(&mut conn, valid_at, as_of).await.unwrap();
    evaluate(
        &snapshot,
        &endpoint("prod"),
        &endpoint("dev"),
        &Key::new("ssh").unwrap(),
        Requested::Service,
        None,
    )
    .verdict
}

/// REQ-010-16: an `allow` rule valid from a later date (« 1er avril ») denies before it,
/// allows after it, and denies at every date as asserted before it was written.
#[kartenn_testing::pg_test]
async fn a_verdict_reads_the_policy_of_its_date(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let before = now(&pool).await;
    let admin = flow_type(&pool).await;
    let april = before + days(30);
    let micros = i64::try_from(april.unix_timestamp_nanos() / 1_000).unwrap();
    let outcome = env
        .submit_from(
            vec![op(
                "rule.create",
                json!({ "key": "r1", "label": "R1", "effect": "allow",
                        "flow_types": [{ "id": admin }],
                        "source": { "policy.environment": ["prod"] },
                        "destination": { "policy.environment": ["dev"] } }),
            )],
            Some(micros),
        )
        .await;
    assert!(
        matches!(outcome, kartenn_service::Outcome::Applied { .. }),
        "{outcome:?}"
    );
    let cache = SnapshotCache::new();
    let march = before + days(1);
    let after = april + days(1);
    assert_eq!(verdict(&cache, &pool, march, None).await, Verdict::Denied);
    assert_eq!(verdict(&cache, &pool, after, None).await, Verdict::Allowed);
    assert_eq!(
        verdict(&cache, &pool, march, Some(before)).await,
        Verdict::Denied
    );
    assert_eq!(
        verdict(&cache, &pool, after, Some(before)).await,
        Verdict::Denied
    );
}

/// The cache: a snapshot is read once over the span in which nothing changes; a new
/// revision reloads the present; a snapshot asserted in the past is never read again
/// (design 010 « Instantané de politique »).
#[kartenn_testing::pg_test]
async fn snapshots_are_kept_until_the_policy_changes(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let before = now(&pool).await;
    let cache = SnapshotCache::new();
    let at = before + Duration::from_secs(3_600);
    let mut conn = pool.acquire().await.unwrap();
    let first = cache.snapshot(&mut conn, at, None).await.unwrap();
    let again = cache
        .snapshot(&mut conn, at + Duration::from_secs(3_600), None)
        .await
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &again));
    assert_eq!(cache.loads(), 1);
    assert_eq!(
        first.revision(),
        u64::try_from(env.revision().await).unwrap()
    );
    let past = cache.snapshot(&mut conn, at, Some(before)).await.unwrap();
    assert_eq!(cache.loads(), 2);
    drop(conn);

    env.applied(vec![op("environment.create", json!({ "key": "test" }))])
        .await;
    let mut conn = pool.acquire().await.unwrap();
    let present = cache.snapshot(&mut conn, at, None).await.unwrap();
    assert_eq!(cache.loads(), 3, "a new revision reloads the present");
    assert!(present.revision() > first.revision());
    let past_again = cache.snapshot(&mut conn, at, Some(before)).await.unwrap();
    assert!(std::sync::Arc::ptr_eq(&past, &past_again));
    assert_eq!(cache.loads(), 3, "the past is never read again");
}
