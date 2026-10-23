//! Filtering objects by resolved value (spec 003, REQ-003-25; task 003-15).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{EXTERNAL_SYSTEM, core_declarations, sync_modules};
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::filter::{Condition, FilterError, filter, filter_plan};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    domain: Uuid,
    env: Uuid,
    floor: Uuid,
}

fn declaration(
    key: &str,
    value_type: TypeInput,
    cardinality: Cardinality,
    carriers: &[&str],
    heritable: bool,
) -> DeclarationInput {
    DeclarationInput {
        key: key.into(),
        value_type,
        cardinality,
        carriers: carriers.iter().map(|c| (*c).to_owned()).collect(),
        inherit: heritable.then(|| "addressing".into()),
        module: None,
        obligations: Vec::new(),
        expected: None,
        labels: None,
    }
}

/// `custom.environment` as local 1 with its values, `custom.floor` as local 2.
fn declarations() -> Vec<Operation> {
    let value = |key: &str| Operation::AddEnumValue {
        local: None,
        property: Ref::Local(1),
        key: key.into(),
        labels: std::collections::BTreeMap::new(),
    };
    vec![
        Operation::DeclareProperty {
            local: Some(1),
            declaration: declaration(
                "custom.environment",
                TypeInput::ENUM,
                Cardinality::Set,
                &["prefix", "address"],
                true,
            ),
        },
        value("prod"),
        value("dev"),
        value("rec"),
        Operation::DeclareProperty {
            local: Some(2),
            declaration: declaration(
                "custom.floor",
                TypeInput::Int { min: 0, max: 99 },
                Cardinality::Single,
                &["address"],
                false,
            ),
        },
    ]
}

impl Fixture {
    /// `custom.environment` (set of `prod`, `dev`, `rec`, heritable, prefixes and
    /// addresses) and `custom.floor` (integer, addresses only); `10.0.0.0/8` in `{prod}`,
    /// network `10.1.0.0/24` in `{dev, rec}` with `10.1.0.1` (floor 3), network
    /// `10.2.0.0/24` without value with `10.2.0.1` (floor 7).
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
            env: Uuid::nil(),
            floor: Uuid::nil(),
        };
        let prefix = |local, value: &str, nature| Operation::CreatePrefix {
            local: Some(local),
            domain: Ref::Local(3),
            prefix: value.into(),
            nature,
            point_to_point: false,
            anycast: false,
        };
        let address = |local, value: &str| Operation::CreateAddress {
            local: Some(local),
            domain: Ref::Local(3),
            address: value.into(),
            usage: AddressUsage::Host,
        };
        let assign = |object, kind, property, values: &[&str]| Operation::AssignProperty {
            object: Ref::Local(object),
            kind,
            property: Ref::Local(property),
            values: values.iter().map(|v| RawInput::Text((*v).into())).collect(),
        };
        let mut operations = declarations();
        operations.extend([
            Operation::CreateDomain {
                local: Some(3),
                slug: "prod".into(),
            },
            prefix(4, "10.0.0.0/8", Nature::Container),
            prefix(5, "10.1.0.0/24", Nature::Network),
            prefix(6, "10.2.0.0/24", Nature::Network),
            address(7, "10.1.0.1"),
            address(8, "10.2.0.1"),
            assign(4, ObjectKind::Prefix, 1, &["prod"]),
            assign(5, ObjectKind::Prefix, 1, &["dev", "rec"]),
            Operation::AssignProperty {
                object: Ref::Local(7),
                kind: ObjectKind::Address,
                property: Ref::Local(2),
                values: vec![RawInput::Int(3)],
            },
            Operation::AssignProperty {
                object: Ref::Local(8),
                kind: ObjectKind::Address,
                property: Ref::Local(2),
                values: vec![RawInput::Int(7)],
            },
        ]);
        f.applied(operations).await;
        f.domain = sqlx::query_scalar("SELECT id FROM addressing_domain")
            .fetch_one(pool)
            .await
            .unwrap();
        let property = |key: &'static str| {
            sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = $1")
                .bind(key)
                .fetch_one(pool)
        };
        f.env = property("custom.environment").await.unwrap();
        f.floor = property("custom.floor").await.unwrap();
        f
    }

    async fn applied(&self, operations: Vec<Operation>) {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        let outcome = submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap();
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    async fn id(&self, value: &str) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM addressing_object WHERE value = $1::inet AND kind <> 'domain'",
        )
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn ids(&self, values: &[&str]) -> Vec<Uuid> {
        let mut out = Vec::new();
        for v in values {
            out.push(self.id(v).await);
        }
        out.sort();
        out
    }

    async fn find(&self, property: Uuid, condition: Condition) -> Vec<Uuid> {
        filter(
            &self.pool,
            property,
            self.domain,
            &condition,
            OffsetDateTime::now_utc(),
            None,
            100,
        )
        .await
        .unwrap()
    }
}

