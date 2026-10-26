//! Heirs that cannot carry the property still inherit it (spec 003, REQ-003-11; design,
//! § Résolution matérialisée: a row per carrier or heir).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::explain::explain;
use kartenn_store_pg::filter::{Condition, filter};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

#[kartenn_testing::pg_test]
async fn an_address_inherits_a_value_it_cannot_carry(pool: PgPool) {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let store = PgStore::new(pool.clone());
    let run = |operations: Vec<Operation>| {
        let request = ChangesetRequest {
            author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        let store = store.clone();
        async move {
            let outcome = submit(&store, &Unrestricted, &request, MAX_OPERATIONS)
                .await
                .unwrap();
            assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        }
    };
    run(vec![
        Operation::DeclareProperty {
            local: Some(1),
            declaration: DeclarationInput {
                key: "custom.zone".into(),
                value_type: TypeInput::ENUM,
                cardinality: Cardinality::Single,
                // Prefixes only: addresses inherit without carrying.
                carriers: vec!["prefix".into()],
                inherit: Some("addressing".into()),
                module: None,
                obligations: Vec::new(),
                expected: None,
                labels: None,
            },
        },
        Operation::AddEnumValue {
            local: None,
            property: Ref::Local(1),
            key: "dmz".into(),
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
            local: Some(4),
            domain: Ref::Local(2),
            address: "10.1.2.10".into(),
            usage: AddressUsage::Host,
        },
    ])
    .await;
    let id = |sql: &'static str| sqlx::query_scalar::<_, Uuid>(sql).fetch_one(&pool);
    let zone = id("SELECT id FROM property_declaration").await.unwrap();
    let domain = id("SELECT id FROM addressing_domain").await.unwrap();
    let network = id("SELECT id FROM prefix").await.unwrap();
    let address = id("SELECT id FROM address").await.unwrap();
    let now = OffsetDateTime::now_utc();
    // Nothing yet: the address has no value, and the filter finds it absent.
    let absent = filter(&pool, zone, domain, &Condition::Absent, now, None, 100)
        .await
        .unwrap();
    assert!(absent.contains(&address), "{absent:?}");
    run(vec![Operation::AssignProperty {
        object: Ref::Id(network),
        kind: ObjectKind::Prefix,
        property: Ref::Id(zone),
        values: vec![RawInput::Text("dmz".into())],
    }])
    .await;
    let now = OffsetDateTime::now_utc();
    let explained = explain(&pool, address, zone, now, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(explained.origin, Some(network));
    assert_eq!(explained.value.as_deref(), Some(r#""dmz""#));
    let found = filter(
        &pool,
        zone,
        domain,
        &Condition::Equals("dmz".into()),
        now,
        None,
        100,
    )
    .await
    .unwrap();
    assert!(found.contains(&address), "{found:?}");
}
