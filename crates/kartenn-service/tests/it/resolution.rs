//! Assignments and the materialised origins of heritable properties (spec 003,
//! REQ-003-4, 8, 11 to 13, 15; task 003-8): examples per requirement, then the
//! materialisation checked against the naive resolver (the oracle) on random sequences of
//! dated changesets.

use std::collections::{BTreeMap, HashMap};

use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::family::ALL;
use kartenn_core::inheritance::{AddressingTree, Assigned, resolve};
use kartenn_core::objects::{Address, AddressUsage, IpRange, Nature, PrefixObject, RangeUsage};
use kartenn_core::values::{Cardinality, Scalar, Value};
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

fn day(d: i64) -> i64 {
    T0 + d * DAY
}

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    domain: Uuid,
    property: Uuid,
}

impl Fixture {
    /// Domain `prod` and `custom.environment` (closed list `a`, `b`), heritable along the
    /// addressing tree, carried by every kind of the tree; created on day 0.
    async fn new(pool: &PgPool, store: PgStore) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let mut f = Self {
            pool: pool.clone(),
            store,
            author,
            domain: Uuid::nil(),
            property: Uuid::nil(),
        };
        let declaration = DeclarationInput {
            key: "custom.environment".into(),
            value_type: TypeInput::ENUM,
            cardinality: Cardinality::Single,
            carriers: ["domain", "prefix", "range", "address"]
                .map(String::from)
                .to_vec(),
            inherit: Some("addressing".into()),
            module: None,
            obligations: Vec::new(),
            expected: None,
        };
        let outcome = f
            .submit(
                vec![
                    Operation::CreateDomain {
                        local: None,
                        slug: "prod".into(),
                    },
                    Operation::DeclareProperty {
                        local: Some(1),
                        declaration,
                    },
                    Operation::AddEnumValue {
                        local: None,
                        property: Ref::Local(1),
                        key: "a".into(),
                    },
                    Operation::AddEnumValue {
                        local: None,
                        property: Ref::Local(1),
                        key: "b".into(),
                    },
                ],
                0,
                None,
            )
            .await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        f.domain = sqlx::query_scalar("SELECT id FROM addressing_domain")
            .fetch_one(pool)
            .await
            .unwrap();
        f.property = sqlx::query_scalar("SELECT id FROM property_declaration")
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
                start: day(start),
                end: end.map(day),
            }),
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>, start: i64, end: Option<i64>) {
        let outcome = self.submit(operations, start, end).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
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

    fn assign(&self, object: Uuid, kind: ObjectKind, value: &str) -> Operation {
        Operation::AssignProperty {
            object: Ref::Id(object),
            kind,
            property: Ref::Id(self.property),
            values: vec![RawInput::Text(value.into())],
        }
    }

    async fn id(&self, table: &str, column: &str, value: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE {column} = $1::inet LIMIT 1"
        )))
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// Stored origin of `object` at day `d`.
    async fn origin(&self, object: Uuid, d: i64) -> Option<Uuid> {
        sqlx::query_scalar(
            "SELECT origin_object_id FROM property_origin
             WHERE object_id = $1 AND property_id = $2 AND valid_period @> $3",
        )
        .bind(object)
        .bind(self.property)
        .bind(instant(day(d)))
        .fetch_optional(&self.pool)
        .await
        .unwrap()
    }
}

fn instant(micros: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(micros) * 1_000).unwrap()
}