fn key(k: &str) -> String {
    k.to_owned()
}

#[kartenn_testing::pg_test]
async fn objects_are_found_by_resolved_value(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // Equality on a set: the sets containing the value, inherited ones included.
    assert_eq!(
        f.find(f.env, Condition::Equals(key("prod"))).await,
        f.ids(&["10.0.0.0/8", "10.2.0.0/24", "10.2.0.1"]).await
    );
    assert_eq!(
        f.find(f.env, Condition::Equals(key("rec"))).await,
        f.ids(&["10.1.0.0/24", "10.1.0.1"]).await
    );
    // Membership: one of the values.
    assert_eq!(
        f.find(f.env, Condition::OneOf(vec![key("prod"), key("rec")]))
            .await
            .len(),
        5
    );
    // Interval on integers, open bounds included.
    assert_eq!(
        f.find(f.floor, Condition::Between(Some(key("2")), Some(key("5"))))
            .await,
        f.ids(&["10.1.0.1"]).await
    );
    assert_eq!(
        f.find(f.floor, Condition::Between(None, Some(key("10"))))
            .await
            .len(),
        2
    );
    // Presence; absence counts explicit "no value" as absence.
    assert_eq!(f.find(f.env, Condition::Present).await.len(), 5);
    assert!(f.find(f.env, Condition::Absent).await.is_empty());
    let net = f.id("10.2.0.0/24").await;
    f.applied(vec![Operation::ClearProperty {
        object: Ref::Id(net),
        kind: ObjectKind::Prefix,
        property: Ref::Id(f.env),
    }])
    .await;
    assert_eq!(
        f.find(f.env, Condition::Absent).await,
        f.ids(&["10.2.0.0/24", "10.2.0.1"]).await
    );
    // A condition that does not fit the type is refused.
    assert!(matches!(
        filter(
            &pool,
            f.env,
            f.domain,
            &Condition::Between(None, None),
            OffsetDateTime::now_utc(),
            None,
            10
        )
        .await,
        Err(FilterError::Unsupported(_))
    ));
}

#[kartenn_testing::pg_test]
async fn pages_follow_a_cursor_and_origins_use_their_index(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let now = OffsetDateTime::now_utc();
    let page = |after| filter(&pool, f.env, f.domain, &Condition::Present, now, after, 2);
    let first = page(None).await.unwrap();
    let second = page(first.last().copied()).await.unwrap();
    let third = page(second.last().copied()).await.unwrap();
    assert_eq!((first.len(), second.len(), third.len()), (2, 2, 1));
    let mut all: Vec<Uuid> = first.into_iter().chain(second).chain(third).collect();
    all.dedup();
    assert_eq!(all.len(), 5);
    // With a realistic volume (4 000 addresses inheriting `{dev}` from a /20), a selective
    // heritable filter reaches the objects from their origins through the index.
    f.applied(vec![Operation::CreatePrefix {
        local: None,
        domain: Ref::Id(f.domain),
        prefix: "10.3.0.0/20".into(),
        nature: Nature::Network,
        point_to_point: false,
        anycast: false,
    }])
    .await;
    let net3 = f.id("10.3.0.0/20").await;
    sqlx::query(
        "INSERT INTO address (id, domain_id, address, usage, public, registry_version, valid_period, changeset_id)
         SELECT gen_random_uuid(), $1, '10.3.0.0'::inet + g, 'host', false, '2025-10-09',
                tstzrange(now() - interval '1 day', NULL), gen_random_uuid()
         FROM generate_series(1, 4000) g",
    )
    .bind(f.domain)
    .execute(&pool)
    .await
    .unwrap();
    f.applied(vec![Operation::AssignProperty {
        object: Ref::Id(net3),
        kind: ObjectKind::Prefix,
        property: Ref::Id(f.env),
        values: vec![RawInput::Text("dev".into())],
    }])
    .await;
    sqlx::raw_sql("ANALYZE").execute(&pool).await.unwrap();
    let now = OffsetDateTime::now_utc();
    assert_eq!(
        f.find(f.env, Condition::Equals(key("rec"))).await,
        f.ids(&["10.1.0.0/24", "10.1.0.1"]).await
    );
    let plan = filter_plan(&pool, f.env, f.domain, &Condition::Equals(key("rec")), now)
        .await
        .unwrap();
    assert!(plan.contains("property_origin_by_origin"), "{plan}");
}

fn external(system: &str, id: &str, label: Option<&str>) -> RawInput {
    RawInput::External {
        system: system.into(),
        id: id.into(),
        label: label.map(Into::into),
    }
}

