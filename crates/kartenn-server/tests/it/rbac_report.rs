//! The bindings written before the groups, reported on groups by the migrations and listed
//! by the migration job (spec 025, REQ-025-14, G9; task 025-4): a base of the 005 (its
//! three subject kinds) deployed.

use kartenn_service::Operation;
use kartenn_service::declarations::SYSTEM_AUTHOR;
use kartenn_service::module::ModuleRegistry;
use kartenn_service::operations::BindingScope;
use kartenn_store_pg::rbac_migration::FormerSubject;
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Uuid;
use sqlx::{Connection, PgConnection, PgPool};

const ISSUER: &str = "https://idp.example.invalid/realms/main";

/// The version of the core migration `name`.
fn version(name: &str) -> i64 {
    kartenn_store_pg::MIGRATOR
        .iter()
        .find(|m| m.description == name)
        .map(|m| m.version)
        .expect("a migration of the core")
}

async fn person(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name, kind) VALUES (gen_random_uuid(), $1, 'person')
         RETURNING id",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A binding in the form of the 005, from a day ago, until `until` days from now.
async fn bind(
    pool: &PgPool,
    changeset: Uuid,
    subject: (&str, Option<Uuid>, Option<&str>),
    scope: (&str, Option<Uuid>),
    until: Option<i32>,
) -> Uuid {
    let (kind, id, group) = subject;
    let issuer = (kind != "identity").then_some(ISSUER);
    sqlx::query_scalar(
        "INSERT INTO binding (id, subject_kind, subject_id, issuer, group_name, role_id,
                              scope_kind, scope_id, valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1, $2, $3, $4,
                 (SELECT id FROM role WHERE key = 'auditor'), $5, $6,
                 tstzrange(now() - interval '1 day',
                           now() + make_interval(days => $7)), $8)
         RETURNING id",
    )
    .bind(kind)
    .bind(id)
    .bind(issuer)
    .bind(group)
    .bind(scope.0)
    .bind(scope.1)
    .bind(until)
    .bind(changeset)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A changeset of the system closing the binding `binding` now, as `Unbind` did.
async fn unbind(pool: &PgPool, binding: Uuid) -> Uuid {
    let changeset = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    applied(&mut tx, changeset).await;
    sqlx::query(
        "UPDATE binding SET valid_period = tstzrange(lower(valid_period), now()),
                            changeset_id = $2
         WHERE id = $1",
    )
    .bind(binding)
    .bind(changeset)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    changeset
}

async fn applied(tx: &mut PgConnection, changeset: Uuid) {
    sqlx::query(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period, applied_at)
         VALUES ($1, $2, 'before the groups', 'applied', 'direct', tstzrange(now(), NULL), now())",
    )
    .bind(changeset)
    .bind(SYSTEM_AUTHOR)
    .execute(&mut *tx)
    .await
    .unwrap();
}

