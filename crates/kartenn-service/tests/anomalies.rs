//! Anomalies: recording, acknowledgement, reappearance, closure (REQ-002-35, 36).

use kartenn_core::objects::Nature;
use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::anomalies::{acknowledge, open};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

fn request(author: Uuid, operations: Vec<Operation>) -> ChangesetRequest {
    ChangesetRequest {
        author,
        motive: "m".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
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

    let listed = open(&pool, None).await.unwrap();
    assert_eq!(listed.len(), 2);
    let doc = listed
        .iter()
        .find(|a| a.rule_id == "core.addr.documentation")
        .unwrap()
        .clone();
    assert_eq!(
        open(&pool, Some("core.addr.shared_space"))
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
        open(&pool, None).await.unwrap().len(),
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
        open(&pool, None).await.unwrap().len(),
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
        }],
    );
    assert!(matches!(
        submit(&store, &Unrestricted, &close, MAX_OPERATIONS)
            .await
            .unwrap(),
        Outcome::Applied { .. }
    ));
    let remaining = open(&pool, None).await.unwrap();
    assert_eq!(
        remaining
            .iter()
            .map(|a| a.rule_id.as_str())
            .collect::<Vec<_>>(),
        vec!["core.addr.shared_space"]
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
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
    let still_open = open(&pool, Some("core.addr.documentation")).await.unwrap();
    assert_eq!(still_open.len(), 1);
    assert_eq!(still_open[0].object_id, net);
}
