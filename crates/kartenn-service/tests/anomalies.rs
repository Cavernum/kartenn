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
