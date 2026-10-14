//! The changeset pipeline against PostgreSQL (spec 004, task 004-7).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::{PgStore, operations_of};
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

fn domain(local: u32, slug: &str) -> Operation {
    Operation::CreateDomain {
        local: Some(local),
        slug: slug.into(),
    }
}

fn prefix(domain: Ref, value: &str, nature: Nature) -> Operation {
    Operation::CreatePrefix {
        local: None,
        domain,
        prefix: value.into(),
        nature,
        point_to_point: false,
        anycast: false,
    }
}

fn address(domain: Ref, value: &str, usage: AddressUsage) -> Operation {
    Operation::CreateAddress {
        local: None,
        domain,
        address: value.into(),
        usage,
    }
}

async fn count(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn new_domain(pool: &PgPool, author: Uuid, slug: &str) -> Uuid {
    let store = PgStore::new(pool.clone());
    let outcome = submit(
        &store,
        &request(author, vec![domain(1, slug)]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(
        matches!(outcome, Outcome::Applied { .. }),
        "domain creation refused: {outcome:?}"
    );
    sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = $1")
        .bind(slug)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_changeset_creates_objects_through_local_references(pool: PgPool) {
    // REQ-004-4, 6.
    let author = person(&pool).await;
    let store = PgStore::new(pool.clone());
    let ops = vec![
        domain(1, "prod"),
        prefix(Ref::Local(1), "10.0.0.0/8", Nature::Container),
        prefix(Ref::Local(1), "10.1.2.0/24", Nature::Network),
        address(Ref::Local(1), "10.1.2.10", AddressUsage::Host),
        address(Ref::Local(1), "10.1.2.11", AddressUsage::Gateway),
    ];
    let outcome = submit(&store, &request(author, ops.clone()), MAX_OPERATIONS)
        .await
        .unwrap();
    let Outcome::Applied {
        changeset,
        warnings,
    } = outcome
    else {
        panic!("{outcome:?}")
    };
    assert!(warnings.is_empty());
    assert_eq!(count(&pool, "SELECT count(*) FROM prefix").await, 2);
    assert_eq!(count(&pool, "SELECT count(*) FROM address").await, 2);
    let state: String = sqlx::query_scalar("SELECT state FROM changeset WHERE id = $1")
        .bind(changeset)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "applied");
    assert_eq!(
        operations_of(&pool, changeset).await.unwrap(),
        ops,
        "operations are journaled for replay"
    );
    let same_tx: bool = sqlx::query_scalar(
        "SELECT count(DISTINCT sys_start) = 1 FROM (SELECT sys_start FROM prefix UNION ALL SELECT sys_start FROM address) s",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(same_tx, "one system time for the whole changeset");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn one_error_refuses_everything_and_is_journaled(pool: PgPool) {
    // REQ-004-6 acceptance: 100 operations, the 73rd violates a rule.
    let author = person(&pool).await;
    let d = new_domain(&pool, author, "prod").await;
    let store = PgStore::new(pool.clone());
    let mut ops: Vec<Operation> = (0..100)
        .map(|i| {
            prefix(
                Ref::Id(d),
                &format!("10.{}.0.0/16", i + 1),
                Nature::Container,
            )
        })
        .collect();
    ops[72] = prefix(Ref::Id(d), "10.5.0.0/16", Nature::Container);
    let outcome = submit(&store, &request(author, ops), MAX_OPERATIONS)
        .await
        .unwrap();
    let Outcome::Refused {
        changeset,
        violations,
    } = outcome
    else {
        panic!("{outcome:?}")
    };
    assert_eq!(
        violations.iter().map(|v| v.rule.id).collect::<Vec<_>>(),
        vec!["core.prefix.duplicate"]
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM prefix").await,
        0,
        "nothing written"
    );
    let (state, journaled): (String, serde_json::Value) =
        sqlx::query_as("SELECT state, violations FROM changeset WHERE id = $1")
            .bind(changeset)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, "refused");
    assert_eq!(journaled[0]["rule"], "core.prefix.duplicate");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn every_database_violation_is_reported(pool: PgPool) {
    // REQ-004-7: savepoints keep the transaction usable after a refused write.
    let author = person(&pool).await;
    let d = new_domain(&pool, author, "prod").await;
    let store = PgStore::new(pool.clone());
    let ops = vec![
        prefix(Ref::Id(d), "10.0.0.0/8", Nature::Container),
        prefix(Ref::Id(d), "10.0.0.0/8", Nature::Container),
        address(Ref::Id(d), "10.1.1.1", AddressUsage::Host),
        address(Ref::Id(d), "10.1.1.1", AddressUsage::Host),
    ];
    let outcome = submit(&store, &request(author, ops), MAX_OPERATIONS)
        .await
        .unwrap();
    let Outcome::Refused { violations, .. } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(
        violations.iter().map(|v| v.rule.id).collect::<Vec<_>>(),
        vec!["core.prefix.duplicate", "core.address.duplicate"]
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn simulation_writes_nothing(pool: PgPool) {
    // REQ-004-9.
    let author = person(&pool).await;
    let store = PgStore::new(pool.clone());
    let mut req = request(
        author,
        vec![
            domain(1, "prod"),
            prefix(Ref::Local(1), "192.0.2.0/24", Nature::Network),
        ],
    );
    req.simulate = true;
    let outcome = submit(&store, &req, MAX_OPERATIONS).await.unwrap();
    let Outcome::Simulated { violations } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(
        violations.iter().map(|v| v.rule.id).collect::<Vec<_>>(),
        vec!["core.addr.documentation"]
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM addressing_domain").await,
        0
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM changeset").await,
        0,
        "a simulation is not journaled"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn concurrent_changesets_never_combine_into_an_invalid_state(pool: PgPool) {
    // REQ-004-12 acceptance: two simultaneous creations of the same prefix.
    let author = person(&pool).await;
    let d = new_domain(&pool, author, "prod").await;
    let store = PgStore::new(pool.clone());
    let req = request(
        author,
        vec![prefix(Ref::Id(d), "10.0.0.0/8", Nature::Container)],
    );
    let (a, b) = tokio::join!(
        submit(&store, &req, MAX_OPERATIONS),
        submit(&store, &req, MAX_OPERATIONS)
    );
    let applied = [a.unwrap(), b.unwrap()]
        .iter()
        .filter(|o| matches!(o, Outcome::Applied { .. }))
        .count();
    assert_eq!(applied, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM prefix").await, 1);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn closing_ends_the_object_and_refuses_a_missing_one(pool: PgPool) {
    // REQ-004-15.
    let author = person(&pool).await;
    let d = new_domain(&pool, author, "prod").await;
    let store = PgStore::new(pool.clone());
    let create = request(
        author,
        vec![prefix(Ref::Id(d), "10.0.0.0/8", Nature::Container)],
    );
    submit(&store, &create, MAX_OPERATIONS).await.unwrap();
    let p: Uuid = sqlx::query_scalar("SELECT id FROM prefix")
        .fetch_one(&pool)
        .await
        .unwrap();
    let close = request(
        author,
        vec![Operation::Close {
            kind: ObjectKind::Prefix,
            id: Ref::Id(p),
            expected: None,
        }],
    );
    let outcome = submit(&store, &close, MAX_OPERATIONS).await.unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM prefix WHERE upper_inf(valid_period)"
        )
        .await,
        0
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM prefix_history").await,
        1,
        "the previous version is history"
    );
    let missing = request(
        author,
        vec![Operation::Close {
            kind: ObjectKind::Prefix,
            id: Ref::Id(Uuid::now_v7()),
            expected: None,
        }],
    );
    let outcome = submit(&store, &missing, MAX_OPERATIONS).await.unwrap();
    let Outcome::Refused { violations, .. } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(violations[0].rule.id, "changeset.operation.no_version");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn public_space_is_unique_across_domains(pool: PgPool) {
    // REQ-002-32 (application rule and last line of defence). REQ-002-46 cannot be met
    // together with REQ-002-25 and REQ-002-32: see specs/002-noyau-adressage/implementation.md.
    let author = person(&pool).await;
    let (d1, d2) = (
        new_domain(&pool, author, "one").await,
        new_domain(&pool, author, "two").await,
    );
    let store = PgStore::new(pool.clone());
    let ops = vec![
        prefix(Ref::Id(d1), "45.0.0.0/24", Nature::Network),
        address(Ref::Id(d1), "45.0.0.53", AddressUsage::Anycast),
        address(Ref::Id(d1), "45.0.0.10", AddressUsage::Host),
    ];
    let outcome = submit(&store, &request(author, ops), MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let dup_network = request(
        author,
        vec![prefix(Ref::Id(d2), "45.0.0.0/25", Nature::Network)],
    );
    let outcome = submit(&store, &dup_network, MAX_OPERATIONS).await.unwrap();
    let Outcome::Refused { violations, .. } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(violations[0].rule.id, "core.public.overlap");
    let private = request(
        author,
        vec![prefix(Ref::Id(d2), "10.0.0.0/24", Nature::Network)],
    );
    let outcome = submit(&store, &private, MAX_OPERATIONS).await.unwrap();
    assert!(
        matches!(outcome, Outcome::Applied { .. }),
        "private space is per domain: {outcome:?}"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn networks_are_leaves_and_addresses_live_in_networks(pool: PgPool) {
    // REQ-002-16, 25, 29.
    let author = person(&pool).await;
    let d = new_domain(&pool, author, "prod").await;
    let store = PgStore::new(pool.clone());
    let base = vec![
        prefix(Ref::Id(d), "10.0.0.0/8", Nature::Container),
        prefix(Ref::Id(d), "10.1.2.0/24", Nature::Network),
        address(Ref::Id(d), "10.1.2.10", AddressUsage::Host),
    ];
    assert!(matches!(
        submit(&store, &request(author, base), MAX_OPERATIONS)
            .await
            .unwrap(),
        Outcome::Applied { .. }
    ));
    let ids = |o: &Outcome| match o {
        Outcome::Refused { violations, .. } => {
            violations.iter().map(|v| v.rule.id).collect::<Vec<_>>()
        }
        other => panic!("{other:?}"),
    };
    let nested = submit(
        &store,
        &request(
            author,
            vec![prefix(Ref::Id(d), "10.1.2.0/25", Nature::Network)],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert_eq!(
        ids(&nested),
        vec!["core.prefix.network_not_leaf"],
        "inside a network"
    );
    let over = submit(
        &store,
        &request(
            author,
            vec![prefix(Ref::Id(d), "10.1.0.0/16", Nature::Network)],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert_eq!(
        ids(&over),
        vec!["core.prefix.network_not_leaf"],
        "a network over a network"
    );
    let container = submit(
        &store,
        &request(
            author,
            vec![prefix(Ref::Id(d), "10.1.0.0/16", Nature::Container)],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(
        matches!(container, Outcome::Applied { .. }),
        "a container may be inserted above: {container:?}"
    );
    let orphan = submit(
        &store,
        &request(
            author,
            vec![address(Ref::Id(d), "10.9.9.9", AddressUsage::Host)],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert_eq!(ids(&orphan), vec!["core.address.outside_network"]);
    let broadcast = submit(
        &store,
        &request(
            author,
            vec![address(Ref::Id(d), "10.1.2.255", AddressUsage::Host)],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert_eq!(ids(&broadcast), vec!["core.address.not_assignable"]);
    let net: Uuid = sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = '10.1.2.0/24'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let close = request(
        author,
        vec![Operation::Close {
            kind: ObjectKind::Prefix,
            id: Ref::Id(net),
            expected: None,
        }],
    );
    let closing = submit(&store, &close, MAX_OPERATIONS).await.unwrap();
    assert_eq!(
        ids(&closing),
        vec!["core.address.outside_network"],
        "closing a network that holds addresses"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn idempotency_keys_replay_or_conflict(pool: PgPool) {
    // REQ-004-10.
    let author = person(&pool).await;
    let store = PgStore::new(pool.clone());
    let mut req = request(author, vec![domain(1, "prod")]);
    req.idempotency_key = Some("key-1".into());
    let Outcome::Applied { changeset, .. } = submit(&store, &req, MAX_OPERATIONS).await.unwrap()
    else {
        panic!()
    };
    let again = submit(&store, &req, MAX_OPERATIONS).await.unwrap();
    assert!(
        matches!(again, Outcome::Replayed { changeset: c, ref state } if c == changeset && state == "applied"),
        "{again:?}"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM addressing_domain").await,
        1,
        "not applied twice"
    );
    let mut other = request(author, vec![domain(1, "other")]);
    other.idempotency_key = Some("key-1".into());
    assert!(matches!(
        submit(&store, &other, MAX_OPERATIONS).await,
        Err(kartenn_service::RequestError::IdempotencyConflict)
    ));
    let mut parallel = request(author, vec![domain(1, "par")]);
    parallel.idempotency_key = Some("key-2".into());
    let (a, b) = tokio::join!(
        submit(&store, &parallel, MAX_OPERATIONS),
        submit(&store, &parallel, MAX_OPERATIONS)
    );
    let outcomes = [a.unwrap(), b.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| matches!(o, Outcome::Applied { .. }))
            .count(),
        1,
        "parallel same key: one applies"
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| matches!(o, Outcome::Replayed { .. }))
            .count(),
        1,
        "the other waits and replays"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_stale_expected_version_is_a_conflict(pool: PgPool) {
    // REQ-004-11.
    let author = person(&pool).await;
    let d = new_domain(&pool, author, "prod").await;
    let store = PgStore::new(pool.clone());
    let create = request(
        author,
        vec![prefix(Ref::Id(d), "10.0.0.0/8", Nature::Container)],
    );
    let Outcome::Applied { changeset, .. } = submit(&store, &create, MAX_OPERATIONS).await.unwrap()
    else {
        panic!()
    };
    let p: Uuid = sqlx::query_scalar("SELECT id FROM prefix")
        .fetch_one(&pool)
        .await
        .unwrap();
    let stale = request(
        author,
        vec![Operation::Close {
            kind: ObjectKind::Prefix,
            id: Ref::Id(p),
            expected: Some(Uuid::now_v7()),
        }],
    );
    let Outcome::Refused { violations, .. } = submit(&store, &stale, MAX_OPERATIONS).await.unwrap()
    else {
        panic!()
    };
    assert_eq!(violations[0].rule.id, "changeset.operation.conflict");
    let fresh = request(
        author,
        vec![Operation::Close {
            kind: ObjectKind::Prefix,
            id: Ref::Id(p),
            expected: Some(changeset),
        }],
    );
    assert!(matches!(
        submit(&store, &fresh, MAX_OPERATIONS).await.unwrap(),
        Outcome::Applied { .. }
    ));
}
