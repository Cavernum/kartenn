//! Operations of a module in the changeset pipeline (spec 008, REQ-008-4, 27; task 008-3).

use crate::support::dummy_module as dummy;

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
        apply_own: true,
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

#[kartenn_testing::pg_test]
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

#[kartenn_testing::pg_test]
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

#[kartenn_testing::pg_test]
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
        module_triggers: Vec::new(),
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

#[kartenn_testing::pg_test]
async fn module_write_needs_its_permission(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let thing = |label: &str| request(author, vec![dummy::create_thing(json!({"label": label}))]);
    let outcome = submit(&store, &Grants::new(), &thing("denied"), MAX_OPERATIONS)
        .await
        .unwrap();
    assert_eq!(rules(&outcome), ["core.object.forbidden"]);
    // `<type>.edit` alone makes it a proposal (spec 024, REQ-024-10).
    let proposer = Grants::new().everywhere(author, "dummy.thing.edit");
    let outcome = submit(&store, &proposer, &thing("proposed"), MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Pending { .. }), "{outcome:?}");
    let writer = proposer.everywhere(author, "dummy.thing.apply_own");
    let outcome = submit(&store, &writer, &thing("granted"), MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}

#[kartenn_testing::pg_test]
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

#[kartenn_testing::pg_test]
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