#[kartenn_testing::pg_test]
async fn external_references_filter_by_system_and_identifier(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // The list of the systems, as the core declares it (the fixture's domain has no
    // owner, which the rest of the core would require).
    let mut core = core_declarations();
    core.properties
        .retain(|(d, _)| d.key.to_string() == EXTERNAL_SYSTEM);
    let synced = sync_modules(&f.store, &Unrestricted, &[core], MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(
        matches!(synced[..], [Outcome::Applied { .. }]),
        "{synced:?}"
    );
    let systems: Uuid = sqlx::query_scalar(
        "SELECT id FROM property_declaration WHERE key = 'core.external_system'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let system = |key: &str| Operation::AddEnumValue {
        local: None,
        property: Ref::Id(systems),
        key: key.into(),
        labels: std::collections::BTreeMap::new(),
    };
    f.applied(vec![
        system("cmdb"),
        system("ticketing"),
        system("retired"),
        Operation::DeclareProperty {
            local: Some(1),
            declaration: declaration(
                "custom.ci",
                TypeInput::ExternalRef,
                Cardinality::Single,
                &["prefix", "address"],
                false,
            ),
        },
        Operation::DeclareProperty {
            local: Some(2),
            declaration: declaration(
                "custom.tickets",
                TypeInput::ExternalRef,
                Cardinality::Set,
                &["address"],
                false,
            ),
        },
    ])
    .await;
    let property = |key: &'static str| {
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = $1")
            .bind(key)
            .fetch_one(&f.pool)
    };
    let (ci, tickets): (Uuid, Uuid) = (
        property("custom.ci").await.unwrap(),
        property("custom.tickets").await.unwrap(),
    );
    let assign = |object: Uuid, kind, property, values| Operation::AssignProperty {
        object: Ref::Id(object),
        kind,
        property: Ref::Id(property),
        values,
    };
    let (net1, net2, host1, host2) = (
        f.id("10.1.0.0/24").await,
        f.id("10.2.0.0/24").await,
        f.id("10.1.0.1").await,
        f.id("10.2.0.1").await,
    );
    f.applied(vec![
        assign(
            net1,
            ObjectKind::Prefix,
            ci,
            vec![external("cmdb", "CI-42", Some("Paie"))],
        ),
        assign(
            net2,
            ObjectKind::Prefix,
            ci,
            vec![external("cmdb", "CI-42", None)],
        ),
        assign(
            host2,
            ObjectKind::Address,
            ci,
            vec![external("ticketing", "CI-42", None)],
        ),
        assign(
            host1,
            ObjectKind::Address,
            tickets,
            vec![
                external("ticketing", "T-1", None),
                external("ticketing", "T-2", Some("Ouverture")),
            ],
        ),
    ])
    .await;
    // REQ-016-3: equal by system and identifier, whatever the label; another system
    // differs; a set holding the reference.
    let mut same = vec![net1, net2];
    same.sort();
    assert_eq!(f.find(ci, Condition::Equals(key("cmdb:CI-42"))).await, same);
    assert_eq!(
        f.find(ci, Condition::Equals(key("ticketing:CI-42"))).await,
        vec![host2]
    );
    assert!(
        f.find(ci, Condition::Equals(key("cmdb:CI-7")))
            .await
            .is_empty()
    );
    assert_eq!(
        f.find(tickets, Condition::Equals(key("ticketing:T-2")))
            .await,
        vec![host1]
    );
    // A system named by a reference stays open; a closed one is refused at writing.
    let value = |key: &'static str| {
        sqlx::query_scalar("SELECT id FROM property_enum_value WHERE property_id = $1 AND key = $2")
            .bind(systems)
            .bind(key)
            .fetch_one(&f.pool)
    };
    let close = |id: Uuid| Operation::Close {
        kind: ObjectKind::EnumValue,
        id: Ref::Id(id),
        expected: None,
        cascade: false,
    };
    let refused = |operations: Vec<Operation>| async {
        let request = ChangesetRequest {
            author: f.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        match submit(&f.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
        {
            Outcome::Refused { violations, .. } => {
                violations.iter().map(|v| v.rule.id).collect::<Vec<_>>()
            }
            other => panic!("expected refused, got {other:?}"),
        }
    };
    assert_eq!(
        refused(vec![close(value("ticketing").await.unwrap())]).await,
        vec!["prop.declaration.incompatible"]
    );
    f.applied(vec![close(value("retired").await.unwrap())])
        .await;
    assert_eq!(
        refused(vec![assign(
            net1,
            ObjectKind::Prefix,
            ci,
            vec![external("retired", "CI-1", None)]
        )])
        .await,
        vec!["prop.assignment.invalid_value"]
    );
}
