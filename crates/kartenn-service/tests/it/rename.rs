//! Renaming a domain, an entity or a site keeps its UUID (spec 002, REQ-002-47).

use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::Named;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

async fn run(pool: &PgPool, author: Uuid, operations: Vec<Operation>) -> Outcome {
    let request = ChangesetRequest {
        author,
        motive: "rename".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
        apply_own: true,
    };
    submit(
        &PgStore::new(pool.clone()),
        &Unrestricted,
        &request,
        MAX_OPERATIONS,
    )
    .await
    .unwrap()
}

fn rename(kind: Named, id: Uuid, slug: &str) -> Operation {
    Operation::Rename {
        kind,
        id: Ref::Id(id),
        slug: slug.into(),
        expected: None,
    }
}

fn refused(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        other => panic!("{other:?}"),
    }
}

#[kartenn_testing::pg_test]
async fn a_renamed_object_keeps_its_uuid(pool: PgPool) {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let created = run(
        &pool,
        author,
        vec![
            Operation::CreateDomain {
                local: None,
                slug: "prod".into(),
            },
            Operation::CreateDomain {
                local: None,
                slug: "lab".into(),
            },
            Operation::CreateEntity {
                local: None,
                slug: "group".into(),
                parent: None,
            },
            Operation::CreateSite {
                local: None,
                slug: "campus".into(),
                parent: None,
            },
        ],
    )
    .await;
    assert!(matches!(created, Outcome::Applied { .. }), "{created:?}");
    let id = |sql: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(sql)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let prod = id("SELECT id FROM addressing_domain WHERE slug = 'prod'").await;
    let group = id("SELECT id FROM entity").await;
    let campus = id("SELECT id FROM site").await;

    let renamed = run(
        &pool,
        author,
        vec![
            rename(Named::Domain, prod, "prod-eu"),
            rename(Named::Entity, group, "holding"),
            rename(Named::Site, campus, "north-campus"),
        ],
    )
    .await;
    assert!(matches!(renamed, Outcome::Applied { .. }), "{renamed:?}");
    assert_eq!(
        id("SELECT id FROM addressing_domain WHERE slug = 'prod-eu'").await,
        prod
    );
    assert_eq!(
        id("SELECT id FROM entity WHERE slug = 'holding'").await,
        group
    );
    assert_eq!(
        id("SELECT id FROM site WHERE slug = 'north-campus'").await,
        campus
    );
    // The former slug stays in the history of the same object.
    let former: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM addressing_domain_history WHERE id = $1 AND slug = 'prod'",
    )
    .bind(prod)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(former, 1);

    // A slug taken by another domain, or malformed, is refused.
    assert_eq!(
        refused(&run(&pool, author, vec![rename(Named::Domain, prod, "lab")]).await),
        vec!["core.slug.duplicate"]
    );
    assert_eq!(
        refused(&run(&pool, author, vec![rename(Named::Domain, prod, "Prod EU")]).await),
        vec!["core.slug.syntax"]
    );
}
