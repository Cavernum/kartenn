//! Modifications over a portion of time and their validation (spec 004, REQ-004-8, 14,
//! 15; spec 002, REQ-002-17, 18; task 004-8).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::period::{Instant, Period, Versions};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{DeclarationInput, PeriodSpec, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

/// 2030-01-01T00:00Z in microseconds; the tests live on days from there.
const T0: i64 = 1_893_456_000_000_000;
const DAY: i64 = 86_400_000_000;

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    domain: Uuid,
}

impl Fixture {
    /// Domain `lab` from day 0.
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
        };
        f.applied(
            vec![Operation::CreateDomain {
                local: None,
                slug: "lab".into(),
            }],
            0,
            None,
        )
        .await;
        f.domain = sqlx::query_scalar("SELECT id FROM addressing_domain")
            .fetch_one(pool)
            .await
            .unwrap();
        f
    }

    async fn submit(&self, operations: Vec<Operation>, start: i64, end: Option<i64>) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: Some(PeriodSpec {
                start: T0 + start * DAY,
                end: end.map(|e| T0 + e * DAY),
            }),
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>, start: i64, end: Option<i64>) -> Uuid {
        match self.submit(operations, start, end).await {
            Outcome::Applied { changeset, .. } => changeset,
            other => panic!("expected applied, got {other:?}"),
        }
    }

    async fn refused(
        &self,
        operations: Vec<Operation>,
        start: i64,
        end: Option<i64>,
    ) -> Vec<&'static str> {
        match self.submit(operations, start, end).await {
            Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
            other => panic!("expected refused, got {other:?}"),
        }
    }

    fn prefix(&self, value: &str, nature: Nature) -> Operation {
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Id(self.domain),
            prefix: value.into(),
            nature,
            point_to_point: false,
            anycast: false,
        }
    }

    fn address(&self, value: &str) -> Operation {
        Operation::CreateAddress {
            local: None,
            domain: Ref::Id(self.domain),
            address: value.into(),
            usage: AddressUsage::Host,
        }
    }

    async fn id(&self, table: &str, column: &str, value: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE {column}::text = $1 LIMIT 1"
        )))
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// Current versions of an address: usage, first day, last day excluded.
    async fn usages(&self, id: Uuid) -> Vec<(String, i64, Option<i64>)> {
        let rows: Vec<(String, OffsetDateTime, Option<OffsetDateTime>)> = sqlx::query_as(
            "SELECT usage, lower(valid_period), upper(valid_period) FROM address
             WHERE id = $1 ORDER BY lower(valid_period)",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .unwrap();
        rows.into_iter()
            .map(|(usage, lower, upper)| (usage, day(lower), upper.map(day)))
            .collect()
    }
}

fn day(t: OffsetDateTime) -> i64 {
    (i64::try_from(t.unix_timestamp_nanos() / 1_000).unwrap() - T0) / DAY
}

fn update_address(id: Uuid, usage: AddressUsage) -> Operation {
    Operation::UpdateAddress {
        id: Ref::Id(id),
        domain: None,
        usage,
        expected: None,
    }
}

