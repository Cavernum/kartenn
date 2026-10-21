//! Audit search, retroactive marking and pseudonymisation (REQ-004-27, 31 to 33).

use kartenn_core::objects::Nature;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::PeriodSpec;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::audit::{AuditFilter, pseudonymise, search};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

async fn person(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO person (id, display_name, email) VALUES (gen_random_uuid(), $1, $1 || '@example.org') RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

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

#[kartenn_testing::pg_test]
async fn audit_is_searchable_by_author_state_object_and_retroactivity(pool: PgPool) {
    let (alice, bob) = (person(&pool, "alice").await, person(&pool, "bob").await);
    let store = PgStore::new(pool.clone());
    let create = request(
        alice,
        vec![Operation::CreateDomain {
            local: None,
            slug: "prod".into(),
        }],
    );
    let Outcome::Applied { changeset: c1, .. } =
        submit(&store, &Unrestricted, &create, MAX_OPERATIONS)
            .await
            .unwrap()
    else {
        panic!()
    };
    let d: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut past = request(
        bob,
        vec![Operation::CreatePrefix {
            local: None,
            domain: Ref::Id(d),
            prefix: "10.0.0.0/8".into(),
            nature: Nature::Container,
            point_to_point: false,
            anycast: false,
        }],
    );
    past.period = Some(PeriodSpec {
        start: 1_735_689_600_000_000,
        end: None,
    }); // 2025-01-01, retroactive
    let Outcome::Applied { changeset: c2, .. } =
        submit(&store, &Unrestricted, &past, MAX_OPERATIONS)
            .await
            .unwrap()
    else {
        panic!()
    };
    submit(
        &store,
        &Unrestricted,
        &request(
            bob,
            vec![Operation::CreateDomain {
                local: None,
                slug: "Bad".into(),
            }],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let close = request(
        alice,
        vec![Operation::Close {
            kind: ObjectKind::Domain,
            id: Ref::Id(d),
            expected: None,
            cascade: false,
        }],
    );
    let Outcome::Applied { changeset: c4, .. } =
        submit(&store, &Unrestricted, &close, MAX_OPERATIONS)
            .await
            .unwrap()
    else {
        panic!()
    };

    let ids = |entries: Vec<kartenn_store_pg::audit::AuditEntry>| {
        entries.into_iter().map(|e| e.id).collect::<Vec<_>>()
    };
    assert_eq!(
        ids(search(
            &pool,
            &AuditFilter {
                author: Some(alice),
                ..AuditFilter::default()
            },
            10
        )
        .await
        .unwrap()),
        vec![c4, c1]
    );
    assert_eq!(
        search(
            &pool,
            &AuditFilter {
                state: Some("refused".into()),
                ..AuditFilter::default()
            },
            10
        )
        .await
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        ids(search(
            &pool,
            &AuditFilter {
                retroactive: Some(true),
                ..AuditFilter::default()
            },
            10
        )
        .await
        .unwrap()),
        vec![c2]
    );
    assert_eq!(
        ids(search(
            &pool,
            &AuditFilter {
                object: Some(d),
                ..AuditFilter::default()
            },
            10
        )
        .await
        .unwrap()),
        vec![c4, c1],
        "creation and closure of the domain, history included"
    );
}

#[kartenn_testing::pg_test]
async fn pseudonymisation_keeps_the_history(pool: PgPool) {
    let alice = person(&pool, "alice").await;
    let store = PgStore::new(pool.clone());
    let Outcome::Applied { changeset, .. } = submit(
        &store,
        &Unrestricted,
        &request(
            alice,
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
    sqlx::query("UPDATE person SET departed_at = now() - interval '400 days' WHERE id = $1")
        .bind(alice)
        .execute(&pool)
        .await
        .unwrap();
    let instance = kartenn_service::audit::Instance::new("test");
    assert_eq!(pseudonymise(&pool, 365, &instance).await.unwrap(), 1);
    assert_eq!(
        pseudonymise(&pool, 365, &instance).await.unwrap(),
        0,
        "only once"
    );
    let entry = search(&pool, &AuditFilter::default(), 10)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(entry.id, changeset, "the changeset is untouched");
    assert!(entry.author.starts_with("personne #"), "{}", entry.author);
    let email: Option<String> = sqlx::query_scalar("SELECT email FROM person WHERE id = $1")
        .bind(alice)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(email, None);
}
