//! Explained resolution, on both time axes (spec 003, REQ-003-14, 15, 22; task 003-14).

use std::sync::Arc;

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::properties::{Expected, ExpectedMode};
use kartenn_core::values::Cardinality;
use kartenn_service::StoreError;
use kartenn_service::authz::Unrestricted;
use kartenn_service::expected::{
    Expected as Answer, ExpectedContext, ExpectedValueSource, Sources, Suggestion,
};
use kartenn_service::module::{BoxFuture, ModuleTx};
use kartenn_service::operations::{DeclarationInput, PeriodSpec, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::ObjectView;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit_with};
use kartenn_store_pg::explain::{Explanation, explain};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

/// 2030-01-01T00:00Z in microseconds; the tests live on days from there.
const T0: i64 = 1_893_456_000_000_000;
const DAY: i64 = 86_400_000_000;

/// `prod` as JSON.
const PROD: &str = r#""prod""#;

fn day(d: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(T0 + d * DAY) * 1_000).unwrap()
}

/// Expects `prod` for every address.
struct AddressesAreProd;

impl ExpectedValueSource for AddressesAreProd {
    fn id(&self) -> &'static str {
        "addresses"
    }

    fn revision<'a>(
        &'a self,
        _ctx: ExpectedContext,
        _tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<String, StoreError>> {
        Box::pin(async { Ok("3".to_owned()) })
    }

    fn expected<'a>(
        &'a self,
        _property: &'a str,
        object: &'a ObjectView,
        _ctx: ExpectedContext,
        _tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Answer, StoreError>> {
        Box::pin(async move {
            Ok(if object.kind == "address" {
                Answer::One(Suggestion {
                    values: vec![RawInput::Text("prod".into())],
                    explanation: "addresses are prod".into(),
                    detail: None,
                })
            } else {
                Answer::None
            })
        })
    }
}

struct Fixture {
    store: PgStore,
    sources: Sources,
    author: Uuid,
    env: Uuid,
    root: Uuid,
    net: Uuid,
    addr: Uuid,
}

impl Fixture {
    /// Day 0: `custom.environment` heritable (`prod`, `dev`), container `10.0.0.0/8` in
    /// `prod`, network `10.1.2.0/24` and address `10.1.2.3` without assignment.
    async fn new(pool: &PgPool, expected: Option<Expected>) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let mut f = Self {
            store: PgStore::new(pool.clone()),
            sources: Sources::none().with(Arc::new(AddressesAreProd)),
            author,
            env: Uuid::nil(),
            root: Uuid::nil(),
            net: Uuid::nil(),
            addr: Uuid::nil(),
        };
        let declaration = DeclarationInput {
            key: "custom.environment".into(),
            value_type: TypeInput::ENUM,
            cardinality: Cardinality::Single,
            carriers: vec!["prefix".into(), "address".into()],
            inherit: Some("addressing".into()),
            module: None,
            obligations: Vec::new(),
            expected,
            labels: None,
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
                    slug: "prod".into(),
                },
                Operation::CreatePrefix {
                    local: Some(3),
                    domain: Ref::Local(2),
                    prefix: "10.0.0.0/8".into(),
                    nature: Nature::Container,
                    point_to_point: false,
                    anycast: false,
                },
                Operation::CreatePrefix {
                    local: None,
                    domain: Ref::Local(2),
                    prefix: "10.1.2.0/24".into(),
                    nature: Nature::Network,
                    point_to_point: false,
                    anycast: false,
                },
                Operation::CreateAddress {
                    local: None,
                    domain: Ref::Local(2),
                    address: "10.1.2.3".into(),
                    usage: AddressUsage::Host,
                },
                Operation::AssignProperty {
                    object: Ref::Local(3),
                    kind: ObjectKind::Prefix,
                    property: Ref::Local(1),
                    values: vec![RawInput::Text("prod".into())],
                },
            ],
            0,
        )
        .await;
        f.env = sqlx::query_scalar("SELECT id FROM property_declaration")
            .fetch_one(pool)
            .await
            .unwrap();
        let id = |v: &'static str| {
            sqlx::query_scalar("SELECT id FROM addressing_object WHERE value = $1::inet")
                .bind(v)
                .fetch_one(pool)
        };
        f.root = id("10.0.0.0/8").await.unwrap();
        f.net = id("10.1.2.0/24").await.unwrap();
        f.addr = id("10.1.2.3").await.unwrap();
        f
    }

    async fn applied(&self, operations: Vec<Operation>, start: i64) {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: Some(PeriodSpec {
                start: T0 + start * DAY,
                end: None,
            }),
            operations,
            simulate: false,
            idempotency_key: None,
        };
        let outcome = submit_with(
            &self.store,
            &Unrestricted,
            &self.sources,
            &request,
            MAX_OPERATIONS,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    fn net_operation(&self, clear: bool) -> Operation {
        if clear {
            Operation::ClearProperty {
                object: Ref::Id(self.net),
                kind: ObjectKind::Prefix,
                property: Ref::Id(self.env),
            }
        } else {
            Operation::AssignProperty {
                object: Ref::Id(self.net),
                kind: ObjectKind::Prefix,
                property: Ref::Id(self.env),
                values: vec![RawInput::Text("dev".into())],
            }
        }
    }
}

