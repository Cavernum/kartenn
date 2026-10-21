//! Imports in batches (spec 004, REQ-004-28 to 30; spec 002, REQ-002-41; tasks 004-18,
//! 19).

use kartenn_core::objects::{AddressUsage, Nature};
use std::sync::atomic::{AtomicBool, Ordering};

use kartenn_service::authz::{Authorizer, Subject, Unrestricted};
use kartenn_service::import::{ImportError, create_import, import_status, run_import};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::{ImportItem, Store};
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, StoreError, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

const SOURCE: &str = "legacy-ipam";

async fn author(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'connector') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

fn item(external_id: Option<&str>, operation: Operation) -> ImportItem {
    ImportItem {
        external_id: external_id.map(Into::into),
        operation,
    }
}

/// A domain, a network, `count` addresses of it and one address outside any network,
/// given addresses first (the import orders them).
fn items(count: u8) -> Vec<ImportItem> {
    let mut items: Vec<ImportItem> = (1..=count)
        .map(|i| {
            item(
                Some(&format!("addr-{i}")),
                Operation::CreateAddress {
                    local: Some(100 + u32::from(i)),
                    domain: Ref::Local(1),
                    address: format!("10.1.2.{i}"),
                    usage: AddressUsage::Host,
                },
            )
        })
        .collect();
    items.push(item(
        Some("stray"),
        Operation::CreateAddress {
            local: Some(99),
            domain: Ref::Local(1),
            address: "10.9.0.1".into(),
            usage: AddressUsage::Host,
        },
    ));
    items.push(item(
        Some("net"),
        Operation::CreatePrefix {
            local: Some(2),
            domain: Ref::Local(1),
            prefix: "10.1.2.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
    ));
    items.push(item(
        Some("lab"),
        Operation::CreateDomain {
            local: Some(1),
            slug: "lab".into(),
        },
    ));
    items
}

async fn count(pool: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn an_import_pauses_on_a_faulty_batch_and_resumes(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let author = author(&pool).await;
    // REQ-004-28: more operations than a changeset of 60 allows, in batches.
    let id = create_import(&store, author, SOURCE, items(200), 60)
        .await
        .unwrap();
    let status = run_import(&store, &Unrestricted, id, 60)
        .await
        .unwrap()
        .unwrap();
    // The stray address comes in the last batch of addresses: refused, paused.
    assert_eq!(status.state, "paused");
    assert!(status.remaining >= 1, "{status:?}");
    assert_eq!(
        status.report["violations"][0]["rule"], "core.address.outside_network",
        "{}",
        status.report
    );
    let before = count(&pool, "address").await;
    assert!(before > 0 && before < 201, "{before}");
    // REQ-004-30: the status is readable at any time.
    assert_eq!(import_status(&store, id).await.unwrap().unwrap(), status);

    // Its network appears; the import resumes at the failed batch (REQ-004-29).
    let domain: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain")
        .fetch_one(&pool)
        .await
        .unwrap();
    let request = ChangesetRequest {
        author,
        motive: "network of the stray address".into(),
        period: None,
        operations: vec![Operation::CreatePrefix {
            local: None,
            domain: Ref::Id(domain),
            prefix: "10.9.0.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        }],
        simulate: false,
        idempotency_key: None,
    };
    assert!(matches!(
        submit(&store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap(),
        Outcome::Applied { .. }
    ));
    // A paused import runs once resumed, not before (REQ-006-19).
    let paused = run_import(&store, &Unrestricted, id, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(paused, status);
    assert!(
        store
            .set_import(id, &["paused"], "ready", &serde_json::json!({}))
            .await
            .unwrap()
    );
    let status = run_import(&store, &Unrestricted, id, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.state, "done", "{status:?}");
    assert_eq!(status.remaining, 0);
    assert_eq!(count(&pool, "address").await, 201);
    assert_eq!(count(&pool, "external_id").await, 203);

    // A full replay of the same elements creates nothing (REQ-004-29).
    let again = create_import(&store, author, SOURCE, items(200), 60)
        .await
        .unwrap();
    let status = run_import(&store, &Unrestricted, again, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.state, "done");
    assert_eq!(count(&pool, "address").await, 201);
    assert_eq!(count(&pool, "prefix").await, 2);
}

#[kartenn_testing::pg_test]
async fn an_import_needs_identifiers_and_known_references(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let author = author(&pool).await;
    let domain = |external: Option<&str>| {
        item(
            external,
            Operation::CreateDomain {
                local: Some(1),
                slug: "lab".into(),
            },
        )
    };
    assert!(matches!(
        create_import(&store, author, SOURCE, vec![domain(None)], 10).await,
        Err(ImportError::MissingExternalId(0))
    ));
    let orphan = item(
        Some("x"),
        Operation::CreateAddress {
            local: Some(2),
            domain: Ref::Local(7),
            address: "10.1.2.3".into(),
            usage: AddressUsage::Host,
        },
    );
    assert!(matches!(
        create_import(
            &store,
            author,
            SOURCE,
            vec![domain(Some("lab")), orphan],
            10
        )
        .await,
        Err(ImportError::UnknownReference {
            element: 1,
            local: 7
        })
    ));
    // REQ-002-41: one object per identifier in a source.
    let bind = |object: Uuid| Operation::BindExternal {
        object: Ref::Id(object),
        kind: ObjectKind::Domain,
        source: SOURCE.into(),
        external_id: "lab".into(),
    };
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
        &request(vec![
            Operation::CreateDomain {
                local: Some(1),
                slug: "lab".into(),
            },
            Operation::BindExternal {
                object: Ref::Local(1),
                kind: ObjectKind::Domain,
                source: SOURCE.into(),
                external_id: "lab".into(),
            },
        ]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(created, Outcome::Applied { .. }), "{created:?}");
    let other = Uuid::now_v7();
    match submit(
        &store,
        &Unrestricted,
        &request(vec![bind(other)]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap()
    {
        Outcome::Refused { violations, .. } => {
            assert_eq!(violations[0].rule.id, "core.external_id.duplicate");
        }
        other => panic!("{other:?}"),
    }
}

/// Grants everything, and cancels the import at its first question: during the first
/// batch.
struct Cancelling {
    pool: PgPool,
    import: Uuid,
    asked: AtomicBool,
}

impl Authorizer for Cancelling {
    async fn authorize_many(
        &self,
        _: &Subject,
        _: &str,
        _: &[Uuid],
    ) -> Result<Vec<Uuid>, StoreError> {
        if !self.asked.swap(true, Ordering::SeqCst) {
            sqlx::query("UPDATE import SET state = 'cancelled' WHERE id = $1")
                .bind(self.import)
                .execute(&self.pool)
                .await
                .unwrap();
        }
        Ok(Vec::new())
    }
}

#[kartenn_testing::pg_test]
async fn a_cancelled_import_stops_after_the_batch_in_progress(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let author = author(&pool).await;
    let id = create_import(&store, author, SOURCE, items(200), 60)
        .await
        .unwrap();
    let cancelling = Cancelling {
        pool: pool.clone(),
        import: id,
        asked: AtomicBool::new(false),
    };
    // REQ-006-19: the batch in progress ends, the batches applied stay.
    let status = run_import(&store, &cancelling, id, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.state, "cancelled");
    assert_eq!(status.applied, 1, "{status:?}");
    assert!(status.remaining > 0);
    let applied: i64 =
        sqlx::query_scalar("SELECT count(*) FROM import_batch WHERE changeset_id IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(applied, 1);
    // A cancelled import does not run again.
    let again = run_import(&store, &Unrestricted, id, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again, status);
}
