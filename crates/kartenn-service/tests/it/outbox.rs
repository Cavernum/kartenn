//! Events of applied changesets and their relay (REQ-004-36, 37; ADR 0018 amended).

use kartenn_core::objects::Nature;
use kartenn_service::authz::Unrestricted;
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

#[kartenn_testing::pg_test]
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
                anycast: false,
            },
        ],
    );
    let Outcome::Applied { changeset, .. } = submit(&store, &Unrestricted, &req, MAX_OPERATIONS)
        .await
        .unwrap()
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
    submit(&store, &Unrestricted, &refused, MAX_OPERATIONS)
        .await
        .unwrap();
    let events = drain(&pool, "test").await;
    assert_eq!(events.len(), 1, "refused changesets emit nothing");
    let e = &events[0];
    assert_eq!(e.changeset, Some(changeset));
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

#[kartenn_testing::pg_test]
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
            .filter_map(|e| e.changeset)
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
            .filter_map(|e| e.changeset)
            .collect::<Vec<_>>(),
        vec![c1],
        "delivered once committed"
    );
    assert!(drain(&pool, "c").await.is_empty(), "each event once");
}

#[kartenn_testing::pg_test]
async fn a_refused_event_is_delivered_again(pool: PgPool) {
    let author = person(&pool).await;
    let store = PgStore::new(pool.clone());
    submit(
        &store,
        &Unrestricted,
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

#[kartenn_testing::pg_test]
async fn events_of_a_domain_follow_the_order_of_its_changesets(pool: PgPool) {
    // REQ-004-37, ADR 0018 amended: within a domain, the lock of ADR 0015 orders the
    // events; a changeset waiting for a running one of its domain comes after it.
    let author = person(&pool).await;
    let store = PgStore::new(pool.clone());
    let Outcome::Applied { .. } = submit(
        &store,
        &Unrestricted,
        &request(
            author,
            vec![Operation::CreateDomain {
                local: None,
                slug: "prod".into(),
            }],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap() else {
        panic!()
    };
    let domain: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain")
        .fetch_one(&pool)
        .await
        .unwrap();
    drain(&pool, "c").await;
    let earlier: Uuid = sqlx::query_scalar(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period)
         VALUES (gen_random_uuid(), $1, 'm', 'applied', 'direct', tstzrange(now(), NULL)) RETURNING id",
    )
    .bind(author)
    .fetch_one(&pool)
    .await
    .unwrap();
    // A changeset of the domain, still running: it holds the domain lock (key: the high 64
    // bits of the domain's UUID) and has written its event.
    let mut running = pool.begin().await.unwrap();
    sqlx::query(
        "SELECT pg_advisory_xact_lock(('x' || left(replace($1::text, '-', ''), 16))::bit(64)::bigint)",
    )
    .bind(domain)
    .execute(&mut *running)
    .await
    .unwrap();
    sqlx::query("INSERT INTO outbox (changeset_id, domain_ids, payload) VALUES ($1, $2, '{}')")
        .bind(earlier)
        .bind(vec![domain])
        .execute(&mut *running)
        .await
        .unwrap();
    let later = tokio::spawn({
        let store = store.clone();
        async move {
            submit(
                &store,
                &Unrestricted,
                &request(
                    author,
                    vec![Operation::CreatePrefix {
                        local: None,
                        domain: Ref::Id(domain),
                        prefix: "10.0.0.0/8".into(),
                        nature: Nature::Container,
                        point_to_point: false,
                        anycast: false,
                    }],
                ),
                MAX_OPERATIONS,
            )
            .await
            .unwrap()
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(!later.is_finished(), "waits for the running changeset");
    assert!(drain(&pool, "c").await.is_empty(), "nothing committed yet");
    running.commit().await.unwrap();
    let Outcome::Applied { changeset, .. } = later.await.unwrap() else {
        panic!()
    };
    assert_eq!(
        drain(&pool, "c")
            .await
            .iter()
            .filter_map(|e| e.changeset)
            .collect::<Vec<_>>(),
        vec![earlier, changeset]
    );
}

#[kartenn_testing::pg_test]
async fn two_changesets_under_one_module_lock_are_delivered_in_order(pool: PgPool) {
    // REQ-016-14: no domain in common, but one lock key of module operations.
    let store = crate::support::dummy_module::store(&pool).await;
    let author = person(&pool).await;
    let earlier: Uuid = sqlx::query_scalar(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period)
         VALUES (gen_random_uuid(), $1, 'm', 'applied', 'direct', tstzrange(now(), NULL)) RETURNING id",
    )
    .bind(author)
    .fetch_one(&pool)
    .await
    .unwrap();
    // A changeset under the key `policy`, still running: it holds the named lock (class
    // "kark", as the store takes it) and has written its event.
    let mut running = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext('policy'))")
        .bind(0x6b61_726b_i32)
        .execute(&mut *running)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO outbox (changeset_id, order_keys, payload) VALUES ($1, '{policy}', '{}')",
    )
    .bind(earlier)
    .execute(&mut *running)
    .await
    .unwrap();
    let later = tokio::spawn({
        let store = store.clone();
        async move {
            let op = crate::support::dummy_module::create_thing(
                serde_json::json!({"label": "x", "keys": ["policy"]}),
            );
            submit(
                &store,
                &Unrestricted,
                &request(author, vec![op]),
                MAX_OPERATIONS,
            )
            .await
            .unwrap()
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(!later.is_finished(), "waits for the running changeset");
    assert!(drain(&pool, "k").await.is_empty(), "nothing committed yet");
    running.commit().await.unwrap();
    let Outcome::Applied { changeset, .. } = later.await.unwrap() else {
        panic!()
    };
    let events = drain(&pool, "k").await;
    assert_eq!(
        events
            .iter()
            .filter_map(|e| e.changeset)
            .collect::<Vec<_>>(),
        vec![earlier, changeset]
    );
    let event = &events[1];
    assert_eq!(event.order_keys, ["policy"]);
    assert!(event.domains.is_empty());
    // The form of a v0.1 event is unchanged: the keys are not in its payload.
    let data = event.payload["data"].as_object().unwrap();
    assert!(
        !event
            .payload
            .as_object()
            .unwrap()
            .contains_key("order_keys")
    );
    assert!(!data.contains_key("order_keys"));
    assert_eq!(event.payload["type"], "org.kartenn.changeset.applied");
}

#[kartenn_testing::pg_test]
async fn the_ordering_keys_of_a_changeset_name_its_domains(pool: PgPool) {
    let author = person(&pool).await;
    let store = PgStore::new(pool.clone());
    let Outcome::Applied { .. } = submit(
        &store,
        &Unrestricted,
        &request(
            author,
            vec![
                Operation::CreateDomain {
                    local: Some(1),
                    slug: "keys".into(),
                },
                Operation::CreatePrefix {
                    local: None,
                    domain: Ref::Local(1),
                    prefix: "10.0.0.0/8".into(),
                    nature: Nature::Container,
                    point_to_point: false,
                    anycast: false,
                },
            ],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap() else {
        panic!()
    };
    let events = drain(&pool, "d").await;
    let event = &events[0];
    assert_eq!(event.domains.len(), 1, "the prefix writes in its domain");
    assert_eq!(event.order_keys, [format!("domain:{}", event.domains[0])]);
}