#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // one base of the 005, its report, then its undo
async fn bindings_of_the_005_are_reported_on_groups(_pool: PgPool) {
    let master = kartenn_testing::master_options();
    let mut admin = PgConnection::connect_with(&master).await.unwrap();
    let name = kartenn_testing::database_name(concat!(module_path!(), "::before"));
    kartenn_testing::create_empty(&mut admin, &name)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(master.clone().database(&name))
        .await
        .unwrap();
    kartenn_store_pg::MIGRATOR
        .run_to(version("rbac groups") - 1, &pool)
        .await
        .unwrap();

    // A base of the 005.
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'auditor', '{\"fr\": \"Auditeur\"}',
                 tstzrange(now() - interval '2 days', NULL), gen_random_uuid())",
    )
    .execute(&pool)
    .await
    .unwrap();
    let alice = person(&pool, "Alice Martin").await;
    let bob = person(&pool, "Bob").await;
    let bob_too = person(&pool, "bob").await;
    let carol = person(&pool, "Carol").await;
    let created = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    applied(&mut tx, created).await;
    tx.commit().await.unwrap();
    let entity = Some(Uuid::now_v7());
    let global = ("global", None);
    let alice_global = bind(
        &pool,
        created,
        ("identity", Some(alice), None),
        global,
        None,
    )
    .await;
    let alice_bounded = bind(
        &pool,
        created,
        ("identity", Some(alice), None),
        ("entity", entity),
        Some(30),
    )
    .await;
    let alice_closed = bind(
        &pool,
        created,
        ("identity", Some(alice), None),
        ("site", entity),
        None,
    )
    .await;
    let netops = bind(
        &pool,
        created,
        ("group", None, Some("Net Ops")),
        global,
        None,
    )
    .await;
    let whole = bind(&pool, created, ("issuer", None, None), global, None).await;
    bind(&pool, created, ("identity", Some(bob), None), global, None).await;
    bind(
        &pool,
        created,
        ("identity", Some(bob_too), None),
        global,
        None,
    )
    .await;
    let carol_closed = bind(
        &pool,
        created,
        ("identity", Some(carol), None),
        global,
        None,
    )
    .await;
    let closed_alice = unbind(&pool, alice_closed).await;
    let closed_carol = unbind(&pool, carol_closed).await;

    // The migration job.
    let deployed = kartenn_store_pg::maintenance::deploy(&pool, ModuleRegistry::empty())
        .await
        .unwrap();
    let report = deployed.rbac_report.expect("a report");
    let mut keys: Vec<(&str, i32)> = report
        .groups
        .iter()
        .map(|g| (g.key.as_str(), g.bindings))
        .collect();
    keys.sort_unstable();
    assert_eq!(keys.len(), 4, "{keys:?}");
    assert_eq!(keys[0], ("compte-alice-martin", 2));
    // Two identities of one name: each its group, its key told apart.
    assert!(keys[1].0.starts_with("compte-bob-") && keys[2].0.starts_with("compte-bob-"));
    assert_ne!(keys[1].0, keys[2].0);
    assert_eq!(
        keys[3],
        ("idp-https-idp-example-invalid-realms-main-net-ops", 1)
    );
    assert!(report.groups.iter().any(|g| g.subject
        == FormerSubject::IdpGroup {
            issuer: ISSUER.into(),
            group: "Net Ops".into()
        }));
    assert_eq!(report.closed.len(), 1);
    assert_eq!(report.closed[0].id, whole);
    assert_eq!(report.closed[0].role.as_deref(), Some("auditor"));
    let lines = report.lines();
    assert!(
        lines.iter().any(|l| l.contains("compte-alice-martin")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains(&whole.to_string())),
        "{lines:?}"
    );

    // Converted in place: same identifier, same period, now on the group.
    let rows: Vec<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT b.id, g.key, upper_inf(b.valid_period) FROM binding b
         JOIN rbac_group g ON g.id = b.group_id ORDER BY g.key, b.id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let alice_rows: Vec<_> = rows
        .iter()
        .filter(|r| r.1 == "compte-alice-martin")
        .collect();
    assert_eq!(alice_rows.len(), 2, "{rows:?}");
    assert!(alice_rows.iter().any(|r| r.0 == alice_global && r.2));
    assert!(
        alice_rows.iter().any(|r| r.0 == alice_bounded && !r.2),
        "a bounded binding keeps its end"
    );
    assert!(rows.iter().any(|r| r.0 == netops));
    for gone in [whole, alice_closed, carol_closed] {
        assert!(rows.iter().all(|r| r.0 != gone), "{gone} left the table");
    }
    // Rights through the group: Alice by her group, a member of Net Ops by its group.
    let held: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM binding
         WHERE group_id IN (SELECT kartenn_member_of($1, $2, ARRAY['Net Ops'], now()))",
    )
    .bind(alice)
    .bind(ISSUER)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(held, 3);
    // The former versions stay in the history as written.
    let former: Option<String> = sqlx::query_scalar(
        "SELECT subject_kind FROM binding_history WHERE id = $1 AND group_id IS NULL LIMIT 1",
    )
    .bind(alice_global)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(former.as_deref(), Some("identity"));
    // Audited once, by the system, with the changeset of the report.
    let audited: Vec<(String, Option<Uuid>, Value)> = sqlx::query_as(
        "SELECT actor_kind, changeset_id, details FROM audit.event
         WHERE name = 'rbac.migration.reported'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audited.len(), 1);
    assert_eq!(audited[0].0, "system");
    assert_eq!(audited[0].1, Some(report.changeset));
    assert_eq!(audited[0].2["closed"], json!([whole]));
    let author: Uuid = sqlx::query_scalar("SELECT author_id FROM changeset WHERE id = $1")
        .bind(report.changeset)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(author, SYSTEM_AUTHOR);
    // Listed once.
    let again = kartenn_store_pg::maintenance::deploy(&pool, ModuleRegistry::empty())
        .await
        .unwrap();
    assert!(again.rbac_report.is_none());

    // The undo of a changeset of the 005: a binding closed is given back on the group of
    // its former subject; without such a group, it is not given back.
    let store = kartenn_store_pg::write::PgStore::new(pool.clone());
    let inverse = kartenn_service::undo::inverse(&store, closed_alice, SYSTEM_AUTHOR)
        .await
        .unwrap()
        .unwrap();
    let binds: Vec<_> = inverse
        .request
        .operations
        .iter()
        .map(Operation::inner)
        .collect();
    assert!(
        matches!(binds.as_slice(), [Operation::Bind { group, role, scope: BindingScope::Site { .. }, .. }]
            if group == "compte-alice-martin" && role == "auditor"),
        "{binds:?}"
    );
    let inverse = kartenn_service::undo::inverse(&store, closed_carol, SYSTEM_AUTHOR)
        .await
        .unwrap()
        .unwrap();
    assert!(inverse.request.operations.is_empty());
    assert_eq!(
        inverse.unsupported,
        vec![("binding".to_owned(), carol_closed)]
    );
    // The bindings converted were written since by the report: a conflict.
    let inverse = kartenn_service::undo::inverse(&store, created, SYSTEM_AUTHOR)
        .await
        .unwrap()
        .unwrap();
    let by_report = inverse
        .rbac_conflicts
        .iter()
        .filter(|c| c.by == report.changeset)
        .count();
    // Alice (2), Net Ops, Bob and bob; the binding of the whole issuer, gone, is left out.
    assert_eq!(by_report, 5, "{:?}", inverse.rbac_conflicts);
    assert!(inverse.request.operations.is_empty());
    // The report itself is not undone.
    let inverse = kartenn_service::undo::inverse(&store, report.changeset, SYSTEM_AUTHOR)
        .await
        .unwrap()
        .unwrap();
    assert!(inverse.request.operations.is_empty());
    assert_eq!(
        inverse.unsupported,
        vec![("rbac_migration".to_owned(), report.changeset)]
    );

    pool.close().await;
    kartenn_testing::drop_database(&mut admin, &name)
        .await
        .unwrap();
}
