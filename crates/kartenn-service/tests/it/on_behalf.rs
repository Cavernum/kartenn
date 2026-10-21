//! A service account that writes on behalf of a requester (spec 016, REQ-016-12, 13; G10;
//! task 016-12): the requester becomes an identity `requester`, named on the changeset and
//! in every event of the request; the rights stay the service account's.

use kartenn_core::audit::{Source, event_schema};
use kartenn_service::audit::{Instance, RequestContext};
use kartenn_service::authn::{
    AuthnError, Identities, IdentityKind, Method, Principal, Requester, act_on_behalf,
};
use kartenn_service::authz::{Grants, Unrestricted};
use kartenn_service::expected::Sources;
use kartenn_service::operations::{BindingScope, BindingSubject};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, submit, submit_in};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

async fn identity(pool: &PgPool, kind: &str) -> Uuid {
    sqlx::query_scalar(
        // Service accounts have unique names.
        "INSERT INTO person (id, display_name, kind)
         VALUES (gen_random_uuid(), $1 || '-' || gen_random_uuid(), $1) RETURNING id",
    )
    .bind(kind)
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
    }
}

fn principal(id: Uuid, kind: IdentityKind) -> Principal {
    Principal {
        identity: id,
        kind,
        issuer: None,
        groups: Vec::new(),
        method: Method::Basic,
    }
}

fn requester() -> Requester {
    Requester {
        sub: "jdoe".into(),
        iss: Some("https://tickets.example".into()),
        name: Some("J. Doe".into()),
    }
}

