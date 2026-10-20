//! Operations of a module in the changeset pipeline (spec 008, REQ-008-4, 27; task 008-3).

#[path = "support/dummy_module.rs"]
mod dummy;

use kartenn_core::objects::Nature;
use kartenn_service::authz::{Grants, Unrestricted};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, RequestError, submit};
use kartenn_store_pg::write::PgStore;
use serde_json::json;
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

async fn count(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

fn rules(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        other => panic!("not refused: {other:?}"),
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_mixed_changeset_is_atomic(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let mixed = request(
        author,
        vec![
            domain(1, "lab"),
            Operation::CreatePrefix {
                local: Some(2),
                domain: Ref::Local(1),
                prefix: "10.0.0.0/8".into(),
                nature: Nature::Container,
                point_to_point: false,
                anycast: false,
            },
            dummy::create_thing(json!({"local": 3, "label": "a", "prefix": {"local": 2}})),
        ],
    );
    let outcome = submit(&store, &Unrestricted, &mixed, MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let (prefix, thing): (Option<Uuid>, Uuid) =
        sqlx::query_as("SELECT prefix_id, id FROM dummy.thing WHERE label = 'a'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let created: Uuid = sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = '10.0.0.0/8'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(prefix, Some(created));
    // Registered by the core, with a version (ETag).
    let kind: String = sqlx::query_scalar("SELECT kind FROM module_object WHERE id = $1")
        .bind(thing)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kind, "dummy.thing");
    let versions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM object_version WHERE object_id = $1")
            .bind(thing)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(versions, 1);

    // A local refusal of the module refuses the core's operations too.
    let refused = request(
        author,
        vec![domain(1, "lab2"), dummy::create_thing(json!({"label": ""}))],
    );
    let outcome = submit(&store, &Unrestricted, &refused, MAX_OPERATIONS)
        .await
        .unwrap();
    assert_eq!(rules(&outcome), ["dummy.thing.invalid"]);
    // So does a refusal of its rule on the final state, after everything was written.
    let refused = request(
        author,
        vec![
            domain(1, "lab3"),
            dummy::create_thing(json!({"label": "forbidden"})),
        ],
    );
    let outcome = submit(&store, &Unrestricted, &refused, MAX_OPERATIONS)
        .await
        .unwrap();
    assert_eq!(rules(&outcome), ["dummy.thing.forbidden"]);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM addressing_domain WHERE slug IN ('lab2', 'lab3')"
        )
        .await,
        0
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM dummy.thing").await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM module_object").await, 1);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_unknown_module_operation_is_refused_before_locks(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    for tag in ["dummy.unknown", "nomodule.create_thing"] {
        let op: Operation = serde_json::from_value(json!({"op": tag, "label": "x"})).unwrap();
        let error = submit(
            &store,
            &Unrestricted,
            &request(author, vec![domain(1, "lab"), op]),
            MAX_OPERATIONS,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(&error, RequestError::UnknownOperation { operation: 1, op } if op == tag),
            "{error:?}"
        );
    }
    // A build without the module does not know its operations.
    let core_only = PgStore::new(pool.clone());
    let error = submit(
        &core_only,
        &Unrestricted,
        &request(author, vec![dummy::create_thing(json!({"label": "x"}))]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error, RequestError::UnknownOperation { .. }),
        "{error:?}"
    );
    // Nothing was journaled: the request was not even considered.
    assert_eq!(count(&pool, "SELECT count(*) FROM changeset").await, 0);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn approval_policy_counts_module_operations(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let policy = Operation::SetApprovalPolicy {
        name: "big".into(),
        min_operations: Some(2),
        min_diff_objects: None,
        property_keys: Vec::new(),
        domains: Vec::new(),
        authors: Vec::new(),
        scopes: Vec::new(),
        author_roles: Vec::new(),
        author_groups: Vec::new(),
        bound_roles: Vec::new(),
        enabled: true,
    };
    let outcome = submit(
        &store,
        &Unrestricted,
        &request(author, vec![policy]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let one = request(author, vec![dummy::create_thing(json!({"label": "one"}))]);
    let outcome = submit(&store, &Unrestricted, &one, MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let two = request(
        author,
        vec![
            dummy::create_thing(json!({"label": "two"})),
            dummy::create_thing(json!({"label": "three"})),
        ],
    );
    let outcome = submit(&store, &Unrestricted, &two, MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Pending { .. }), "{outcome:?}");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn module_write_needs_its_permission(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let thing = |label: &str| request(author, vec![dummy::create_thing(json!({"label": label}))]);
    let outcome = submit(&store, &Grants::new(), &thing("denied"), MAX_OPERATIONS)
        .await
        .unwrap();
    assert_eq!(rules(&outcome), ["core.object.forbidden"]);
    // `<type>.propose` makes it a proposal.
    let proposer = Grants::new().everywhere(author, "dummy.thing.propose");
    let outcome = submit(&store, &proposer, &thing("proposed"), MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Pending { .. }), "{outcome:?}");
    let writer = Grants::new().everywhere(author, "dummy.thing.write");
    let outcome = submit(&store, &writer, &thing("granted"), MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_module_operation_plans_core_operations(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let op = dummy::create_thing(json!({"label": "b", "domain": "from-module"}));
    let outcome = submit(
        &store,
        &Unrestricted,
        &request(author, vec![op]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM addressing_domain WHERE slug = 'from-module'"
        )
        .await,
        1
    );
    // The core operation is checked like any other: a bad slug refuses the whole.
    let op = dummy::create_thing(json!({"label": "c", "domain": "Bad_Slug"}));
    let outcome = submit(
        &store,
        &Unrestricted,
        &request(author, vec![op]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert_eq!(rules(&outcome), ["core.slug.syntax"]);
    assert_eq!(count(&pool, "SELECT count(*) FROM dummy.thing").await, 1);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn named_lock_keys_do_not_deadlock(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    // Keys named in opposite orders: taken in canonical order, never crossed.
    for round in 0..10 {
        let forward = request(
            author,
            vec![dummy::create_thing(
                json!({"label": format!("f{round}"), "keys": ["x", "y"]}),
            )],
        );
        let backward = request(
            author,
            vec![dummy::create_thing(
                json!({"label": format!("b{round}"), "keys": ["y", "x"]}),
            )],
        );
        let (a, b) = tokio::join!(
            submit(&store, &Unrestricted, &forward, MAX_OPERATIONS),
            submit(&store, &Unrestricted, &backward, MAX_OPERATIONS),
        );
        assert!(matches!(a.unwrap(), Outcome::Applied { .. }));
        assert!(matches!(b.unwrap(), Outcome::Applied { .. }));
    }
    assert_eq!(count(&pool, "SELECT count(*) FROM dummy.thing").await, 20);
}
