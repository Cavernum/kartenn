//! Events of applied changesets and their relay (REQ-004-36, 37; ADR 0018 amended).

use kartenn_core::objects::Nature;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::outbox::{Event, RelayError, relay};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

async fn person(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'tester') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

fn request(author: Uuid, operations: Vec<Operation>) -> ChangesetRequest {
    ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
    }
}

async fn drain(pool: &PgPool, consumer: &str) -> Vec<Event> {
    let mut got = Vec::new();
    relay(pool, consumer, 100, |e| {
        got.push(e);
        async { Ok::<(), std::convert::Infallible>(()) }
    })
    .await
    .unwrap();
    got
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn applied_changesets_emit_one_cloudevent_each(pool: PgPool) {
    let author = person(&pool).await;
    let store = PgStore::new(pool.clone());
    let req = request(
        author,
        vec![
            Operation::CreateDomain {
                local: Some(1),
                slug: "prod".into(),
            },
            Operation::CreatePrefix {
                local: None,
                domain: Ref::Local(1),
                prefix: "10.0.0.0/8".into(),
                nature: Nature::Container,
                point_to_point: false,
            },
        ],
    );
    let Outcome::Applied { changeset, .. } = submit(&store, &req, MAX_OPERATIONS).await.unwrap()
    else {
        panic!()
    };
    let refused = request(
        author,
        vec![Operation::CreateDomain {
            local: None,
            slug: "Bad".into(),
        }],
    );
    submit(&store, &refused, MAX_OPERATIONS).await.unwrap();
    let events = drain(&pool, "test").await;
    assert_eq!(events.len(), 1, "refused changesets emit nothing");
    let e = &events[0];
    assert_eq!(e.changeset, changeset);
    assert_eq!(e.payload["specversion"], "1.0");
    assert_eq!(e.payload["id"], changeset.to_string());
    assert_eq!(e.payload["type"], "org.kartenn.changeset.applied");
    assert_eq!(e.payload["data"]["objects"].as_array().unwrap().len(), 2);
    assert!(
        drain(&pool, "test").await.is_empty(),
        "delivered once per consumer"
    );
    assert_eq!(
        drain(&pool, "other").await.len(),
        1,
        "each consumer gets every event"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_running_transaction_is_never_skipped(pool: PgPool) {
    // The scenario that defeated a single last-delivered cursor.
    let author = person(&pool).await;
    let cs = |pool: PgPool| async move {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO changeset (id, author_id, motive, state, kind, default_period)
             VALUES (gen_random_uuid(), $1, 'm', 'applied', 'direct', tstzrange(now(), NULL)) RETURNING id",
        )
        .bind(author)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let (c1, c2) = (cs(pool.clone()).await, cs(pool.clone()).await);
    let mut long = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO outbox (changeset_id, payload) VALUES ($1, '{}')")
        .bind(c1)
        .execute(&mut *long)
        .await
        .unwrap();
    sqlx::query("INSERT INTO outbox (changeset_id, payload) VALUES ($1, '{}')")
        .bind(c2)
        .execute(&pool)
        .await
        .unwrap();
    // Without domain in common, the later event does not wait for the running
    // transaction (ordering is per domain, ADR 0018) …
    assert_eq!(
        drain(&pool, "c")
            .await
            .iter()
            .map(|e| e.changeset)
            .collect::<Vec<_>>(),
        vec![c2],
        "committed events are not held back"
    );
    long.commit().await.unwrap();
    // … and the earlier one, committed afterwards, is not skipped.
    assert_eq!(
        drain(&pool, "c")
            .await
            .iter()
            .map(|e| e.changeset)
            .collect::<Vec<_>>(),
        vec![c1],
        "delivered once committed"
    );
    assert!(drain(&pool, "c").await.is_empty(), "each event once");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_refused_event_is_delivered_again(pool: PgPool) {
    let author = person(&pool).await;
    let store = PgStore::new(pool.clone());
    submit(
        &store,
        &request(
            author,
            vec![Operation::CreateDomain {
                local: None,
                slug: "a".into(),
            }],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let failed = relay(&pool, "flaky", 10, |_| async { Err::<(), &str>("down") }).await;
    assert!(matches!(failed, Err(RelayError::Sink { .. })));
    assert_eq!(drain(&pool, "flaky").await.len(), 1, "at least once");
}
