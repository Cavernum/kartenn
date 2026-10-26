//! References between objects and their closure (spec 003, REQ-003-10; task 003-12).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

struct Fixture {
    store: PgStore,
    author: Uuid,
    gateway: Uuid,
    net: Uuid,
    addr: Uuid,
}

impl Fixture {
    /// Network `10.1.2.0/24` with the address `10.1.2.1`, and `custom.gateway`, a
    /// reference to an address carried by prefixes (not heritable).
    async fn new(pool: &PgPool) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let mut f = Self {
            store: PgStore::new(pool.clone()),
            author,
            gateway: Uuid::nil(),
            net: Uuid::nil(),
            addr: Uuid::nil(),
        };
        let outcome = f
            .submit(vec![
                Operation::CreateDomain {
                    local: Some(1),
                    slug: "prod".into(),
                },
                Operation::CreatePrefix {
                    local: None,
                    domain: Ref::Local(1),
                    prefix: "10.1.2.0/24".into(),
                    nature: Nature::Network,
                    point_to_point: false,
                    anycast: false,
                },
                Operation::CreateAddress {
                    local: None,
                    domain: Ref::Local(1),
                    address: "10.1.2.1".into(),
                    usage: AddressUsage::Gateway,
                },
                Operation::DeclareProperty {
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
                },
            ])
            .await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        f.gateway = sqlx::query_scalar("SELECT id FROM property_declaration")
            .fetch_one(pool)
            .await
            .unwrap();
        f.net = sqlx::query_scalar("SELECT id FROM prefix")
            .fetch_one(pool)
            .await
            .unwrap();
        f.addr = sqlx::query_scalar("SELECT id FROM address")
            .fetch_one(pool)
            .await
            .unwrap();
        f
    }

    async fn submit(&self, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    fn assign(&self) -> Operation {
        Operation::AssignProperty {
            object: Ref::Id(self.net),
            kind: ObjectKind::Prefix,
            property: Ref::Id(self.gateway),
            values: vec![RawInput::Text(self.addr.to_string())],
        }
    }

    fn unassign(&self) -> Operation {
        Operation::UnassignProperty {
            object: Ref::Id(self.net),
            kind: ObjectKind::Prefix,
            property: Ref::Id(self.gateway),
        }
    }

    fn close_address(&self) -> Operation {
        Operation::Close {
            kind: ObjectKind::Address,
            id: Ref::Id(self.addr),
            expected: None,
            cascade: false,
        }
    }

    async fn refused(&self, operations: Vec<Operation>) -> (Vec<&'static str>, Vec<Uuid>) {
        match self.submit(operations).await {
            Outcome::Refused { violations, .. } => (
                violations.iter().map(|v| v.rule.id).collect(),
                violations[0].subjects.clone(),
            ),
            other => panic!("expected refused, got {other:?}"),
        }
    }
}

#[kartenn_testing::pg_test]
async fn a_referenced_object_cannot_be_closed(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let outcome = f.submit(vec![f.assign()]).await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    // REQ-003-10: closing the gateway would leave the network's reference dangling.
    let (rules, subjects) = f.refused(vec![f.close_address()]).await;
    assert_eq!(rules, vec!["prop.assignment.dangling_reference"]);
    assert_eq!(subjects, vec![f.addr, f.net]);
    // On the final state: removing the reference in the same changeset makes it valid.
    let outcome = f.submit(vec![f.unassign(), f.close_address()]).await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}

#[kartenn_testing::pg_test]
async fn a_reference_needs_its_target_for_its_whole_period(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // Edge case: a reference to an object closed later in the same changeset → refused.
    let (rules, _) = f.refused(vec![f.assign(), f.close_address()]).await;
    assert_eq!(rules, vec!["prop.assignment.dangling_reference"]);
    // Closed first: the reference itself is invalid (its target does not cover the period).
    let (rules, _) = f.refused(vec![f.close_address(), f.assign()]).await;
    assert_eq!(rules, vec!["prop.assignment.invalid_value"]);
}