/// Applies `operations`, returns the changeset.
async fn applied(store: &PgStore, author: Uuid, operations: Vec<Operation>) -> Uuid {
    let outcome = submit(
        store,
        &Unrestricted,
        &request(author, operations),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    match outcome {
        Outcome::Applied { changeset, .. } => changeset,
        other => panic!("not applied: {other:?}"),
    }
}

async fn thing(pool: &PgPool, label: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM dummy.thing WHERE label = $1")
        .bind(label)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn current_label(pool: &PgPool, id: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT label FROM dummy.thing WHERE id = $1 AND valid_period @> now()")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn version(pool: &PgPool, id: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT changeset_id FROM object_version WHERE object_id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn close(id: Uuid) -> Operation {
    Operation::Close {
        kind: dummy::thing_kind(),
        id: Ref::Id(id),
        expected: None,
        cascade: false,
    }
}

#[kartenn_testing::pg_test]
async fn module_object_has_etag_and_history(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let created = applied(
        &store,
        author,
        vec![dummy::create_thing(json!({"label": "a"}))],
    )
    .await;
    let id = thing(&pool, "a").await;
    assert_eq!(version(&pool, id).await, created);
    let update = |label: &str| dummy::update_thing(json!({"id": {"id": id}, "label": label}));
    let changed = applied(&store, author, vec![update("b")]).await;
    // A new version (ETag), its record rewritten, the former one in the history.
    assert_eq!(version(&pool, id).await, changed);
    let record: Uuid = sqlx::query_scalar("SELECT changeset_id FROM module_object WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(record, changed);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM module_object_history WHERE kind = 'dummy.thing'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM dummy.thing_history WHERE label = 'a'"
        )
        .await,
        1
    );
    // `Expect` guards it like an object of the core.
    let expect = |expected: Uuid| Operation::Expect {
        kind: dummy::thing_kind(),
        id: Ref::Id(id),
        expected,
    };
    let outcome = submit(
        &store,
        &Unrestricted,
        &request(author, vec![expect(created), update("c")]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert_eq!(rules(&outcome), ["changeset.operation.conflict"]);
    applied(&store, author, vec![expect(changed), update("c")]).await;
    assert_eq!(current_label(&pool, id).await.as_deref(), Some("c"));
    // `Within`: an operation of a module on its own period.
    let start: i64 = sqlx::query_scalar(
        "SELECT (extract(epoch FROM now() + interval '1 day') * 1000000)::bigint",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let later = Operation::Within {
        period: kartenn_service::operations::PeriodSpec { start, end: None },
        operation: Box::new(dummy::create_thing(json!({"label": "later"}))),
    };
    applied(&store, author, vec![later]).await;
    let future: bool = sqlx::query_scalar(
        "SELECT lower(o.valid_period) > now() AND lower(t.valid_period) = lower(o.valid_period)
         FROM module_object o JOIN dummy.thing t USING (id) WHERE t.label = 'later'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(future);
    // Closed, record and rows: no current version; an unknown one is refused.
    applied(&store, author, vec![close(id)]).await;
    assert_eq!(current_label(&pool, id).await, None);
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM module_object WHERE id = $1 AND valid_period @> now()",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(live, 0);
    let outcome = submit(
        &store,
        &Unrestricted,
        &request(author, vec![close(Uuid::now_v7())]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert_eq!(rules(&outcome), ["changeset.operation.no_version"]);
}

#[kartenn_testing::pg_test]
async fn undo_closes_a_created_module_object(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let created = applied(
        &store,
        author,
        vec![dummy::create_thing(json!({"label": "a"}))],
    )
    .await;
    let id = thing(&pool, "a").await;
    let undo = kartenn_service::undo::inverse(&store, created, author)
        .await
        .unwrap()
        .unwrap();
    assert!(undo.conflicts.is_empty() && undo.unsupported.is_empty());
    assert!(
        matches!(
            undo.request.operations.as_slice(),
            [Operation::Close { kind, id: Ref::Id(closed), .. }]
                if *kind == dummy::thing_kind() && *closed == id
        ),
        "{:?}",
        undo.request.operations
    );
    applied(&store, author, undo.request.operations).await;
    assert_eq!(current_label(&pool, id).await, None);
}

#[kartenn_testing::pg_test]
async fn undo_restores_a_modified_module_object(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    applied(
        &store,
        author,
        vec![dummy::create_thing(json!({"label": "a"}))],
    )
    .await;
    let id = thing(&pool, "a").await;
    let changed = applied(
        &store,
        author,
        vec![dummy::update_thing(json!({"id": {"id": id}, "label": "b"}))],
    )
    .await;
    assert_eq!(current_label(&pool, id).await.as_deref(), Some("b"));
    let undo = kartenn_service::undo::inverse(&store, changed, author)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(
            undo.request.operations.as_slice(),
            [Operation::Restore { kind, id: Ref::Id(restored), before, property: None }]
                if *kind == dummy::thing_kind() && *restored == id && *before == changed
        ),
        "{:?}",
        undo.request.operations
    );
    applied(&store, author, undo.request.operations).await;
    assert_eq!(current_label(&pool, id).await.as_deref(), Some("a"));
}

/// A domain `lab` with a network `10.0.0.0/24`, and a thing `a` on the network that
/// references a domain `other` without prefixes: (network, other domain, thing).
async fn referenced(store: &PgStore, pool: &PgPool, author: Uuid) -> (Uuid, Uuid, Uuid) {
    applied(
        store,
        author,
        vec![
            domain(1, "lab"),
            Operation::CreatePrefix {
                local: Some(2),
                domain: Ref::Local(1),
                prefix: "10.0.0.0/24".into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
            domain(3, "other"),
            dummy::create_thing(
                json!({"label": "a", "prefix": {"local": 2}, "domain_ref": {"local": 3}}),
            ),
        ],
    )
    .await;
    let network: Uuid = sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = '10.0.0.0/24'")
        .fetch_one(pool)
        .await
        .unwrap();
    let other: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = 'other'")
        .fetch_one(pool)
        .await
        .unwrap();
    (network, other, thing(pool, "a").await)
}

fn close_kind(kind: kartenn_service::ObjectKind, id: Uuid, cascade: bool) -> Operation {
    Operation::Close {
        kind,
        id: Ref::Id(id),
        expected: None,
        cascade,
    }
}

fn subjects(outcome: &Outcome, rule: &str) -> Vec<Uuid> {
    match outcome {
        Outcome::Refused { violations, .. } => violations
            .iter()
            .find(|v| v.rule.id == rule)
            .map(|v| v.subjects.clone())
            .unwrap_or_default(),
        other => panic!("not refused: {other:?}"),
    }
}

#[kartenn_testing::pg_test]
async fn closing_a_referenced_core_object_is_refused_with_dependents(pool: PgPool) {
    use kartenn_service::ObjectKind;
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let (network, other, thing) = referenced(&store, &pool, author).await;
    for (kind, target) in [(ObjectKind::Prefix, network), (ObjectKind::Domain, other)] {
        let outcome = submit(
            &store,
            &Unrestricted,
            &request(author, vec![close_kind(kind, target, false)]),
            MAX_OPERATIONS,
        )
        .await
        .unwrap();
        assert_eq!(rules(&outcome), ["core.object.dependents"]);
        assert_eq!(
            subjects(&outcome, "core.object.dependents"),
            [target, thing]
        );
    }
    assert_eq!(current_label(&pool, thing).await.as_deref(), Some("a"));
}

#[kartenn_testing::pg_test]
async fn cascade_closes_module_dependents(pool: PgPool) {
    use kartenn_service::ObjectKind;
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let (network, _, thing) = referenced(&store, &pool, author).await;
    let outcome = submit(
        &store,
        &Unrestricted,
        &request(author, vec![close_kind(ObjectKind::Prefix, network, true)]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    // The thing closed with its network, its record and its rows.
    assert_eq!(current_label(&pool, thing).await, None);
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM module_object WHERE id = $1 AND valid_period @> now()",
    )
    .bind(thing)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(live, 0);
}

#[kartenn_testing::pg_test]
async fn nullify_policy_unassigns(pool: PgPool) {
    use kartenn_service::ObjectKind;
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let (_, other, thing) = referenced(&store, &pool, author).await;
    let outcome = submit(
        &store,
        &Unrestricted,
        &request(author, vec![close_kind(ObjectKind::Domain, other, true)]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    // The thing lives on, its reference to the domain cleared under a new version.
    let (label, cleared): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT label, domain_ref FROM dummy.thing WHERE id = $1 AND valid_period @> now()",
    )
    .bind(thing)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((label.as_str(), cleared), ("a", None));
    let Outcome::Applied { changeset, .. } = outcome else {
        unreachable!()
    };
    assert_eq!(version(&pool, thing).await, changeset);
    // Clearing a reference is a write on the thing: it needs its permission.
    applied(
        &store,
        author,
        vec![
            domain(1, "third"),
            dummy::create_thing(json!({"label": "b", "domain_ref": {"local": 1}})),
        ],
    )
    .await;
    let other: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = 'third'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let only_domains = Grants::new()
        .everywhere(author, "core.domain.edit")
        .everywhere(author, "core.domain.apply_own");
    let outcome = submit(
        &store,
        &only_domains,
        &request(author, vec![close_kind(ObjectKind::Domain, other, true)]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(
        rules(&outcome).contains(&"core.object.forbidden"),
        "{outcome:?}"
    );
}

#[kartenn_testing::pg_test]
async fn bind_external_on_module_object(pool: PgPool) {
    use kartenn_service::port::Store as _;
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let bind = Operation::BindExternal {
        object: Ref::Local(1),
        kind: dummy::thing_kind(),
        source: "inventory".into(),
        external_id: "T-1".into(),
    };
    applied(
        &store,
        author,
        vec![dummy::create_thing(json!({"local": 1, "label": "a"})), bind],
    )
    .await;
    let id = thing(&pool, "a").await;
    let (object, kind): (Uuid, String) = sqlx::query_as(
        "SELECT object_id, object_type FROM external_id
         WHERE source = 'inventory' AND external_id = 'T-1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((object, kind.as_str()), (id, "dummy.thing"));
    // An import finds it by its identifier in the source.
    let found = store
        .external_objects("inventory", &["T-1".to_owned()])
        .await
        .unwrap();
    assert_eq!(found.get("T-1"), Some(&id));
}
