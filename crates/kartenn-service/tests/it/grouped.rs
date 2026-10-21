//! Grouped read of resolved values (spec 016, REQ-016-1, 6; task 016-4): the same
//! answers as the explanation one by one, the objects the reader may not see left out.

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::period::Instant;
use kartenn_core::values::Cardinality;
use kartenn_service::audit::{MemoryJournal, RequestContext};
use kartenn_service::authz::{Grants, Unrestricted};
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::reads::resolved_many;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::explain::{self, explain};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

const KEYS: [&str; 4] = [
    "custom.environment",
    "custom.floor",
    "custom.zone",
    "custom.missing",
];

fn declaration(
    key: &str,
    value_type: TypeInput,
    carriers: &[&str],
    inherit: Option<&str>,
) -> DeclarationInput {
    DeclarationInput {
        key: key.into(),
        value_type,
        cardinality: Cardinality::Single,
        carriers: carriers.iter().map(|c| (*c).to_owned()).collect(),
        inherit: inherit.map(Into::into),
        module: None,
        obligations: Vec::new(),
        expected: None,
    }
}

struct Fixture {
    store: PgStore,
    /// Every object of the data set.
    objects: Vec<Uuid>,
    addresses: Vec<Uuid>,
}

impl Fixture {
    /// `custom.environment` (closed list, addressing tree): `10.0.0.0/8` in `prod`,
    /// `10.1.0.0/24` in `dev`, `10.2.0.0/24` without value, 20 addresses in each network;
    /// `custom.floor` (integer, addresses, not inherited) on half of them; `custom.zone`
    /// (text, entity tree) on the root of `corp` → `lab` → `bench`.
    #[allow(clippy::too_many_lines)] // the data set, written out
    async fn new(pool: &PgPool) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let store = PgStore::new(pool.clone());
        let text = |values: &[&str]| -> Vec<RawInput> {
            values.iter().map(|v| RawInput::Text((*v).into())).collect()
        };
        let mut operations = vec![
            Operation::DeclareProperty {
                local: Some(1),
                declaration: declaration(
                    KEYS[0],
                    TypeInput::ENUM,
                    &["domain", "prefix", "address"],
                    Some("addressing"),
                ),
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
            Operation::DeclareProperty {
                local: Some(2),
                declaration: declaration(
                    KEYS[1],
                    TypeInput::Int { min: 0, max: 99 },
                    &["address"],
                    None,
                ),
            },
            Operation::DeclareProperty {
                local: Some(3),
                declaration: declaration(
                    KEYS[2],
                    TypeInput::Text {
                        min_len: 1,
                        max_len: 20,
                        pattern: None,
                    },
                    &["entity"],
                    Some("entity"),
                ),
            },
            Operation::CreateDomain {
                local: Some(4),
                slug: "prod".into(),
            },
            Operation::CreateEntity {
                local: Some(5),
                slug: "corp".into(),
                parent: None,
            },
            Operation::CreateEntity {
                local: Some(6),
                slug: "lab".into(),
                parent: Some(Ref::Local(5)),
            },
            Operation::CreateEntity {
                local: Some(7),
                slug: "bench".into(),
                parent: Some(Ref::Local(6)),
            },
            Operation::AssignProperty {
                object: Ref::Local(5),
                kind: ObjectKind::Entity,
                property: Ref::Local(3),
                values: text(&["nord"]),
            },
        ];
        for (local, prefix, nature, value) in [
            (8, "10.0.0.0/8", Nature::Container, Some("prod")),
            (9, "10.1.0.0/24", Nature::Network, Some("dev")),
            (10, "10.2.0.0/24", Nature::Network, None),
        ] {
            operations.push(Operation::CreatePrefix {
                local: Some(local),
                domain: Ref::Local(4),
                prefix: prefix.into(),
                nature,
                point_to_point: false,
                anycast: false,
            });
            if let Some(value) = value {
                operations.push(Operation::AssignProperty {
                    object: Ref::Local(local),
                    kind: ObjectKind::Prefix,
                    property: Ref::Local(1),
                    values: text(&[value]),
                });
            }
        }
        let mut local = 100;
        for net in [1, 2] {
            for host in 1..=20 {
                local += 1;
                operations.push(Operation::CreateAddress {
                    local: Some(local),
                    domain: Ref::Local(4),
                    address: format!("10.{net}.0.{host}"),
                    usage: AddressUsage::Host,
                });
                if host % 2 == 0 {
                    operations.push(Operation::AssignProperty {
                        object: Ref::Local(local),
                        kind: ObjectKind::Address,
                        property: Ref::Local(2),
                        values: vec![RawInput::Int(host)],
                    });
                }
            }
        }
        let request = ChangesetRequest {
            author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        let outcome = submit(&store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap();
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        let ids = |sql: &'static str| sqlx::query_scalar::<_, Uuid>(sql).fetch_all(pool);
        let addresses = ids("SELECT id FROM address ORDER BY address")
            .await
            .unwrap();
        let mut objects = ids(
            "SELECT id FROM addressing_domain UNION ALL SELECT id FROM prefix
             UNION ALL SELECT id FROM entity",
        )
        .await
        .unwrap();
        objects.extend(&addresses);
        Self {
            store,
            objects,
            addresses,
        }
    }
}

fn instant(at: OffsetDateTime) -> Instant {
    Instant(i64::try_from(at.unix_timestamp_nanos() / 1_000).unwrap())
}

#[kartenn_testing::pg_test]
async fn resolved_many_matches_explain_on_random_pairs(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let keys: Vec<String> = KEYS.iter().map(|k| (*k).to_owned()).collect();
    let now = OffsetDateTime::now_utc();
    let all = explain::resolved_many(&pool, &f.objects, &keys, now, None)
        .await
        .unwrap();
    // One row per object and declared key, in the order asked.
    assert_eq!(all.len(), f.objects.len() * 3);
    assert_eq!(all[0].object, f.objects[0]);
    assert_eq!(all[0].property, KEYS[0]);
    let property = |key: &str| {
        let pool = pool.clone();
        let key = key.to_owned();
        async move {
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM property_declaration WHERE key = $1")
                .bind(key)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let ids = [
        property(KEYS[0]).await,
        property(KEYS[1]).await,
        property(KEYS[2]).await,
    ];
    // REQ-016-1: 1 000 pairs drawn at random give the answer of the explanation.
    let mut seed: u64 = 0x5eed;
    let mut draw = |n: usize| {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(seed >> 33).unwrap() % n
    };
    let mut inherited = 0;
    for _ in 0..1_000 {
        let (object, k) = (f.objects[draw(f.objects.len())], draw(3));
        let one = explain(&pool, object, ids[k], now, None)
            .await
            .unwrap()
            .unwrap();
        let grouped = all
            .iter()
            .find(|r| r.object == object && r.property == KEYS[k])
            .unwrap();
        assert_eq!(
            (
                &grouped.value,
                grouped.origin,
                &grouped.chain,
                &grouped.provenance
            ),
            (&one.value, one.origin, &one.chain, &one.provenance),
            "{object} {}",
            KEYS[k]
        );
        if one.chain.len() > 1 {
            inherited += 1;
        }
    }
    assert!(inherited > 100, "{inherited} inherited pairs only");
}

#[kartenn_testing::pg_test]
async fn an_unreadable_object_is_absent_without_error(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let reader = Uuid::now_v7();
    // The reader sees the first five addresses only.
    let grants = Grants::new().on(reader, "core.address.read", f.addresses[..5].to_vec());
    let keys = vec![KEYS[0].to_owned(), KEYS[1].to_owned()];
    let found = resolved_many(
        &f.store,
        &grants,
        &MemoryJournal::new(),
        &RequestContext::person(reader),
        &f.addresses,
        &keys,
        instant(OffsetDateTime::now_utc()),
        None,
    )
    .await
    .unwrap();
    let objects: std::collections::BTreeSet<Uuid> = found.iter().map(|r| r.object).collect();
    assert_eq!(objects, f.addresses[..5].iter().copied().collect());
    assert_eq!(found.len(), 10);
    // The system reads everything.
    let all = resolved_many(
        &f.store,
        &grants,
        &MemoryJournal::new(),
        &RequestContext::system(),
        &f.addresses,
        &keys,
        instant(OffsetDateTime::now_utc()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(all.len(), f.addresses.len() * 2);
}
