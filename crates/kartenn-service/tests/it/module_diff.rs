//! Contributions of the modules to the differential (spec 008, REQ-008-26; task 008-19):
//! in the order of the registry, each reading the sections before it, in the simulation
//! and stored with the applied changeset, read by the rules of the modules.

use crate::support::section_modules as sections;

use kartenn_core::objects::Nature;
use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
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

fn request(author: Uuid, operations: Vec<Operation>, simulate: bool) -> ChangesetRequest {
    ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations,
        simulate,
        idempotency_key: None,
        approval_self: true,
    }
}

fn domains(slugs: &[&str]) -> Vec<Operation> {
    slugs
        .iter()
        .map(|slug| Operation::CreateDomain {
            local: None,
            slug: (*slug).into(),
        })
        .collect()
}

fn store(pool: &PgPool) -> PgStore {
    PgStore::new(pool.clone()).with_modules(sections::registry())
}

#[kartenn_testing::pg_test]
async fn b_reads_the_section_of_a(pool: PgPool) {
    let author = person(&pool).await;
    let outcome = submit(
        &store(&pool),
        &Unrestricted,
        &request(author, domains(&["a", "b", "c"]), true),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let Outcome::Simulated {
        violations,
        sections,
        ..
    } = outcome
    else {
        panic!("not simulated: {outcome:?}");
    };
    assert!(violations.is_empty(), "{violations:?}");
    // Alpha first, though given after beta: the order of the dependencies.
    let names: Vec<&str> = sections.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["alpha.objects", "beta.echo"]);
    assert_eq!(sections[0].count, 3);
    assert!(sections[0].rows.iter().all(|r| r.kind == "domain"));
    assert_eq!(sections[1].rows[0].after, Some(json!({ "alpha_rows": 3 })));
}

#[kartenn_testing::pg_test]
async fn sections_in_simulation_and_after_application(pool: PgPool) {
    let author = person(&pool).await;
    let store = store(&pool);
    let simulated = submit(
        &store,
        &Unrestricted,
        &request(author, domains(&["a", "b"]), true),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let Outcome::Simulated { sections, .. } = simulated else {
        panic!("not simulated: {simulated:?}");
    };
    assert_eq!(sections.len(), 2);
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM changeset_diff_section")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 0, "a simulation stores nothing");

    let applied = submit(
        &store,
        &Unrestricted,
        &request(author, domains(&["a", "b"]), false),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let Outcome::Applied { changeset, .. } = applied else {
        panic!("not applied: {applied:?}");
    };
    let counts = kartenn_store_pg::sections::section_counts(&pool, changeset)
        .await
        .unwrap();
    assert_eq!(
        counts,
        [("alpha.objects".to_owned(), 2), ("beta.echo".to_owned(), 1)]
    );
    let echo = kartenn_store_pg::sections::section_page(&pool, changeset, "beta.echo", None, 10)
        .await
        .unwrap();
    assert_eq!(echo[0].1.object, changeset);
    assert_eq!(echo[0].1.after, Some(json!({ "alpha_rows": 2 })));
}

#[kartenn_testing::pg_test]
async fn a_rule_of_b_reads_a_section(pool: PgPool) {
    let author = person(&pool).await;
    let mut operations = domains(&["lab"]);
    operations[0] = Operation::CreateDomain {
        local: Some(1),
        slug: "lab".into(),
    };
    operations.push(Operation::CreatePrefix {
        local: None,
        domain: Ref::Local(1),
        prefix: "10.0.0.0/8".into(),
        nature: Nature::Container,
        point_to_point: false,
        anycast: false,
    });
    let outcome = submit(
        &store(&pool),
        &Unrestricted,
        &request(author, operations, false),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let Outcome::Refused { violations, .. } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    let rules: Vec<&str> = violations.iter().map(|v| v.rule.id).collect();
    assert_eq!(rules, ["beta.alpha.prefix"]);
    let prefixes: i64 = sqlx::query_scalar("SELECT count(*) FROM prefix")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(prefixes, 0);
}

#[kartenn_testing::pg_test]
async fn sections_are_paginated(pool: PgPool) {
    let author = person(&pool).await;
    let applied = submit(
        &store(&pool),
        &Unrestricted,
        &request(author, domains(&["a", "b", "c", "d", "e"]), false),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let Outcome::Applied { changeset, .. } = applied else {
        panic!("not applied: {applied:?}");
    };
    let mut seen = Vec::new();
    let mut after = None;
    loop {
        let page =
            kartenn_store_pg::sections::section_page(&pool, changeset, "alpha.objects", after, 2)
                .await
                .unwrap();
        if page.is_empty() {
            break;
        }
        assert!(page.len() <= 2);
        after = page.last().map(|(seq, _)| *seq);
        seen.extend(page.into_iter().map(|(_, row)| row.object));
    }
    assert_eq!(seen.len(), 5);
    let distinct: std::collections::BTreeSet<_> = seen.iter().collect();
    assert_eq!(distinct.len(), 5);
}
