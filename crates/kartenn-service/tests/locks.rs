//! Domain locks taken by the pipeline (ADR 0015): every write into a domain, closure of
//! the domain included, waits for the domain's lock.

use std::time::Duration;

use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn closing_a_domain_waits_for_its_lock(pool: PgPool) {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'tester') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let store = PgStore::new(pool.clone());
    let request = |operations| ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
    };
    let created = submit(
        &store,
        &Unrestricted,
        &request(vec![Operation::CreateDomain {
            local: None,
            slug: "prod".into(),
        }]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(created, Outcome::Applied { .. }));
    let domain: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Another writer holds the domain's lock (same key as the store, ADR 0015).
    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(domain.as_u64_pair().0.cast_signed())
        .execute(&mut *holder)
        .await
        .unwrap();

    let close = request(vec![Operation::Close {
        kind: ObjectKind::Domain,
        id: Ref::Id(domain),
        expected: None,
        cascade: false,
    }]);
    let pending = tokio::spawn({
        let store = store.clone();
        async move {
            submit(&store, &Unrestricted, &close, MAX_OPERATIONS)
                .await
                .unwrap()
        }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!pending.is_finished(), "the closure waits for the lock");
    holder.rollback().await.unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(5), pending)
        .await
        .expect("the closure proceeds once the lock is released")
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}
