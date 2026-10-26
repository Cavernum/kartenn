//! Anomalies: recording, acknowledgement, reappearance, closure (REQ-002-35, 36).

use kartenn_core::objects::{Nature, RangeUsage};
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{core_declarations, sync_modules};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::anomalies::{AnomalyFilter, acknowledge, open};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

fn rule(id: &str) -> AnomalyFilter<'_> {
    AnomalyFilter {
        rule: Some(id),
        ..AnomalyFilter::default()
    }
}

fn request(author: Uuid, operations: Vec<Operation>) -> ChangesetRequest {
    ChangesetRequest {
        author,
        motive: "m".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
        approval_self: true,
    }
}

#[kartenn_testing::pg_test]
async fn anomalies_are_recorded_acknowledged_and_closed(pool: PgPool) {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let store = PgStore::new(pool.clone());
    let ops = vec![
        Operation::CreateDomain {
            local: Some(1),
            slug: "lab".into(),
        },
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Local(1),
            prefix: "192.0.2.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Local(1),
            prefix: "100.64.1.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
    ];
    let outcome = submit(&store, &Unrestricted, &request(author, ops), MAX_OPERATIONS)
        .await
        .unwrap();
    let Outcome::Applied { warnings, .. } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(warnings.len(), 2, "documentation and shared space");

    let listed = open(&pool, &AnomalyFilter::default()).await.unwrap();
    assert_eq!(listed.len(), 2);
    let doc = listed
        .iter()
        .find(|a| a.rule_id == "core.addr.documentation")
        .unwrap()
        .clone();
    assert_eq!(
        open(&pool, &rule("core.addr.shared_space"))
            .await
            .unwrap()
            .len(),
        1,
        "filter by rule"
    );

    assert!(
        acknowledge(
            &pool,
            doc.id,
            author,
            "lab network, documentation range on purpose"
        )
        .await
        .unwrap()
    );
    assert!(
        acknowledge(&pool, doc.id, author, "").await.is_err(),
        "a motive is mandatory"
    );
    assert_eq!(
        open(&pool, &AnomalyFilter::default()).await.unwrap().len(),
        1,
        "acknowledged anomalies are hidden"
    );

    // A later change of the object makes the acknowledged anomaly visible again.
    sqlx::query("UPDATE prefix SET changeset_id = gen_random_uuid() WHERE id = $1")
        .bind(doc.object_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        open(&pool, &AnomalyFilter::default()).await.unwrap().len(),
        2,
        "reopened after modification"
    );

    // Closing the object closes its anomalies.
    let close = request(
        author,
        vec![Operation::Close {
            kind: ObjectKind::Prefix,
            id: Ref::Id(doc.object_id),
            expected: None,
            cascade: false,
        }],
    );
    assert!(matches!(
        submit(&store, &Unrestricted, &close, MAX_OPERATIONS)
            .await
            .unwrap(),
        Outcome::Applied { .. }
    ));
    let remaining = open(&pool, &AnomalyFilter::default()).await.unwrap();
    assert_eq!(
        remaining
            .iter()
            .map(|a| a.rule_id.as_str())
            .collect::<Vec<_>>(),
        vec!["core.addr.shared_space"]
    );
}

