//! Inverse changesets (spec 004, REQ-004-34, 35; task 004-20).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::undo::inverse;
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
                    value_type: TypeInput::Enum,
                    cardinality: Cardinality::Single,
                    carriers: vec!["prefix".into(), "address".into()],
                    inherit: None,
                    module: None,
                    obligations: Vec::new(),
                    expected: None,
                },
            },
            Operation::AddEnumValue {
                local: None,
                property: Ref::Local(1),
                key: "prod".into(),
            },
            Operation::AddEnumValue {
                local: None,
                property: Ref::Local(1),
                key: "dev".into(),
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

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
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

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
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