/// A service account bound, everywhere, to a role that may act on behalf of others.
async fn connector(pool: &PgPool, store: &PgStore) -> Uuid {
    let admin = identity(pool, "person").await;
    let account = identity(pool, "service_account").await;
    let bound = submit(
        store,
        &Unrestricted,
        &request(
            admin,
            vec![
                Operation::SetRole {
                    key: "connector".into(),
                    label: "Connecteur".into(),
                    entries: vec!["core.identity.act_on_behalf".into()],
                },
                Operation::Bind {
                    local: None,
                    subject: BindingSubject::Identity { id: account },
                    role: "connector".into(),
                    scope: BindingScope::Global,
                },
            ],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(bound, Outcome::Applied { .. }), "{bound:?}");
    account
}

fn journal(pool: &PgPool) -> PgAuditJournal {
    PgAuditJournal::new(pool.clone(), Instance::new("test"))
}

// REQ-016-12, REQ-016-13
#[kartenn_testing::pg_test]
async fn a_service_account_writes_on_behalf_of_a_requester(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let account = connector(&pool, &store).await;
    let sa = principal(account, IdentityKind::ServiceAccount);
    let id = act_on_behalf(
        &store,
        &journal(&pool),
        &sa,
        &requester(),
        Source::default(),
    )
    .await
    .unwrap();
    // Unknown until now: an identity `requester`, recorded once.
    let kind: String = sqlx::query_scalar("SELECT kind FROM person WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kind, "requester");
    let again = act_on_behalf(
        &store,
        &journal(&pool),
        &sa,
        &requester(),
        Source::default(),
    )
    .await
    .unwrap();
    assert_eq!(again, id);
    let created: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.event WHERE name = 'authn.requester.created'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(created, 1);
    // A write for the requester: the changeset and its events name it.
    let mut context = sa.context(Source::default());
    context.on_behalf_of = Some(id);
    let outcome = submit_in(
        &store,
        &Unrestricted,
        &Sources::none(),
        &context,
        &request(
            account,
            vec![Operation::CreateDomain {
                local: None,
                slug: "tickets".into(),
            }],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let Outcome::Applied { changeset, .. } = outcome else {
        panic!("expected applied, got {outcome:?}");
    };
    let on_behalf: Option<Uuid> =
        sqlx::query_scalar("SELECT on_behalf_of FROM changeset WHERE id = $1")
            .bind(changeset)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(on_behalf, Some(id));
    let events: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT to_jsonb(e) FROM audit.event e WHERE changeset_id = $1 ORDER BY seq",
    )
    .bind(changeset)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!events.is_empty());
    let schema = jsonschema::draft202012::new(&event_schema()).expect("a valid schema");
    for event in &events {
        assert_eq!(event["details"]["on_behalf_of"], id.to_string(), "{event}");
        let errors: Vec<String> = schema.iter_errors(event).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{errors:?}");
    }
}

#[kartenn_testing::pg_test]
async fn without_the_permission_or_as_a_person_it_is_refused(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let bare = identity(&pool, "service_account").await;
    let refused = act_on_behalf(
        &store,
        &journal(&pool),
        &principal(bare, IdentityKind::ServiceAccount),
        &requester(),
        Source::default(),
    )
    .await;
    assert!(matches!(refused, Err(AuthnError::Refused)), "{refused:?}");
    // A person, even one holding the permission, never names a requester.
    let account = connector(&pool, &store).await;
    sqlx::query("UPDATE person SET kind = 'person' WHERE id = $1")
        .bind(account)
        .execute(&pool)
        .await
        .unwrap();
    let person = act_on_behalf(
        &store,
        &journal(&pool),
        &principal(account, IdentityKind::Person),
        &requester(),
        Source::default(),
    )
    .await;
    assert!(matches!(person, Err(AuthnError::Refused)), "{person:?}");
    let requesters: i64 =
        sqlx::query_scalar("SELECT count(*) FROM person WHERE kind = 'requester'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(requesters, 0);
    // Both attempts are recorded (INV-60).
    let denied: Vec<(Option<Uuid>, Option<String>)> = sqlx::query_as(
        "SELECT actor_id, details->>'permission' FROM audit.event
         WHERE name = 'authz.access.denied' ORDER BY seq",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let permission = Some("core.identity.act_on_behalf".to_owned());
    assert_eq!(
        denied,
        [
            (Some(bare), permission.clone()),
            (Some(account), permission)
        ]
    );
}

// REQ-016-12, INV-60: the requester a service account named is a placeholder, never a
// reservation; the person who then logs in through the issuer takes the row over.
#[kartenn_testing::pg_test]
async fn the_requester_who_logs_in_becomes_a_person_under_the_issuer_name(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let account = connector(&pool, &store).await;
    let sa = principal(account, IdentityKind::ServiceAccount);
    let named = Requester {
        sub: "jdoe".into(),
        iss: Some("https://idp.example".into()),
        name: Some("A name the connector chose".into()),
    };
    let id = act_on_behalf(&store, &journal(&pool), &sa, &named, Source::default())
        .await
        .unwrap();
    // A requester is not an account: no binding names it.
    let bind = |by: Uuid| {
        request(
            by,
            vec![Operation::Bind {
                local: None,
                subject: BindingSubject::Identity { id },
                role: "connector".into(),
                scope: BindingScope::Global,
            }],
        )
    };
    let refused = submit(
        &store,
        &Unrestricted,
        &bind(identity(&pool, "person").await),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    match refused {
        Outcome::Refused { violations, .. } => {
            assert_eq!(violations[0].rule.id, "core.binding.requester");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    // The real person logs in through the issuer: the same row, now a person, named by
    // the issuer.
    let (logged, disabled) = store
        .upsert_person("https://idp.example", "jdoe", "Jane Doe")
        .await
        .unwrap();
    assert_eq!(logged, id);
    assert!(!disabled);
    let (kind, name): (String, String) =
        sqlx::query_as("SELECT kind, display_name FROM person WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((kind.as_str(), name.as_str()), ("person", "Jane Doe"));
    // Its rights are those of its bindings, as any person's: none until bound, then the
    // role's.
    let subject = kartenn_service::authz::Subject::from(id);
    assert!(
        store
            .rights(&subject)
            .await
            .unwrap()
            .global_permissions
            .is_empty()
    );
    let bound = submit(
        &store,
        &Unrestricted,
        &bind(identity(&pool, "person").await),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(bound, Outcome::Applied { .. }), "{bound:?}");
    assert!(
        store
            .rights(&subject)
            .await
            .unwrap()
            .global_permissions
            .iter()
            .any(|p| p == "core.identity.act_on_behalf")
    );
    // A connector naming it again leaves it a person under the issuer's name.
    let again = act_on_behalf(&store, &journal(&pool), &sa, &named, Source::default())
        .await
        .unwrap();
    assert_eq!(again, id);
    let (kind, name): (String, String) =
        sqlx::query_as("SELECT kind, display_name FROM person WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((kind.as_str(), name.as_str()), ("person", "Jane Doe"));
}

#[kartenn_testing::pg_test]
async fn the_requester_lends_no_right(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let account = connector(&pool, &store).await;
    let sa = principal(account, IdentityKind::ServiceAccount);
    let id = act_on_behalf(
        &store,
        &journal(&pool),
        &sa,
        &requester(),
        Source::default(),
    )
    .await
    .unwrap();
    // The requester may do everything, the service account nothing: refused as without
    // the header.
    let rights = Grants::new().everywhere(id, "*");
    let mut context = sa.context(Source::default());
    context.on_behalf_of = Some(id);
    let outcome = submit_in(
        &store,
        &rights,
        &Sources::none(),
        &context,
        &request(
            account,
            vec![Operation::CreateDomain {
                local: None,
                slug: "denied".into(),
            }],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Refused { .. }), "{outcome:?}");
}

#[kartenn_testing::pg_test]
async fn the_requester_is_pseudonymised_like_a_person(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let account = connector(&pool, &store).await;
    let sa = principal(account, IdentityKind::ServiceAccount);
    let id = act_on_behalf(
        &store,
        &journal(&pool),
        &sa,
        &requester(),
        Source::default(),
    )
    .await
    .unwrap();
    kartenn_store_pg::audit::pseudonymise_identity(
        &pool,
        id,
        &RequestContext::system(),
        "test",
        &Instance::new("test"),
    )
    .await
    .unwrap();
    let (name, issuer, subject): (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT display_name, issuer, subject FROM person WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!name.contains("Doe"), "{name}");
    assert_eq!((issuer, subject), (None, None));
    // The audit names it by its identifier only.
    let clear: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.event e WHERE to_jsonb(e)::text ~ 'jdoe|J\\. Doe'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(clear, 0);
}
