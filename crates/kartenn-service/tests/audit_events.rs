//! Audit events of writes, decisions and administration (spec 012, REQ-012-8, 9, 11;
//! task 012-6): one test per row of the table of REQ-012-8 that the core emits.

use kartenn_service::authz::{Grants, Unrestricted};
use kartenn_service::expected::Sources;
use kartenn_service::operations::{DeclarationInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::review::{expire_pending, reject, validate_pending, withdraw};
use kartenn_service::{ChangesetRequest, Operation, Outcome, RequestError, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

async fn person(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), $1) RETURNING id",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn request(author: Uuid, operations: Vec<Operation>) -> ChangesetRequest {
    ChangesetRequest {
        author,
        motive: "audit test".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
    }
}

fn domain(slug: &str) -> Operation {
    Operation::CreateDomain {
        local: None,
        slug: slug.into(),
    }
}

/// `(name, actor id, outcome, changeset, reason)` of the events, in order.
type Row = (String, Option<Uuid>, String, Option<Uuid>, Option<String>);

async fn events(pool: &PgPool) -> Vec<Row> {
    sqlx::query_as(
        "SELECT name, actor_id, outcome, changeset_id, reason FROM audit.event ORDER BY seq",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn names(pool: &PgPool) -> Vec<String> {
    events(pool).await.into_iter().map(|e| e.0).collect()
}

async fn clear(pool: &PgPool) {
    // As the owner: the application role cannot delete audit events.
    sqlx::query("TRUNCATE audit.event")
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn applied_and_refused_changesets_are_audited_with_their_author(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let author = person(&pool, "author").await;
    let Outcome::Applied { changeset, .. } = submit(
        &store,
        &Unrestricted,
        &request(author, vec![domain("lab")]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap() else {
        panic!()
    };
    let refused = submit(
        &store,
        &Unrestricted,
        &request(author, vec![domain("Bad")]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let Outcome::Refused { changeset: no, .. } = refused else {
        panic!("{refused:?}")
    };
    let rows = events(&pool).await;
    assert_eq!(
        rows,
        vec![
            (
                "changeset.state.applied".into(),
                Some(author),
                "success".into(),
                Some(changeset),
                None
            ),
            (
                "changeset.state.refused".into(),
                Some(author),
                "failure".into(),
                Some(no),
                None
            ),
        ]
    );
    let (motive, rules): (String, serde_json::Value) = sqlx::query_as(
        "SELECT motive, details -> 'rules' FROM audit.event WHERE name = 'changeset.state.refused'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(motive, "audit test");
    assert_eq!(rules, serde_json::json!(["core.slug.syntax"]));
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn refusals_of_rights_and_administration_are_audited(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let author = person(&pool, "author").await;
    let policy = Operation::SetApprovalPolicy {
        name: "large".into(),
        min_operations: Some(5),
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
    // Without the right: refused, and the refusal of right is audited.
    let outcome = submit(
        &store,
        &Grants::new(),
        &request(author, vec![policy.clone()]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Refused { .. }), "{outcome:?}");
    let rows = events(&pool).await;
    assert_eq!(
        rows.iter()
            .map(|r| (r.0.as_str(), r.4.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("changeset.state.refused", None),
            ("authz.access.denied", Some("changeset.policy.forbidden")),
        ]
    );
    clear(&pool).await;
    // With it, the policy and a declaration are audited with the changeset.
    let declare = Operation::DeclareProperty {
        local: None,
        declaration: DeclarationInput {
            key: "custom.note".into(),
            value_type: TypeInput::Text {
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
        },
    };
    submit(
        &store,
        &Unrestricted,
        &request(author, vec![policy, declare]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert_eq!(
        names(&pool).await,
        [
            "changeset.state.applied",
            "policy.approval.changed",
            "property.declaration.changed"
        ]
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn decisions_on_pending_changesets_are_audited(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let author = person(&pool, "author").await;
    let validator = person(&pool, "validator").await;
    let hold_all = Operation::SetApprovalPolicy {
        name: "all".into(),
        min_operations: Some(1),
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
    let admin = person(&pool, "admin").await;
    submit(
        &store,
        &Unrestricted,
        &request(admin, vec![hold_all]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    clear(&pool).await;
    let pending = |slug: &'static str| {
        let store = store.clone();
        async move {
            match submit(
                &store,
                &Unrestricted,
                &request(author, vec![domain(slug)]),
                MAX_OPERATIONS,
            )
            .await
            .unwrap()
            {
                Outcome::Pending { changeset, .. } => changeset,
                other => panic!("{other:?}"),
            }
        }
    };
    let (one, two, three) = (
        pending("one").await,
        pending("two").await,
        pending("three").await,
    );
    withdraw(&store, one, author).await.unwrap();
    reject(&store, &Unrestricted, two, &validator.into(), "not now")
        .await
        .unwrap();
    // Validating one's own changeset is a refusal of right, audited.
    let own = validate_pending(
        &store,
        &Unrestricted,
        &Sources::none(),
        three,
        &author.into(),
        MAX_OPERATIONS,
    )
    .await;
    assert!(matches!(own, Err(RequestError::Review(_))), "{own:?}");
    sqlx::query("UPDATE changeset SET expires_at = now() - interval '1 day' WHERE id = $1")
        .bind(three)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(expire_pending(&store).await.unwrap(), vec![three]);
    let rows = events(&pool).await;
    let summary: Vec<(&str, Option<Uuid>, Option<Uuid>)> =
        rows.iter().map(|r| (r.0.as_str(), r.1, r.3)).collect();
    assert_eq!(
        summary,
        vec![
            ("changeset.state.pending", Some(author), Some(one)),
            ("changeset.state.pending", Some(author), Some(two)),
            ("changeset.state.pending", Some(author), Some(three)),
            ("changeset.state.withdrawn", Some(author), Some(one)),
            ("changeset.state.rejected", Some(validator), Some(two)),
            ("authz.access.denied", Some(author), Some(three)),
            ("changeset.state.expired", None, Some(three)),
        ]
    );
    let rejection: String = sqlx::query_scalar(
        "SELECT motive FROM audit.event WHERE name = 'changeset.state.rejected'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rejection, "not now");
    let expired_by: String = sqlx::query_scalar(
        "SELECT actor_kind FROM audit.event WHERE name = 'changeset.state.expired'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(expired_by, "system");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_migration_job_and_module_sync_are_the_system(pool: PgPool) {
    kartenn_store_pg::maintenance::deploy(&pool).await.unwrap();
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT name, actor_kind FROM audit.event ORDER BY seq")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows[0], ("sys.schema.migrated".into(), "system".into()));
    assert!(
        rows.iter()
            .any(|r| r == &("changeset.state.applied".into(), "system".into())),
        "the declarations of the core, synchronised by the system: {rows:?}"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_changeset_whose_audit_fails_is_not_applied(pool: PgPool) {
    sqlx::raw_sql(
        "CREATE FUNCTION refuse() RETURNS trigger LANGUAGE plpgsql AS
             $$ BEGIN RAISE 'audit unavailable'; END $$;
         CREATE TRIGGER refuse BEFORE INSERT ON audit.event FOR EACH ROW EXECUTE FUNCTION refuse();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let store = PgStore::new(pool.clone());
    let author = person(&pool, "author").await;
    let outcome = submit(
        &store,
        &Unrestricted,
        &request(author, vec![domain("lab")]),
        MAX_OPERATIONS,
    )
    .await;
    assert!(outcome.is_err(), "{outcome:?}");
    let domains: i64 = sqlx::query_scalar("SELECT count(*) FROM addressing_domain")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(domains, 0, "REQ-012-11: no audit, no action");
}
