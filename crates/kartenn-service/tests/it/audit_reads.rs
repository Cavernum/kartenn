//! Audited reads, sensitive properties and the consultation of the audit journal (spec
//! 012, REQ-012-17, 18; spec 003, REQ-003-27; tasks 012-7, 003-19).

use std::future::Future;

use kartenn_core::audit::AuditEvent;
use kartenn_core::values::Cardinality;
use kartenn_service::audit::{AuditJournal, MemoryJournal, RequestContext, now};
use kartenn_service::authz::{Grants, Unrestricted};
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::AuditQuery;
use kartenn_service::reads::{AUDIT_READ_PERMISSION, ReadAudit, read_audit, read_object};
use kartenn_service::{
    ChangesetRequest, ObjectKind, Operation, Outcome, Ref, RequestError, StoreError, submit,
};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

fn text(local: u32, key: &str) -> Operation {
    Operation::DeclareProperty {
        local: Some(local),
        declaration: DeclarationInput {
            key: key.into(),
            value_type: TypeInput::Text {
                min_len: 0,
                max_len: 40,
                pattern: None,
            },
            cardinality: Cardinality::Single,
            carriers: vec!["prefix".into()],
            inherit: None,
            module: None,
            obligations: Vec::new(),
            expected: None,
            labels: None,
        },
    }
}

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
}

impl Fixture {
    async fn new(pool: &PgPool) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        Self {
            pool: pool.clone(),
            store: PgStore::new(pool.clone()),
            author,
        }
    }

    async fn applied(&self, operations: Vec<Operation>) {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        let outcome = submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap();
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    async fn id(&self, sql: &'static str) -> Uuid {
        sqlx::query_scalar(sql).fetch_one(&self.pool).await.unwrap()
    }
}

/// A journal that cannot record.
struct Broken;

impl AuditJournal for Broken {
    fn record(&self, _: &AuditEvent) -> impl Future<Output = Result<(), StoreError>> + Send {
        std::future::ready(Err(StoreError("audit unavailable".into())))
    }
}

#[kartenn_testing::pg_test]
async fn reads_are_audited_sensitive_ones_longer(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        text(1, "custom.contact"),
        text(2, "custom.note"),
        Operation::CreateDomain {
            local: Some(3),
            slug: "lab".into(),
        },
        Operation::CreatePrefix {
            local: Some(4),
            domain: Ref::Local(3),
            prefix: "10.1.0.0/16".into(),
            nature: kartenn_core::objects::Nature::Network,
            point_to_point: false,
            anycast: false,
        },
    ])
    .await;
    let prefix = f.id("SELECT id FROM prefix").await;
    let contact = f
        .id("SELECT id FROM property_declaration WHERE key = 'custom.contact'")
        .await;
    let note = f
        .id("SELECT id FROM property_declaration WHERE key = 'custom.note'")
        .await;
    let assign = |property, value: &str| Operation::AssignProperty {
        object: Ref::Id(prefix),
        kind: ObjectKind::Prefix,
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.into())],
    };
    f.applied(vec![
        assign(contact, "noc@example.net"),
        assign(note, "lab"),
    ])
    .await;
    let mark = |sensitive| Operation::SetSensitivity {
        property: Ref::Id(contact),
        sensitive,
    };
    f.applied(vec![mark(true)]).await;
    // REQ-003-27: the change of sensitivity is audited with its changeset.
    let flag: serde_json::Value = sqlx::query_scalar(
        "SELECT details -> 'sensitive' FROM audit.event WHERE name = 'property.declaration.changed'
         ORDER BY seq DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(flag, serde_json::json!(true));

    let journal = MemoryJournal::new();
    let reader = RequestContext::person(f.author);
    let read = read_object(
        &f.store,
        &Unrestricted,
        &journal,
        &reader,
        prefix,
        now(),
        None,
        ReadAudit::default(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        read.properties["custom.contact"],
        Some(serde_json::json!("noc@example.net"))
    );
    let events = journal.events();
    let names: Vec<(&str, &str)> = events
        .iter()
        .map(|e| (e.kind.name, e.kind.criticality.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("data.objects.read", "low"),
            ("data.sensitive.read", "high")
        ]
    );
    assert_eq!(events[0].target.as_ref().unwrap().ids, vec![prefix]);
    assert_eq!(events[1].details["property"], "custom.contact");
    assert!(
        !serde_json::to_string(&events.iter().map(AuditEvent::to_json).collect::<Vec<_>>())
            .unwrap()
            .contains("noc@example.net"),
        "never the values"
    );

    // Unmarked: only the ordinary read.
    f.applied(vec![mark(false)]).await;
    let journal = MemoryJournal::new();
    read_object(
        &f.store,
        &Unrestricted,
        &journal,
        &reader,
        prefix,
        now(),
        None,
        ReadAudit::default(),
    )
    .await
    .unwrap();
    assert_eq!(journal.events().len(), 1);

    // REQ-012-11: a read that cannot be audited returns nothing.
    assert!(matches!(
        read_object(
            &f.store,
            &Unrestricted,
            &Broken,
            &reader,
            prefix,
            now(),
            None,
            ReadAudit::default()
        )
        .await,
        Err(RequestError::Audit(_))
    ));
}

#[kartenn_testing::pg_test]
async fn consulting_the_audit_needs_its_permission_and_is_audited(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![Operation::CreateDomain {
        local: None,
        slug: "lab".into(),
    }])
    .await;
    let journal = kartenn_store_pg::audit_journal::PgAuditJournal::new(
        pool.clone(),
        kartenn_service::audit::Instance::new("test"),
    );
    let reader = RequestContext::person(f.author);
    let query = AuditQuery {
        name: Some("changeset.state.applied".into()),
        limit: 10,
        ..AuditQuery::default()
    };
    assert!(matches!(
        read_audit(&f.store, &Grants::new(), &journal, &reader, &query).await,
        Err(RequestError::Forbidden(p)) if p == AUDIT_READ_PERMISSION
    ));
    let auditor = Grants::new().everywhere(f.author, AUDIT_READ_PERMISSION);
    let (found, more) = read_audit(&f.store, &auditor, &journal, &reader, &query)
        .await
        .unwrap();
    assert!(!more);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["name"], "changeset.state.applied");
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM audit.event ORDER BY seq")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(
        names,
        [
            "changeset.state.pending",
            "changeset.state.applied",
            "authz.access.denied",
            "audit.events.read"
        ]
    );
}