fn update_prefix(id: Uuid, prefix: Option<&str>, nature: Option<Nature>) -> Operation {
    Operation::UpdatePrefix {
        id: Ref::Id(id),
        domain: None,
        prefix: prefix.map(Into::into),
        nature,
        point_to_point: None,
        anycast: None,
        expected: None,
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_portion_splits_and_merges_back(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(
        vec![
            f.prefix("10.1.2.0/24", Nature::Network),
            f.address("10.1.2.10"),
        ],
        0,
        None,
    )
    .await;
    let a = f.id("address", "host(address)", "10.1.2.10").await;
    // REQ-004-14: reserved from day 60 to day 150, host before and after.
    f.applied(
        vec![update_address(a, AddressUsage::Reserved)],
        60,
        Some(150),
    )
    .await;
    assert_eq!(
        f.usages(a).await,
        vec![
            ("host".into(), 0, Some(60)),
            ("reserved".into(), 60, Some(150)),
            ("host".into(), 150, None),
        ]
    );
    // The replaced version is in the history, once.
    let history: i64 = sqlx::query_scalar("SELECT count(*) FROM address_history WHERE id = $1")
        .bind(a)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(history, 1);
    // Back to host over the same portion: one version again.
    f.applied(vec![update_address(a, AddressUsage::Host)], 60, Some(150))
        .await;
    assert_eq!(f.usages(a).await, vec![("host".into(), 0, None)]);
    // Nothing to modify where the address does not exist.
    f.applied(
        vec![Operation::Close {
            kind: ObjectKind::Address,
            id: Ref::Id(a),
            expected: None,
            cascade: false,
        }],
        200,
        None,
    )
    .await;
    assert_eq!(
        f.refused(vec![update_address(a, AddressUsage::Reserved)], 180, None)
            .await,
        vec!["changeset.operation.no_version"]
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_modification_is_checked_like_a_creation(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(
        vec![
            f.prefix("10.1.2.0/24", Nature::Network),
            f.address("10.1.2.10"),
        ],
        0,
        None,
    )
    .await;
    let net = f.id("prefix", "prefix", "10.1.2.0/24").await;
    // REQ-004-8: a network planned from day 30 under a /25 created from today.
    f.applied(vec![f.prefix("10.9.0.0/24", Nature::Network)], 30, None)
        .await;
    assert!(
        f.refused(vec![f.prefix("10.9.0.0/25", Nature::Network)], 0, None)
            .await
            .contains(&"core.prefix.network_not_leaf")
    );
    // The local rules apply to the final version: a /24 cannot be point to point.
    let p2p = Operation::UpdatePrefix {
        id: Ref::Id(net),
        domain: None,
        prefix: None,
        nature: None,
        point_to_point: Some(true),
        anycast: None,
        expected: None,
    };
    assert!(
        f.refused(vec![p2p], 10, None)
            .await
            .contains(&"core.prefix.p2p_length")
    );
    // REQ-002-17: a network holding an address cannot become a container.
    assert!(
        f.refused(
            vec![update_prefix(net, None, Some(Nature::Container))],
            10,
            None
        )
        .await
        .contains(&"core.prefix.not_empty_for_nature")
    );
    // A container holding a prefix cannot become a network.
    f.applied(vec![f.prefix("10.0.0.0/8", Nature::Container)], 0, None)
        .await;
    let container = f.id("prefix", "prefix", "10.0.0.0/8").await;
    assert!(
        f.refused(
            vec![update_prefix(container, None, Some(Nature::Network))],
            10,
            None
        )
        .await
        .contains(&"core.prefix.not_empty_for_nature")
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn resizing_keeps_the_identifier_and_checks_the_content(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(
        vec![
            f.prefix("10.1.2.0/24", Nature::Network),
            f.address("10.1.2.200"),
        ],
        0,
        None,
    )
    .await;
    let net = f.id("prefix", "prefix", "10.1.2.0/24").await;
    // REQ-002-18: shrinking to /25 would leave 10.1.2.200 outside any network.
    assert!(
        f.refused(
            vec![update_prefix(net, Some("10.1.2.0/25"), None)],
            10,
            None
        )
        .await
        .contains(&"core.address.outside_network")
    );
    // Growing to /23 is accepted while 10.1.3.0/24 does not exist; same identifier.
    f.applied(
        vec![update_prefix(net, Some("10.1.2.0/23"), None)],
        10,
        None,
    )
    .await;
    assert_eq!(f.id("prefix", "prefix", "10.1.2.0/23").await, net);
    let versions: Vec<String> = sqlx::query_scalar(
        "SELECT prefix::text FROM prefix WHERE id = $1 ORDER BY lower(valid_period)",
    )
    .bind(net)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(versions, vec!["10.1.2.0/24", "10.1.2.0/23"]);
    // A neighbour network blocks growing over it.
    f.applied(vec![f.prefix("10.1.4.0/24", Nature::Network)], 0, None)
        .await;
    assert!(
        f.refused(
            vec![update_prefix(net, Some("10.1.0.0/21"), None)],
            20,
            None
        )
        .await
        .contains(&"core.prefix.network_not_leaf")
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_stale_modification_is_a_conflict(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let first = f
        .applied(
            vec![
                f.prefix("10.1.2.0/24", Nature::Network),
                f.address("10.1.2.10"),
            ],
            0,
            None,
        )
        .await;
    let a = f.id("address", "host(address)", "10.1.2.10").await;
    let seen = Operation::UpdateAddress {
        id: Ref::Id(a),
        domain: None,
        usage: AddressUsage::Vip,
        expected: Some(first),
    };
    let second = f.applied(vec![seen.clone()], 0, None).await;
    // REQ-004-11: written since `first`; the conflict names the current version.
    match f.submit(vec![seen], 0, None).await {
        Outcome::Refused { violations, .. } => {
            assert_eq!(violations.len(), 1, "{violations:?}");
            assert_eq!(violations[0].rule.id, "changeset.operation.conflict");
            assert!(
                violations[0].message.contains(&second.to_string()),
                "{}",
                violations[0].message
            );
        }
        other => panic!("expected a conflict, got {other:?}"),
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn resizing_moves_the_inheritance(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let declaration = DeclarationInput {
        key: "custom.environment".into(),
        value_type: TypeInput::Enum,
        cardinality: Cardinality::Single,
        carriers: vec!["prefix".into(), "address".into()],
        inherit: Some("addressing".into()),
        module: None,
        obligations: Vec::new(),
        expected: None,
    };
    f.applied(
        vec![
            Operation::DeclareProperty {
                local: Some(1),
                declaration,
            },
            Operation::AddEnumValue {
                local: None,
                property: Ref::Local(1),
                key: "prod".into(),
            },
            f.prefix("10.0.0.0/16", Nature::Container),
            f.prefix("10.1.0.0/24", Nature::Network),
            f.address("10.1.0.5"),
        ],
        0,
        None,
    )
    .await;
    let env: Uuid = sqlx::query_scalar("SELECT id FROM property_declaration")
        .fetch_one(&pool)
        .await
        .unwrap();
    let container = f.id("prefix", "prefix", "10.0.0.0/16").await;
    let a = f.id("address", "host(address)", "10.1.0.5").await;
    f.applied(
        vec![Operation::AssignProperty {
            object: Ref::Id(container),
            kind: ObjectKind::Prefix,
            property: Ref::Id(env),
            values: vec![RawInput::Text("prod".into())],
        }],
        0,
        None,
    )
    .await;
    let origin = || {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT origin_object_id FROM property_origin
             WHERE object_id = $1 AND property_id = $2 AND valid_period @> $3::timestamptz",
        )
        .bind(a)
        .bind(env)
        .bind(OffsetDateTime::from_unix_timestamp((T0 + 20 * DAY) / 1_000_000).unwrap())
        .fetch_optional(&pool)
    };
    assert_eq!(origin().await.unwrap(), None);
    // The container grows over 10.1.0.0/24 from day 10: the address inherits from it.
    f.applied(
        vec![update_prefix(container, Some("10.0.0.0/15"), None)],
        10,
        None,
    )
    .await;
    assert_eq!(origin().await.unwrap(), Some(container));
    // And shrinks back: the address no longer inherits.
    f.applied(
        vec![update_prefix(container, Some("10.0.0.0/16"), None)],
        15,
        None,
    )
    .await;
    assert_eq!(origin().await.unwrap(), None);
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> i64 {
        i64::try_from(self.next() % n).unwrap()
    }
}

/// The splitting in the database matches the model of task 004-3 (REQ-004-14, 15) on
/// random sequences of modifications and closures of one address.
#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_database_splits_like_the_model(pool: PgPool) {
    const USAGES: [AddressUsage; 3] = [
        AddressUsage::Host,
        AddressUsage::Reserved,
        AddressUsage::Vip,
    ];
    let f = Fixture::new(&pool).await;
    f.applied(vec![f.prefix("10.1.2.0/24", Nature::Network)], 0, None)
        .await;
    let at = |d: i64| Instant(T0 + d * DAY);
    for seed in 1..=3_u64 {
        let host = format!("10.1.2.{}", 10 + seed);
        f.applied(vec![f.address(&host)], 0, None).await;
        let a = f.id("address", "host(address)", &host).await;
        let mut model: Versions<AddressUsage> = Versions::default();
        model
            .create(Period::new(at(0), None).unwrap(), AddressUsage::Host)
            .unwrap();
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for step in 0..30 {
            let start = 1 + rng.below(40);
            let end = (rng.below(3) > 0).then(|| start + 1 + rng.below(20));
            let (operation, expected) = if rng.below(10) == 0 {
                let ok = model.close(at(start)).is_ok();
                let close = Operation::Close {
                    kind: ObjectKind::Address,
                    id: Ref::Id(a),
                    expected: None,
                    cascade: false,
                };
                (close, ok)
            } else {
                let usage = USAGES[usize::try_from(rng.below(3)).unwrap()];
                let portion = Period::new(at(start), end.map(at)).unwrap();
                let ok = model.update(portion, |_| usage).is_ok();
                (update_address(a, usage), ok)
            };
            let outcome = f.submit(vec![operation.clone()], start, end).await;
            assert_eq!(
                matches!(outcome, Outcome::Applied { .. }),
                expected,
                "seed {seed}, step {step}: {operation:?} from day {start} to {end:?}: {outcome:?}"
            );
            let wanted: Vec<(String, i64, Option<i64>)> = model
                .as_slice()
                .iter()
                .map(|(p, u)| {
                    (
                        u.as_str().to_owned(),
                        (p.start().0 - T0) / DAY,
                        p.end().map(|e| (e.0 - T0) / DAY),
                    )
                })
                .collect();
            assert_eq!(f.usages(a).await, wanted, "seed {seed}, step {step}");
        }
    }
}