#[kartenn_testing::pg_test]
async fn values_are_inherited_replaced_cleared_and_dated(pool: PgPool) {
    let f = Fixture::new(&pool, PgStore::new(pool.clone())).await;
    f.applied(
        vec![
            f.prefix("10.0.0.0/8", Nature::Container),
            f.prefix("10.1.2.0/24", Nature::Network),
            f.address("10.1.2.3"),
        ],
        0,
        None,
    )
    .await;
    let root = f.id("prefix", "prefix", "10.0.0.0/8").await;
    let net = f.id("prefix", "prefix", "10.1.2.0/24").await;
    let addr = f.id("address", "address", "10.1.2.3").await;

    // REQ-003-11: assigned on the container, inherited by the address.
    f.applied(vec![f.assign(root, ObjectKind::Prefix, "a")], 1, None)
        .await;
    assert_eq!(f.origin(addr, 0).await, None);
    assert_eq!(f.origin(addr, 1).await, Some(root));
    assert_eq!(f.origin(net, 1).await, Some(root));
    // REQ-003-12: the network's own assignment replaces it below.
    f.applied(vec![f.assign(net, ObjectKind::Prefix, "b")], 2, None)
        .await;
    assert_eq!(f.origin(addr, 2).await, Some(net));
    assert_eq!(f.origin(addr, 1).await, Some(root), "the past is kept");
    // REQ-003-13: explicit "no value" also stops inheritance.
    f.applied(
        vec![Operation::ClearProperty {
            object: Ref::Id(net),
            kind: ObjectKind::Prefix,
            property: Ref::Id(f.property),
        }],
        3,
        None,
    )
    .await;
    assert_eq!(f.origin(addr, 3).await, Some(net));
    let kind: String = sqlx::query_scalar(
        "SELECT kind FROM property_assignment WHERE object_id = $1 AND valid_period @> $2",
    )
    .bind(net)
    .bind(instant(day(3)))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind, "cleared");
    // Removing the assignment: the address inherits from the container again.
    f.applied(
        vec![Operation::UnassignProperty {
            object: Ref::Id(net),
            kind: ObjectKind::Prefix,
            property: Ref::Id(f.property),
        }],
        4,
        None,
    )
    .await;
    assert_eq!(f.origin(addr, 4).await, Some(root));
    // REQ-003-15: an intermediate container created on day 5 with its own value changes
    // the resolution from day 5 on, not before.
    f.applied(vec![f.prefix("10.1.0.0/16", Nature::Container)], 5, None)
        .await;
    let mid = f.id("prefix", "prefix", "10.1.0.0/16").await;
    f.applied(vec![f.assign(mid, ObjectKind::Prefix, "b")], 5, None)
        .await;
    assert_eq!(f.origin(addr, 4).await, Some(root));
    assert_eq!(f.origin(addr, 5).await, Some(mid));
    // Closing the intermediate container: its inheritors look further up again.
    f.applied(
        vec![Operation::Close {
            kind: ObjectKind::Prefix,
            id: Ref::Id(mid),
            expected: None,
            cascade: false,
        }],
        6,
        None,
    )
    .await;
    assert_eq!(f.origin(addr, 6).await, Some(root));
    assert_eq!(f.origin(mid, 6).await, None);
    assert_eq!(f.origin(mid, 5).await, Some(mid));
}

#[kartenn_testing::pg_test]
async fn assignments_respect_carriers_types_and_existence(pool: PgPool) {
    let f = Fixture::new(&pool, PgStore::new(pool.clone())).await;
    f.applied(vec![f.prefix("10.0.0.0/8", Nature::Container)], 0, None)
        .await;
    let root = f.id("prefix", "prefix", "10.0.0.0/8").await;
    let ids = |outcome: Outcome| match outcome {
        Outcome::Refused { violations, .. } => {
            violations.iter().map(|v| v.rule.id).collect::<Vec<_>>()
        }
        other => panic!("expected refused, got {other:?}"),
    };
    // REQ-003-8: a key outside the closed list.
    assert_eq!(
        ids(f
            .submit(vec![f.assign(root, ObjectKind::Prefix, "c")], 1, None)
            .await),
        vec!["prop.assignment.invalid_value"]
    );
    // The object must exist over the whole period, with the kind given.
    assert_eq!(
        ids(f
            .submit(vec![f.assign(root, ObjectKind::Address, "a")], 1, None)
            .await),
        vec!["changeset.operation.no_version"]
    );
    // REQ-003-4: a property not carried by the kind.
    f.applied(
        vec![Operation::DeclareProperty {
            local: None,
            declaration: DeclarationInput {
                key: "custom.label".into(),
                value_type: TypeInput::Text {
                    min_len: 0,
                    max_len: 10,
                    pattern: None,
                },
                cardinality: Cardinality::Single,
                carriers: vec!["address".into()],
                inherit: None,
                module: None,
                obligations: Vec::new(),
                expected: None,
            },
        }],
        0,
        None,
    )
    .await;
    let label: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'custom.label'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        ids(f
            .submit(
                vec![Operation::AssignProperty {
                    object: Ref::Id(root),
                    kind: ObjectKind::Prefix,
                    property: Ref::Id(label),
                    values: vec![RawInput::Text("x".into())],
                }],
                1,
                None
            )
            .await),
        vec!["prop.assignment.not_carrier"]
    );
    // A non-heritable property is not materialised.
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM property_origin WHERE property_id = $1")
            .bind(label)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 0);
}

