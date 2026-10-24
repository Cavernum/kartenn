//! Closures in cascade (spec 004, REQ-004-25, 26; task 004-15).

use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

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

    async fn submit(&self, operations: Vec<Operation>, simulate: bool) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate,
            idempotency_key: None,
            apply_own: true,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>) -> Vec<(ObjectKind, Uuid)> {
        match self.submit(operations, false).await {
            Outcome::Applied { cascaded, .. } => cascaded,
            other => panic!("expected applied, got {other:?}"),
        }
    }

    async fn refused(&self, operations: Vec<Operation>) -> Vec<&'static str> {
        match self.submit(operations, false).await {
            Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
            other => panic!("expected refused, got {other:?}"),
        }
    }

    async fn id(&self, sql: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn current(&self, table: &str) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM {table} WHERE valid_period @> now()"
        )))
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn close(kind: ObjectKind, id: Uuid, cascade: bool) -> Operation {
    Operation::Close {
        kind,
        id: Ref::Id(id),
        expected: None,
        cascade,
    }
}

/// Domain `lab`, network `10.1.2.0/24` with 3 ranges and 40 addresses.
async fn network(f: &Fixture) -> Uuid {
    let mut operations = vec![
        Operation::CreateDomain {
            local: Some(1),
            slug: "lab".into(),
        },
        Operation::CreatePrefix {
            local: Some(2),
            domain: Ref::Local(1),
            prefix: "10.1.2.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
    ];
    for r in 1..=3 {
        operations.push(Operation::CreateRange {
            local: None,
            domain: Ref::Local(1),
            first: format!("10.1.2.{}", r * 10),
            last: format!("10.1.2.{}", r * 10 + 9),
            usage: RangeUsage::Static,
        });
    }
    for a in 100..140 {
        operations.push(Operation::CreateAddress {
            local: None,
            domain: Ref::Local(1),
            address: format!("10.1.2.{a}"),
            usage: AddressUsage::Host,
        });
    }
    f.applied(operations).await;
    f.id("SELECT id FROM prefix").await
}

#[kartenn_testing::pg_test]
async fn a_network_closes_with_its_ranges_and_addresses(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let net = network(&f).await;
    // REQ-004-25: without cascade, refused, listing the dependents.
    let refused = f.refused(vec![close(ObjectKind::Prefix, net, false)]).await;
    assert!(
        refused.contains(&"core.address.outside_network"),
        "{refused:?}"
    );
    assert!(
        refused.contains(&"core.range.outside_network"),
        "{refused:?}"
    );
    // The simulation tells what the cascade adds.
    match f
        .submit(vec![close(ObjectKind::Prefix, net, true)], true)
        .await
    {
        Outcome::Simulated {
            violations,
            cascaded,
            ..
        } => {
            assert!(violations.is_empty(), "{violations:?}");
            assert_eq!(cascaded.len(), 43);
        }
        other => panic!("{other:?}"),
    }
    // REQ-004-26: 44 closures.
    let cascaded = f.applied(vec![close(ObjectKind::Prefix, net, true)]).await;
    assert_eq!(cascaded.len(), 43);
    assert_eq!(
        cascaded
            .iter()
            .filter(|(k, _)| *k == ObjectKind::Range)
            .count(),
        3
    );
    for table in ["prefix", "ip_range", "address"] {
        assert_eq!(f.current(table).await, 0, "{table}");
    }
}

#[kartenn_testing::pg_test]
async fn entity_trees_and_references_follow(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        Operation::CreateEntity {
            local: Some(1),
            slug: "group".into(),
            parent: None,
        },
        Operation::CreateEntity {
            local: Some(2),
            slug: "branch".into(),
            parent: Some(Ref::Local(1)),
        },
        Operation::CreateEntity {
            local: None,
            slug: "team".into(),
            parent: Some(Ref::Local(2)),
        },
    ])
    .await;
    let group = f.id("SELECT id FROM entity WHERE slug = 'group'").await;
    assert_eq!(
        f.refused(vec![close(ObjectKind::Entity, group, false)])
            .await,
        vec!["core.anchor.in_use"]
    );
    let cascaded = f
        .applied(vec![close(ObjectKind::Entity, group, true)])
        .await;
    // Deepest first: the team, then the branch.
    let team = f.id("SELECT id FROM entity WHERE slug = 'team'").await;
    let branch = f.id("SELECT id FROM entity WHERE slug = 'branch'").await;
    assert_eq!(
        cascaded,
        vec![(ObjectKind::Entity, team), (ObjectKind::Entity, branch)]
    );
    assert_eq!(f.current("entity").await, 0);

    // A reference to a closed address is removed with it.
    let net = network(&f).await;
    f.applied(vec![Operation::DeclareProperty {
        local: None,
        declaration: DeclarationInput {
            key: "custom.gateway".into(),
            value_type: TypeInput::Ref {
                target: "address".into(),
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
    let gateway = f
        .id("SELECT id FROM property_declaration WHERE key = 'custom.gateway'")
        .await;
    let address = f
        .id("SELECT id FROM address WHERE address = '10.1.2.100'")
        .await;
    f.applied(vec![Operation::AssignProperty {
        object: Ref::Id(net),
        kind: ObjectKind::Prefix,
        property: Ref::Id(gateway),
        values: vec![RawInput::Text(address.to_string())],
    }])
    .await;
    assert_eq!(
        f.refused(vec![close(ObjectKind::Address, address, false)])
            .await,
        vec!["prop.assignment.dangling_reference"]
    );
    f.applied(vec![close(ObjectKind::Address, address, true)])
        .await;
    let left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM property_assignment WHERE property_id = $1 AND valid_period @> now()",
    )
    .bind(gateway)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(left, 0);
}
