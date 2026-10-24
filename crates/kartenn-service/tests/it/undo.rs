//! Inverse changesets (spec 004, REQ-004-34, 35; task 004-20).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{
    BindingScope, BindingSubject, DeclarationInput, PeriodSpec, RawInput, SecretRecord, TypeInput,
};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::RbacRow;
use kartenn_service::undo::{RbacConflict, inverse};
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    domain: Uuid,
    network: Uuid,
    address: Uuid,
    env: Uuid,
}

impl Fixture {
    /// Domain `lab`, network `10.1.2.0/24` with the host `10.1.2.10`, `custom.environment`
    /// (`prod`, `dev`, not inherited) assigned `prod` on the network.
    async fn new(pool: &PgPool) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let mut f = Self {
            pool: pool.clone(),
            store: PgStore::new(pool.clone()),
            author,
            domain: Uuid::nil(),
            network: Uuid::nil(),
            address: Uuid::nil(),
            env: Uuid::nil(),
        };
        f.applied(vec![
            Operation::DeclareProperty {
                local: Some(1),
                declaration: DeclarationInput {
                    key: "custom.environment".into(),
                    value_type: TypeInput::ENUM,
                    cardinality: Cardinality::Single,
                    carriers: vec!["prefix".into(), "address".into()],
                    inherit: None,
                    module: None,
                    obligations: Vec::new(),
                    expected: None,
                    labels: None,
                },
            },
            Operation::AddEnumValue {
                local: None,
                property: Ref::Local(1),
                key: "prod".into(),
                labels: std::collections::BTreeMap::new(),
            },
            Operation::AddEnumValue {
                local: None,
                property: Ref::Local(1),
                key: "dev".into(),
                labels: std::collections::BTreeMap::new(),
            },
            Operation::CreateDomain {
                local: Some(2),
                slug: "lab".into(),
            },
            Operation::CreatePrefix {
                local: Some(3),
                domain: Ref::Local(2),
                prefix: "10.1.2.0/24".into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
            Operation::CreateAddress {
                local: None,
                domain: Ref::Local(2),
                address: "10.1.2.10".into(),
                usage: AddressUsage::Host,
            },
        ])
        .await;
        f.domain = f.id("SELECT id FROM addressing_domain").await;
        f.network = f.id("SELECT id FROM prefix").await;
        f.address = f.id("SELECT id FROM address").await;
        f.env = f.id("SELECT id FROM property_declaration").await;
        f.applied(vec![f.assign(f.network, "prod")]).await;
        f
    }