/// A small deterministic generator (xorshift64*), to replay a failing seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[usize::try_from(self.below(items.len() as u64)).unwrap()]
    }
}

const CONTAINERS: [&str; 3] = ["10.0.0.0/8", "10.0.0.0/16", "10.1.0.0/16"];
const NETWORKS: [&str; 3] = ["10.0.0.0/24", "10.0.1.0/24", "10.1.0.0/24"];
const ADDRESSES: [&str; 4] = ["10.0.0.1", "10.0.0.2", "10.0.1.1", "10.1.0.1"];
const RANGES: [(&str, &str); 2] = [("10.0.0.10", "10.0.0.19"), ("10.0.1.5", "10.0.1.9")];

/// Objects of the domain with their kind, whatever their period, in an order that does not
/// depend on the identifiers or the plan (so that a seed replays).
async fn objects(pool: &PgPool) -> Vec<(Uuid, ObjectKind)> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, kind FROM addressing_object WHERE kind <> 'domain'
         GROUP BY id, kind ORDER BY kind, min(value), min(lower(valid_period))",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter()
        .map(|(id, kind)| {
            let kind = match kind.as_str() {
                "prefix" => ObjectKind::Prefix,
                "range" => ObjectKind::Range,
                _ => ObjectKind::Address,
            };
            (id, kind)
        })
        .collect()
}

/// The oracle at instant `t`: origin of every object of the tree.
async fn oracle(f: &Fixture, t: OffsetDateTime) -> BTreeMap<Uuid, Option<Uuid>> {
    let prefixes: Vec<(Uuid, String, String)> =
        sqlx::query_as("SELECT id, prefix::text, nature FROM prefix WHERE valid_period @> $1")
            .bind(t)
            .fetch_all(&f.pool)
            .await
            .unwrap();
    let addresses: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, host(address) FROM address WHERE valid_period @> $1")
            .bind(t)
            .fetch_all(&f.pool)
            .await
            .unwrap();
    let assigned: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT object_id, kind FROM property_assignment WHERE property_id = $1 AND valid_period @> $2",
    )
    .bind(f.property)
    .bind(t)
    .fetch_all(&f.pool)
    .await
    .unwrap();
    let prefix_objects: Vec<PrefixObject> = prefixes
        .iter()
        .map(|(id, value, nature)| PrefixObject {
            id: *id,
            domain: f.domain,
            value: Prefix::parse_any(&ALL, value).unwrap(),
            nature: if nature == "network" {
                Nature::Network
            } else {
                Nature::Container
            },
            point_to_point: false,
            anycast: false,
        })
        .collect();
    let address_objects: Vec<Address> = addresses
        .iter()
        .map(|(id, value)| Address {
            id: *id,
            domain: f.domain,
            value: Addr::parse_any(&ALL, value).unwrap(),
            usage: AddressUsage::Host,
        })
        .collect();
    let ranges: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, host(lower(range)), host(upper(range)) FROM ip_range WHERE valid_period @> $1",
    )
    .bind(t)
    .fetch_all(&f.pool)
    .await
    .unwrap();
    let range_objects: Vec<IpRange> = ranges
        .iter()
        .map(|(id, first, last)| IpRange {
            id: *id,
            domain: f.domain,
            first: Addr::parse_any(&ALL, first).unwrap(),
            last: Addr::parse_any(&ALL, last).unwrap(),
            usage: RangeUsage::Static,
        })
        .collect();
    let tree = AddressingTree::new(prefix_objects, &range_objects, &address_objects);
    let assignments: HashMap<Uuid, Assigned> = assigned
        .into_iter()
        .map(|(id, kind)| {
            let a = if kind == "cleared" {
                Assigned::Cleared
            } else {
                Assigned::Value(Value::One(Scalar::Int(0)))
            };
            (id, a)
        })
        .collect();
    std::iter::once(f.domain)
        .chain(prefixes.iter().map(|p| p.0))
        .chain(addresses.iter().map(|a| a.0))
        .chain(ranges.iter().map(|r| r.0))
        .map(|o| (o, resolve(&tree, &assignments, o, true).origin))
        .collect()
}