#[kartenn_testing::pg_test]
async fn the_resolution_is_explained_on_both_axes(pool: PgPool) {
    let f = Fixture::new(&pool, None).await;
    let at = |object, d, as_of| explain(&pool, object, f.env, day(d), as_of);
    // REQ-003-14: inherited from the container, through the network.
    assert_eq!(
        at(f.addr, 1, None).await.unwrap(),
        Some(Explanation {
            property: "custom.environment".into(),
            value: Some(PROD.into()),
            origin: Some(f.root),
            chain: vec![f.addr, f.net, f.root],
            provenance: Some("input".into()),
            replaces: None,
        })
    );
    // From day 2 the network carries `dev`: it replaces the inherited `prod`.
    let before = OffsetDateTime::now_utc();
    f.applied(vec![f.net_operation(false)], 2).await;
    let now = at(f.addr, 3, None).await.unwrap().unwrap();
    assert_eq!(now.value, Some(r#""dev""#.into()));
    assert_eq!(now.chain, vec![f.addr, f.net]);
    assert_eq!(now.replaces, Some((f.root, Some(PROD.into()))));
    // REQ-003-15: before day 2 in application time, or as asserted before the change.
    assert_eq!(
        at(f.addr, 1, None).await.unwrap().unwrap().origin,
        Some(f.root)
    );
    assert_eq!(
        at(f.addr, 3, Some(before)).await.unwrap().unwrap().origin,
        Some(f.root)
    );
    // REQ-003-13: an explicit "no value" is an origin with no value, replacing `prod`.
    f.applied(vec![f.net_operation(true)], 4).await;
    let cleared = at(f.addr, 5, None).await.unwrap().unwrap();
    assert_eq!(
        (cleared.value, cleared.origin, cleared.replaces),
        (None, Some(f.net), Some((f.root, Some(PROD.into()))))
    );
    // Before day 0 the property is not declared: nothing to explain.
    assert_eq!(at(f.root, -1, None).await.unwrap(), None);
    // Without the container's value, nothing applies: the object alone, no origin.
    f.applied(
        vec![Operation::UnassignProperty {
            object: Ref::Id(f.root),
            kind: ObjectKind::Prefix,
            property: Ref::Id(f.env),
        }],
        6,
    )
    .await;
    let root = at(f.root, 7, None).await.unwrap().unwrap();
    assert_eq!(
        (root.origin, root.chain, root.value),
        (None, vec![f.root], None)
    );
}

#[kartenn_testing::pg_test]
async fn a_default_value_shows_its_source(pool: PgPool) {
    let f = Fixture::new(
        &pool,
        Some(Expected {
            source: "addresses".into(),
            mode: ExpectedMode::Default,
        }),
    )
    .await;
    // REQ-003-22: the address created with the property received the source's default.
    let e = explain(&pool, f.addr, f.env, day(1), None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(e.origin, Some(f.addr));
    assert_eq!(e.provenance.as_deref(), Some("source:addresses@3"));
    assert_eq!(e.replaces, Some((f.root, Some(PROD.into()))));
}