#[kartenn_testing::pg_test]
async fn a_changeset_closes_only_the_anomalies_of_the_rules_it_evaluated(pool: PgPool) {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let store = PgStore::new(pool.clone());
    let declaration = kartenn_service::operations::DeclarationInput {
        key: "custom.label".into(),
        value_type: kartenn_service::operations::TypeInput::Text {
            min_len: 0,
            max_len: 20,
            pattern: None,
        },
        cardinality: kartenn_core::values::Cardinality::Single,
        carriers: vec!["prefix".into()],
        inherit: None,
        module: None,
        obligations: Vec::new(),
        expected: None,
        labels: None,
    };
    let ops = vec![
        Operation::CreateDomain {
            local: Some(1),
            slug: "lab".into(),
        },
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Local(1),
            prefix: "192.0.2.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
        Operation::DeclareProperty {
            local: None,
            declaration,
        },
    ];
    let outcome = submit(&store, &Unrestricted, &request(author, ops), MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let (net, label): (Uuid, Uuid) =
        sqlx::query_as("SELECT (SELECT id FROM prefix), (SELECT id FROM property_declaration)")
            .fetch_one(&pool)
            .await
            .unwrap();
    // Assigning a property does not evaluate the core rules of the prefix: its
    // documentation anomaly stays open.
    let assign = vec![Operation::AssignProperty {
        object: Ref::Id(net),
        kind: ObjectKind::Prefix,
        property: Ref::Id(label),
        values: vec![kartenn_service::operations::RawInput::Text("lab".into())],
    }];
    let outcome = submit(
        &store,
        &Unrestricted,
        &request(author, assign),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let still_open = open(&pool, &rule("core.addr.documentation")).await.unwrap();
    assert_eq!(still_open.len(), 1);
    assert_eq!(still_open[0].object_id, net);
}

#[kartenn_testing::pg_test]
async fn anomalies_are_listed_by_domain_and_entity_ranges_included(pool: PgPool) {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let store = PgStore::new(pool.clone());
    let run = |operations| {
        let store = store.clone();
        async move {
            let outcome = submit(
                &store,
                &Unrestricted,
                &request(author, operations),
                MAX_OPERATIONS,
            )
            .await
            .unwrap();
            assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        }
    };
    for outcome in sync_modules(
        &store,
        &Unrestricted,
        &[core_declarations()],
        MAX_OPERATIONS,
    )
    .await
    .unwrap()
    {
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }
    let entity = |local, slug: &str, parent| Operation::CreateEntity {
        local: Some(local),
        slug: slug.into(),
        parent,
    };
    run(vec![
        entity(1, "group", None),
        entity(2, "branch", Some(Ref::Local(1))),
        entity(3, "vendor", None),
    ])
    .await;
    let id = |sql: String| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(sqlx::AssertSqlSafe(sql))
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let owner = id("SELECT id FROM property_declaration WHERE key = 'core.owner'".into()).await;
    let slug = |s: &str| format!("SELECT id FROM entity WHERE slug = '{s}'");
    let (group, branch, vendor) = (
        id(slug("group")).await,
        id(slug("branch")).await,
        id(slug("vendor")).await,
    );
    // Two domains, each with a network in documentation space; the first also has a range.
    let domain = |local, name: &str, owner_id: Uuid, network: &str| {
        vec![
            Operation::CreateDomain {
                local: Some(local),
                slug: name.into(),
            },
            Operation::AssignProperty {
                object: Ref::Local(local),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(owner_id.to_string())],
            },
            Operation::CreatePrefix {
                local: None,
                domain: Ref::Local(local),
                prefix: network.into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
        ]
    };
    let mut ops = domain(1, "lab", branch, "192.0.2.0/24");
    ops.push(Operation::CreateRange {
        local: None,
        domain: Ref::Local(1),
        first: "192.0.2.10".into(),
        last: "192.0.2.20".into(),
        usage: RangeUsage::Static,
    });
    ops.extend(domain(2, "partner", vendor, "198.51.100.0/24"));
    run(ops).await;
    let lab = id("SELECT id FROM addressing_domain WHERE slug = 'lab'".into()).await;
    let range = id("SELECT id FROM ip_range".into()).await;

    let count = |filter: AnomalyFilter<'static>| {
        let pool = pool.clone();
        async move { open(&pool, &filter).await.unwrap().len() }
    };
    assert_eq!(count(AnomalyFilter::default()).await, 3);
    let by_domain = AnomalyFilter {
        domain: Some(lab),
        ..AnomalyFilter::default()
    };
    assert_eq!(count(by_domain).await, 2, "network and range of the domain");
    let by_entity = |e| AnomalyFilter {
        entity: Some(e),
        ..AnomalyFilter::default()
    };
    assert_eq!(count(by_entity(group)).await, 2, "owned by a descendant");
    assert_eq!(count(by_entity(vendor)).await, 1);

    // An acknowledged anomaly of a range comes back once the range changes.
    let anomaly = open(&pool, &AnomalyFilter::default())
        .await
        .unwrap()
        .into_iter()
        .find(|a| a.object_id == range)
        .unwrap();
    assert!(
        acknowledge(&pool, anomaly.id, author, "test range")
            .await
            .unwrap()
    );
    assert_eq!(count(AnomalyFilter::default()).await, 2);
    run(vec![Operation::UpdateRange {
        id: Ref::Id(range),
        domain: None,
        usage: RangeUsage::Reserved,
        expected: None,
    }])
    .await;
    assert_eq!(
        count(AnomalyFilter::default()).await,
        3,
        "reopened after modification"
    );
}
