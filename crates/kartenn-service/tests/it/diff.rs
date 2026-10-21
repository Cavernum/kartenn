//! Differential of resolved values and simulation (spec 003, REQ-003-19, 20; task 003-9).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{DeclarationInput, PeriodSpec, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::DiffCount;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::audit::diff_page;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

/// 2030-01-01T00:00Z in microseconds; the tests live on days from there.
const T0: i64 = 1_893_456_000_000_000;
const DAY: i64 = 86_400_000_000;

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    domain: Uuid,
    env: Uuid,
}

impl Fixture {
    /// Domain `prod`, `custom.environment` (set of `prod`, `dev`, heritable along the
    /// addressing tree, on prefixes and addresses), container `10.0.0.0/8` and network
    /// `10.1.0.0/19`, all from day 0.
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
        };
        let declaration = DeclarationInput {
            key: "custom.environment".into(),
            value_type: TypeInput::Enum,
            cardinality: Cardinality::Set,
            carriers: vec!["prefix".into(), "address".into()],
            inherit: Some("addressing".into()),
            module: None,
            obligations: Vec::new(),
            expected: None,
        };
        f.applied(
            vec![
                Operation::CreateDomain {
                    local: Some(1),
                    slug: "prod".into(),
                },
                Operation::DeclareProperty {
                    local: Some(2),
                    declaration,
                },
                Operation::AddEnumValue {
                    local: None,
                    property: Ref::Local(2),
                    key: "prod".into(),
                },
                Operation::AddEnumValue {
                    local: None,
                    property: Ref::Local(2),
                    key: "dev".into(),
                },
                Operation::CreatePrefix {
                    local: None,
                    domain: Ref::Local(1),
                    prefix: "10.0.0.0/8".into(),
                    nature: Nature::Container,
                    point_to_point: false,
                    anycast: false,
                },
                Operation::CreatePrefix {
                    local: None,
                    domain: Ref::Local(1),
                    prefix: "10.1.0.0/19".into(),
                    nature: Nature::Network,
                    point_to_point: false,
                    anycast: false,
                },
            ],
            0,
        )
        .await;
        f.domain = sqlx::query_scalar("SELECT id FROM addressing_domain")
            .fetch_one(pool)
            .await
            .unwrap();
        f.env = sqlx::query_scalar("SELECT id FROM property_declaration")
            .fetch_one(pool)
            .await
            .unwrap();
        f
    }

    fn request(&self, operations: Vec<Operation>, day: i64, simulate: bool) -> ChangesetRequest {
        ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: Some(PeriodSpec {
                start: T0 + day * DAY,
                end: None,
            }),
            operations,
            simulate,
            idempotency_key: None,
        }
    }

    async fn submit(&self, operations: Vec<Operation>, day: i64, simulate: bool) -> Outcome {
        submit(
            &self.store,
            &Unrestricted,
            &self.request(operations, day, simulate),
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>, day: i64) -> (Uuid, Vec<DiffCount>) {
        match self.submit(operations, day, false).await {
            Outcome::Applied {
                changeset, diff, ..
            } => (changeset, diff),
            other => panic!("expected applied, got {other:?}"),
        }
    }

    async fn prefix(&self, value: &str) -> Uuid {
        sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = $1::cidr LIMIT 1")
            .bind(value)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    fn assign(&self, object: Ref, kind: ObjectKind, values: &[&str]) -> Operation {
        Operation::AssignProperty {
            object,
            kind,
            property: Ref::Id(self.env),
            values: values.iter().map(|v| RawInput::Text((*v).into())).collect(),
        }
    }

    /// `n` addresses of the network `10.1.0.0/19` from day 0, written directly (the
    /// pipeline checks them one by one, which is not what this test is about).
    async fn addresses(&self, n: i32) {
        sqlx::query(
            "INSERT INTO address (id, domain_id, address, usage, public, registry_version, valid_period, changeset_id)
             SELECT gen_random_uuid(), $1, '10.1.0.0'::inet + g, 'host', false, '2025-10-09',
                    tstzrange($2, NULL), gen_random_uuid()
             FROM generate_series(1, $3) g",
        )
        .bind(self.domain)
        .bind(sqlx::types::time::OffsetDateTime::from_unix_timestamp(T0 / 1_000_000).unwrap())
        .bind(n)
        .execute(&self.pool)
        .await
        .unwrap();
    }
}

fn change(before: &str, after: &str, count: u64) -> DiffCount {
    DiffCount {
        property: "custom.environment".into(),
        before: Some(before.into()),
        after: Some(after.into()),
        count,
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_container_above_4096_addresses_changes_them_all(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.addresses(4096).await;
    let root = f.prefix("10.0.0.0/8").await;
    // Existing objects that start resolving `{prod}`: the network and its addresses; the
    // container itself carries the assignment.
    let (_, diff) = f
        .applied(
            vec![f.assign(Ref::Id(root), ObjectKind::Prefix, &["prod"])],
            1,
        )
        .await;
    assert_eq!(
        diff,
        vec![DiffCount {
            property: "custom.environment".into(),
            before: None,
            after: Some(r#"["prod"]"#.into()),
            count: 4098,
        }]
    );
    let insert = || {
        vec![
            Operation::CreatePrefix {
                local: Some(1),
                domain: Ref::Id(f.domain),
                prefix: "10.1.0.0/16".into(),
                nature: Nature::Container,
                point_to_point: false,
                anycast: false,
            },
            f.assign(Ref::Local(1), ObjectKind::Prefix, &["dev"]),
        ]
    };
    // REQ-003-20: the simulation tells the effect and writes nothing.
    let Outcome::Simulated {
        violations,
        diff,
        sample,
        ..
    } = f.submit(insert(), 2, true).await
    else {
        panic!("expected a simulation")
    };
    assert!(violations.is_empty(), "{violations:?}");
    assert_eq!(diff, vec![change(r#"["prod"]"#, r#"["dev"]"#, 4097)]);
    assert_eq!(sample.len(), 100);
    let written: i64 =
        sqlx::query_scalar("SELECT count(*) FROM prefix WHERE prefix = '10.1.0.0/16'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(written, 0);

    // REQ-003-19: applied, counted in the result, detailed by pages; the new container is
    // not part of it, the 4 096 addresses and their network are.
    let (changeset, diff) = f.applied(insert(), 2).await;
    assert_eq!(diff, vec![change(r#"["prod"]"#, r#"["dev"]"#, 4097)]);
    let mut seen = Vec::new();
    let mut after = None;
    loop {
        let page = diff_page(&pool, changeset, Some("custom.environment"), after, 1000)
            .await
            .unwrap();
        let Some((last, _)) = page.last() else { break };
        after = Some(*last);
        seen.extend(page.into_iter().map(|(_, item)| item.object));
    }
    assert_eq!(seen.len(), 4097);
    let addresses: i64 = sqlx::query_scalar("SELECT count(*) FROM address WHERE id = ANY($1)")
        .bind(&seen)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(addresses, 4096);
    let summary: serde_json::Value =
        sqlx::query_scalar("SELECT diff_summary FROM changeset WHERE id = $1")
            .bind(changeset)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(summary[0]["count"], 4097);
    // REQ-004-36: its event carries the same summarised differential.
    let event: serde_json::Value = sqlx::query_scalar(
        "SELECT payload -> 'data' -> 'diff' FROM outbox WHERE changeset_id = $1",
    )
    .bind(changeset)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event, summary);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn every_effect_in_the_window_counts_and_only_existing_objects(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.addresses(2).await;
    let root = f.prefix("10.0.0.0/8").await;
    let net = f.prefix("10.1.0.0/19").await;
    f.applied(
        vec![f.assign(Ref::Id(root), ObjectKind::Prefix, &["prod"])],
        1,
    )
    .await;
    // A value change on the origin: the origin and every inheritor change value (root,
    // network, two addresses); no origin moves.
    let (changeset, diff) = f
        .applied(
            vec![f.assign(Ref::Id(root), ObjectKind::Prefix, &["prod", "dev"])],
            2,
        )
        .await;
    assert_eq!(diff, vec![change(r#"["prod"]"#, r#"["dev", "prod"]"#, 4)]);
    let moved: i64 =
        sqlx::query_scalar("SELECT count(*) FROM property_origin WHERE changeset_id = $1")
            .bind(changeset)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(moved, 0, "a value change rewrites no origin");
    // A new address inherits, but is not an existing object whose value changes.
    let (_, diff) = f
        .applied(
            vec![Operation::CreateAddress {
                local: None,
                domain: Ref::Id(f.domain),
                address: "10.1.1.1".into(),
                usage: AddressUsage::Host,
            }],
            3,
        )
        .await;
    assert!(diff.is_empty(), "{diff:?}");
    // The network gets its own value from day 6 on; a changeset from day 4 that removes the
    // root's value changes the addresses on days 4 and 5 only: still in the window.
    f.applied(
        vec![f.assign(Ref::Id(net), ObjectKind::Prefix, &["dev"])],
        6,
    )
    .await;
    let (_, diff) = f
        .applied(
            vec![Operation::UnassignProperty {
                object: Ref::Id(root),
                kind: ObjectKind::Prefix,
                property: Ref::Id(f.env),
            }],
            4,
        )
        .await;
    let total: u64 = diff.iter().map(|d| d.count).sum();
    assert_eq!(total, 5, "root, network and three addresses: {diff:?}");
    assert!(diff.iter().all(|d| d.after.is_none()), "{diff:?}");
}
