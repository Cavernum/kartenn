//! Reading one object with its resolved properties, now and as known in the past (spec
//! 002, REQ-002-45; spec 004, REQ-004-16; task 002-27).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::read::object;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

fn declare(local: u32, key: &str, value_type: TypeInput, heritable: bool) -> Operation {
    Operation::DeclareProperty {
        local: Some(local),
        declaration: DeclarationInput {
            key: key.into(),
            value_type,
            cardinality: Cardinality::Single,
            carriers: vec!["prefix".into(), "address".into()],
            inherit: heritable.then(|| "addressing".into()),
            module: None,
            obligations: Vec::new(),
            expected: None,
            labels: None,
        },
    }
}

fn assign(object: Uuid, kind: ObjectKind, property: Ref, value: &str) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(object),
        kind,
        property,
        values: vec![RawInput::Text(value.into())],
    }
}

#[kartenn_testing::pg_test]
async fn an_object_is_read_with_its_resolved_properties(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let run = |operations| {
        let request = ChangesetRequest {
            author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            apply_own: true,
        };
        let store = store.clone();
        async move {
            match submit(&store, &Unrestricted, &request, MAX_OPERATIONS)
                .await
                .unwrap()
            {
                Outcome::Applied { .. } => {}
                other => panic!("{other:?}"),
            }
        }
    };
    run(vec![
        declare(1, "custom.environment", TypeInput::ENUM, true),
        Operation::AddEnumValue {
            local: None,
            property: Ref::Local(1),
            key: "prod".into(),
            labels: std::collections::BTreeMap::new(),
        },
        declare(
            2,
            "custom.note",
            TypeInput::Text {
                min_len: 0,
                max_len: 40,
                pattern: None,
            },
            false,
        ),
        Operation::CreateDomain {
            local: Some(3),
            slug: "lab".into(),
        },
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Local(3),
            prefix: "10.1.2.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
        Operation::CreateAddress {
            local: None,
            domain: Ref::Local(3),
            address: "10.1.2.3".into(),
            usage: AddressUsage::Host,
        },
    ])
    .await;
    let id = |sql: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(sql)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let network = id("SELECT id FROM prefix").await;
    let address = id("SELECT id FROM address").await;
    let environment =
        Ref::Id(id("SELECT id FROM property_declaration WHERE key = 'custom.environment'").await);
    let note = Ref::Id(id("SELECT id FROM property_declaration WHERE key = 'custom.note'").await);
    let before = OffsetDateTime::now_utc();
    run(vec![
        assign(network, ObjectKind::Prefix, environment, "prod"),
        assign(address, ObjectKind::Address, note, "printer"),
    ])
    .await;

    let now = OffsetDateTime::now_utc();
    let read = object(&pool, address, now, None).await.unwrap().unwrap();
    assert_eq!(read.kind, "address");
    assert_eq!(read.fields["address"], "10.1.2.3");
    assert_eq!(read.fields["usage"], "host");
    assert_eq!(
        read.properties["custom.environment"],
        Some(serde_json::json!("prod")),
        "inherited from the network"
    );
    assert_eq!(
        read.properties["custom.note"],
        Some(serde_json::json!("printer"))
    );
    // REQ-004-16: as known before the assignments, nothing resolved.
    let past = object(&pool, address, now, Some(before))
        .await
        .unwrap()
        .unwrap();
    assert!(past.properties.is_empty(), "{:?}", past.properties);
    assert!(
        object(&pool, Uuid::now_v7(), now, None)
            .await
            .unwrap()
            .is_none()
    );
}