/// The materialisation at instant `t`.
async fn stored(f: &Fixture, t: OffsetDateTime) -> BTreeMap<Uuid, Option<Uuid>> {
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT object_id, origin_object_id FROM property_origin WHERE property_id = $1 AND valid_period @> $2",
    )
    .bind(f.property)
    .bind(t)
    .fetch_all(&f.pool)
    .await
    .unwrap();
    rows.into_iter().map(|(o, g)| (o, Some(g))).collect()
}

/// One random operation on the object pool, with a random period.
async fn random_step(f: &Fixture, rng: &mut Rng) -> (Vec<Operation>, i64, Option<i64>) {
    let start = i64::try_from(rng.below(10)).unwrap();
    let end = (rng.below(3) == 0).then(|| start + 1 + i64::try_from(rng.below(4)).unwrap());
    let pool = objects(&f.pool).await;
    let any = |rng: &mut Rng| -> Option<(Uuid, ObjectKind)> {
        let mut choices = pool.clone();
        choices.push((f.domain, ObjectKind::Domain));
        Some(*rng.pick(&choices))
    };
    let op = match rng.below(10) {
        9 => {
            let (first, last) = *rng.pick(&RANGES);
            Operation::CreateRange {
                local: None,
                domain: Ref::Id(f.domain),
                first: first.into(),
                last: last.into(),
                usage: RangeUsage::Static,
            }
        }
        0 => f.prefix(rng.pick(&CONTAINERS), Nature::Container),
        1 => f.prefix(rng.pick(&NETWORKS), Nature::Network),
        2 => f.address(rng.pick(&ADDRESSES)),
        3 | 4 => {
            let (o, k) = any(rng).unwrap();
            f.assign(o, k, rng.pick(&["a", "b"]))
        }
        5 => {
            let (o, k) = any(rng).unwrap();
            Operation::ClearProperty {
                object: Ref::Id(o),
                kind: k,
                property: Ref::Id(f.property),
            }
        }
        6 => {
            let (o, k) = any(rng).unwrap();
            Operation::UnassignProperty {
                object: Ref::Id(o),
                kind: k,
                property: Ref::Id(f.property),
            }
        }
        _ if pool.is_empty() => f.prefix(rng.pick(&CONTAINERS), Nature::Container),
        _ => {
            let (o, k) = *rng.pick(&pool);
            Operation::Close {
                kind: k,
                id: Ref::Id(o),
                expected: None,
                cascade: false,
            }
        }
    };
    (vec![op], start, end)
}

#[kartenn_testing::pg_test]
async fn the_materialisation_matches_the_oracle(pool: PgPool) {
    matches_the_oracle(&pool, PgStore::new(pool.clone())).await;
}

/// The same, the origins being recomputed going down from the carriers of an assignment
/// (the way of the large changesets).
#[kartenn_testing::pg_test]
async fn going_down_matches_the_oracle(pool: PgPool) {
    matches_the_oracle(&pool, PgStore::new(pool.clone()).with_down_from(0)).await;
}

async fn matches_the_oracle(pool: &PgPool, store: PgStore) {
    let f = Fixture::new(pool, store).await;
    for seed in 1..=4_u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let mut applied = 0;
        for step in 0..40 {
            let (operations, start, end) = random_step(&f, &mut rng).await;
            let outcome = f.submit(operations.clone(), start, end).await;
            if matches!(outcome, Outcome::Applied { .. }) {
                applied += 1;
            }
            // Every half day of the horizon, boundaries included.
            for half in 0..30 {
                let t = instant(day(0) + i64::from(half) * DAY / 2);
                let expected: BTreeMap<Uuid, Option<Uuid>> = oracle(&f, t)
                    .await
                    .into_iter()
                    .filter(|(_, origin)| origin.is_some())
                    .collect();
                assert_eq!(
                    stored(&f, t).await,
                    expected,
                    "seed {seed}, step {step} ({operations:?} from day {start} to {end:?}), at {t}"
                );
            }
        }
        assert!(
            applied > 5,
            "seed {seed}: too few changesets applied ({applied})"
        );
        // Segments are maximal: no two touching rows with the same origin.
        let touching: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM property_origin a JOIN property_origin b
               ON a.object_id = b.object_id AND a.property_id = b.property_id
              AND upper(a.valid_period) = lower(b.valid_period)
              AND a.origin_object_id = b.origin_object_id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(touching, 0, "seed {seed}");
    }
}