    async fn id(&self, sql: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn run(&self, request: &ChangesetRequest) -> Outcome {
        submit(&self.store, &Unrestricted, request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>) -> Uuid {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            apply_own: true,
        };
        match self.run(&request).await {
            Outcome::Applied { changeset, .. } => changeset,
            other => panic!("expected applied, got {other:?}"),
        }
    }

    fn assign(&self, object: Uuid, value: &str) -> Operation {
        Operation::AssignProperty {
            object: Ref::Id(object),
            kind: ObjectKind::Prefix,
            property: Ref::Id(self.env),
            values: vec![RawInput::Text(value.into())],
        }
    }

    fn usage(&self, usage: AddressUsage) -> Operation {
        Operation::UpdateAddress {
            id: Ref::Id(self.address),
            domain: None,
            usage,
            expected: None,
        }
    }

    async fn current_usage(&self) -> String {
        sqlx::query_scalar("SELECT usage FROM address WHERE valid_period @> now()")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn environment(&self) -> Option<String> {
        sqlx::query_scalar(
            "SELECT e.key FROM property_assignment a JOIN property_enum_value e ON e.id = a.v_uuid
             WHERE a.object_id = $1 AND a.valid_period @> now()",
        )
        .bind(self.network)
        .fetch_optional(&self.pool)
        .await
        .unwrap()
    }
}

#[kartenn_testing::pg_test]
async fn an_inverse_restores_what_a_changeset_changed(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let changed = f
        .applied(vec![
            Operation::CreatePrefix {
                local: None,
                domain: Ref::Id(f.domain),
                prefix: "10.0.0.0/8".into(),
                nature: Nature::Container,
                point_to_point: false,
                anycast: false,
            },
            f.usage(AddressUsage::Reserved),
            f.assign(f.network, "dev"),
        ])
        .await;
    assert_eq!(f.current_usage().await, "reserved");
    assert_eq!(f.environment().await.as_deref(), Some("dev"));
    // REQ-004-34: generated, not submitted.
    let undo = inverse(&f.store, changed, f.author).await.unwrap().unwrap();
    assert!(undo.conflicts.is_empty(), "{:?}", undo.conflicts);
    assert_eq!(
        undo.request.operations.len(),
        3,
        "{:?}",
        undo.request.operations
    );
    assert_eq!(f.current_usage().await, "reserved");
    match f.run(&undo.request).await {
        Outcome::Applied { .. } => {}
        other => panic!("{other:?}"),
    }
    assert_eq!(f.current_usage().await, "host");
    assert_eq!(f.environment().await.as_deref(), Some("prod"));
    let containers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM prefix WHERE nature = 'container' AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(containers, 0);
    // Not an applied changeset: nothing to invert.
    assert!(
        inverse(&f.store, Uuid::now_v7(), f.author)
            .await
            .unwrap()
            .is_none()
    );
}

#[kartenn_testing::pg_test]
async fn a_later_change_is_a_conflict_not_overwritten(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let changed = f
        .applied(vec![
            f.usage(AddressUsage::Reserved),
            f.assign(f.network, "dev"),
        ])
        .await;
    let later = f.applied(vec![f.usage(AddressUsage::Vip)]).await;
    // REQ-004-35: the address is left alone, the assignment is restored.
    let undo = inverse(&f.store, changed, f.author).await.unwrap().unwrap();
    assert_eq!(undo.conflicts.len(), 1, "{:?}", undo.conflicts);
    assert_eq!(undo.conflicts[0].id, f.address);
    assert_eq!(undo.conflicts[0].by, later);
    assert_eq!(undo.request.operations.len(), 1);
    assert!(matches!(
        f.run(&undo.request).await,
        Outcome::Applied { .. }
    ));
    assert_eq!(f.current_usage().await, "vip");
    assert_eq!(f.environment().await.as_deref(), Some("prod"));
    // Declarations are not undone, only listed.
    let declared = f
        .applied(vec![Operation::DeclareProperty {
            local: None,
            declaration: DeclarationInput {
                key: "custom.note".into(),
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
        }])
        .await;
    let undo = inverse(&f.store, declared, f.author)
        .await
        .unwrap()
        .unwrap();
    assert!(undo.request.operations.is_empty());
    assert_eq!(undo.unsupported.len(), 1);
    assert_eq!(undo.unsupported[0].0, "property");
}

const IDP: &str = "https://idp.example.invalid";

impl Fixture {
    /// The label and entries of the role `key` in force now.
    async fn role_now(&self, key: &str) -> Option<(String, Vec<String>)> {
        sqlx::query_as(
            "SELECT r.labels->>'fr', array(SELECT p.permission FROM role_permission p
                                   WHERE p.role_id = r.id AND p.valid_period @> now()
                                   ORDER BY 1)
             FROM role r WHERE r.key = $1 AND r.valid_period @> now()",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .unwrap()
    }

    /// The bindings in force now and later: subject kind, role key, scope kind, start.
    async fn bindings(&self) -> Vec<(String, String, String, bool)> {
        sqlx::query_as(
            "SELECT b.subject_kind, r.key, b.scope_kind, lower(b.valid_period) > now()
             FROM binding b JOIN role r ON r.id = b.role_id AND r.valid_period @> now()
             WHERE upper_inf(b.valid_period) ORDER BY 1, 3",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    async fn attachments(&self) -> i64 {
        sqlx::query_scalar(
            "SELECT count(*) FROM issuer_attachment WHERE valid_period && tstzrange(now(), NULL)",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// Computes the inverse of `changeset`, applies it, returns it.
    async fn undone(&self, changeset: Uuid) -> kartenn_service::undo::Inverse {
        let undo = inverse(&self.store, changeset, self.author)
            .await
            .unwrap()
            .unwrap();
        match self.run(&undo.request).await {
            Outcome::Applied { .. } => undo,
            other => panic!("expected the inverse applied, got {other:?}"),
        }
    }
}

fn auditor(label: &str, entries: &[&str]) -> Operation {
    Operation::SetRole {
        key: "auditor".into(),
        labels: [("fr".into(), label.into())].into(),
        entries: entries.iter().map(|&e| e.to_owned()).collect(),
    }
}

fn bind(subject: BindingSubject, scope: BindingScope) -> Operation {
    Operation::Bind {
        local: None,
        subject,
        role: "auditor".into(),
        scope,
    }
}

#[kartenn_testing::pg_test]
async fn an_administration_changeset_is_undone(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let administered = f
        .applied(vec![
            auditor("Auditor", &["core.audit.read"]),
            bind(
                BindingSubject::Identity { id: f.author },
                BindingScope::Domain {
                    id: Ref::Id(f.domain),
                },
            ),
            Operation::AttachIssuer {
                issuer: IDP.into(),
                entity: None,
            },
            Operation::CreateServiceAccount {
                local: None,
                name: "backup".into(),
                secret: SecretRecord {
                    id: Uuid::now_v7(),
                    digest: "ab".repeat(32),
                    key_id: "k1".into(),
                    expires_at: None,
                },
            },
        ])
        .await;
    assert_eq!(f.bindings().await.len(), 1);
    let undo = f.undone(administered).await;
    // Removals first, then the role, so that closing it finds nothing left to end.
    let ops: Vec<&str> = undo
        .request
        .operations
        .iter()
        .map(|op| match op {
            Operation::Unbind { .. } => "unbind",
            Operation::DetachIssuer { .. } => "detach",
            Operation::CloseRole { .. } => "close_role",
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(ops, ["unbind", "detach", "close_role"]);
    assert_eq!(f.role_now("auditor").await, None);
    assert!(f.bindings().await.is_empty());
    assert_eq!(f.attachments().await, 0);
    // A service account keeps no history: listed, not undone.
    assert_eq!(
        undo.unsupported,
        [("create_service_account".to_owned(), administered)]
    );
}

#[kartenn_testing::pg_test]
async fn undoing_a_closure_gives_the_role_and_its_bindings_back(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        auditor("Auditor", &["core.audit.read", "core.address.read"]),
        bind(
            BindingSubject::Identity { id: f.author },
            BindingScope::Global,
        ),
        bind(
            BindingSubject::Group {
                issuer: IDP.into(),
                group: "noc".into(),
            },
            BindingScope::Domain {
                id: Ref::Id(f.domain),
            },
        ),
        Operation::AttachIssuer {
            issuer: IDP.into(),
            entity: None,
        },
    ])
    .await;
    let before = f.bindings().await;
    // A modification: the label and the entries come back.
    let changed = f
        .applied(vec![auditor("Readers", &["core.audit.read"])])
        .await;
    f.undone(changed).await;
    let role = Some((
        "Auditor".to_owned(),
        vec!["core.address.read".to_owned(), "core.audit.read".to_owned()],
    ));
    assert_eq!(f.role_now("auditor").await, role);
    // A closure: the role and both bindings come back, an attachment too.
    let closed = f
        .applied(vec![
            Operation::CloseRole {
                key: "auditor".into(),
            },
            Operation::DetachIssuer {
                issuer: IDP.into(),
                entity: None,
            },
        ])
        .await;
    assert!(f.bindings().await.is_empty());
    let undo = f.undone(closed).await;
    assert!(undo.rbac_conflicts.is_empty(), "{:?}", undo.rbac_conflicts);
    assert_eq!(f.role_now("auditor").await, role);
    assert_eq!(f.bindings().await, before);
    assert_eq!(f.attachments().await, 1);
}

#[kartenn_testing::pg_test]
async fn undoing_the_end_of_a_future_binding_gives_it_back_on_its_period(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let tomorrow = i64::try_from(
        (sqlx::types::time::OffsetDateTime::now_utc() + std::time::Duration::from_hours(24))
            .unix_timestamp_nanos()
            / 1_000,
    )
    .unwrap();
    f.applied(vec![
        auditor("Auditor", &["core.audit.read"]),
        Operation::Within {
            period: PeriodSpec {
                start: tomorrow,
                end: None,
            },
            operation: Box::new(bind(
                BindingSubject::Identity { id: f.author },
                BindingScope::Global,
            )),
        },
    ])
    .await;
    let binding: Uuid = f.id("SELECT id FROM binding").await;
    // Ending it before it starts deletes the row: its history ends at the changeset.
    let unbound = f.applied(vec![Operation::Unbind { id: binding }]).await;
    assert!(f.bindings().await.is_empty());
    let undo = f.undone(unbound).await;
    assert!(
        matches!(
            undo.request.operations.as_slice(),
            [Operation::Within { period, .. }] if period.start == tomorrow
        ),
        "{:?}",
        undo.request.operations
    );
    assert_eq!(
        f.bindings().await,
        [(
            "identity".to_owned(),
            "auditor".to_owned(),
            "global".to_owned(),
            true
        )]
    );
}

#[kartenn_testing::pg_test]
async fn a_later_administration_is_a_conflict(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let created = f
        .applied(vec![auditor("Auditor", &["core.audit.read"])])
        .await;
    // Another changeset binds the role: closing it would end that binding.
    let bound = f
        .applied(vec![bind(
            BindingSubject::Identity { id: f.author },
            BindingScope::Global,
        )])
        .await;
    let undo = inverse(&f.store, created, f.author).await.unwrap().unwrap();
    assert!(undo.request.operations.is_empty());
    assert_eq!(
        undo.rbac_conflicts,
        [RbacConflict {
            row: RbacRow::Role {
                key: "auditor".into()
            },
            by: bound,
        }]
    );
    // Another changeset ends the binding: undoing its creation leaves it alone.
    let binding: Uuid = f.id("SELECT id FROM binding").await;
    let unbound = f.applied(vec![Operation::Unbind { id: binding }]).await;
    let undo = inverse(&f.store, bound, f.author).await.unwrap().unwrap();
    assert!(undo.request.operations.is_empty());
    assert_eq!(
        undo.rbac_conflicts,
        [RbacConflict {
            row: RbacRow::Binding { id: binding },
            by: unbound,
        }]
    );
}
