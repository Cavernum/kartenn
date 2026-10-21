//! Rights on properties: direct permission and permission on the effect (spec 003,
//! REQ-003-17, 18; task 003-11), one test per edge case of the design.

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::{Grants, Unrestricted};
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::{MAX_OPERATIONS, PendingKind};
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

const WRITE: &str = "prop.custom.environment.write";
const PROPOSE: &str = "prop.custom.environment.propose";

struct Fixture {
    pool: PgPool,
    store: PgStore,
    admin: Uuid,
    contractor: Uuid,
    domain: Uuid,
    env: Uuid,
}

impl Fixture {
    /// `10.0.0.0/8` in `{prod}`, `10.5.0.0/16` in `{dev}` with the network `10.5.0.0/24`
    /// and the address `10.5.0.1` below, written by an administrator.
    async fn new(pool: &PgPool) -> Self {
        let person = |name: &'static str| {
            sqlx::query_scalar(
                "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), $1) RETURNING id",
            )
            .bind(name)
            .fetch_one(pool)
        };
        let mut f = Self {
            pool: pool.clone(),
            store: PgStore::new(pool.clone()),
            admin: person("admin").await.unwrap(),
            contractor: person("contractor").await.unwrap(),
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
        let assign = |local, value: &str| Operation::AssignProperty {
            object: Ref::Local(local),
            kind: ObjectKind::Prefix,
            property: Ref::Local(2),
            values: vec![RawInput::Text(value.into())],
        };
        let outcome = f
            .submit(
                &Unrestricted,
                f.admin,
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
                    prefix(3, Ref::Local(1), "10.0.0.0/8", Nature::Container),
                    prefix(4, Ref::Local(1), "10.5.0.0/16", Nature::Container),
                    prefix(5, Ref::Local(1), "10.5.0.0/24", Nature::Network),
                    Operation::CreateAddress {
                        local: None,
                        domain: Ref::Local(1),
                        address: "10.5.0.1".into(),
                        usage: AddressUsage::Host,
                    },
                    assign(3, "prod"),
                    assign(4, "dev"),
                ],
            )
            .await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
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

    async fn submit<A: kartenn_service::authz::Authorizer>(
        &self,
        authorizer: &A,
        author: Uuid,
        operations: Vec<Operation>,
    ) -> Outcome {
        let request = ChangesetRequest {
            author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit(&self.store, authorizer, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    /// Refused by the rights: the objects refused.
    /// `grants` plus the right to write the objects themselves (005): these tests are about
    /// the rights on the properties and their effects.
    fn with_objects(&self, grants: &Grants) -> Grants {
        ["prefix", "range", "address"]
            .iter()
            .fold(grants.clone(), |g, kind| {
                g.everywhere(self.contractor, &format!("core.{kind}.write"))
            })
    }

    async fn forbidden(&self, grants: &Grants, operations: Vec<Operation>) -> Vec<Uuid> {
        let grants = self.with_objects(grants);
        match self.submit(&grants, self.contractor, operations).await {
            Outcome::Refused { violations, .. } => {
                assert_eq!(violations.len(), 1, "{violations:?}");
                assert_eq!(violations[0].rule.id, "prop.assignment.forbidden_effect");
                let mut subjects = violations[0].subjects.clone();
                subjects.sort();
                subjects
            }
            other => panic!("expected refused, got {other:?}"),
        }
    }

    async fn applied(&self, grants: &Grants, operations: Vec<Operation>) {
        let outcome = self
            .submit(&self.with_objects(grants), self.contractor, operations)
            .await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    async fn id(&self, value: &str) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM addressing_object WHERE value = $1::inet AND kind <> 'domain' LIMIT 1",
        )
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    fn assign(&self, object: Ref, kind: ObjectKind, value: &str) -> Operation {
        Operation::AssignProperty {
            object,
            kind,
            property: Ref::Id(self.env),
            values: vec![RawInput::Text(value.into())],
        }
    }
}

fn prefix(local: u32, domain: Ref, value: &str, nature: Nature) -> Operation {
    Operation::CreatePrefix {
        local: Some(local),
        domain,
        prefix: value.into(),
        nature,
        point_to_point: false,
        anycast: false,
    }
}

fn sorted(mut ids: Vec<Uuid>) -> Vec<Uuid> {
    ids.sort();
    ids
}

#[kartenn_testing::pg_test]
async fn an_assignment_needs_the_property_right_on_the_object_and_its_effects(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let net = f.id("10.5.0.0/24").await;
    let addr = f.id("10.5.0.1").await;
    // REQ-003-17: allowed to touch the network, but not the property → refused.
    let none = Grants::new();
    assert_eq!(
        f.forbidden(
            &none,
            vec![f.assign(Ref::Id(net), ObjectKind::Prefix, "prod")]
        )
        .await,
        sorted(vec![net, addr])
    );
    // The right on the network alone does not cover the address whose value changes.
    let network_only = Grants::new().on(f.contractor, WRITE, [net]);
    assert_eq!(
        f.forbidden(
            &network_only,
            vec![f.assign(Ref::Id(net), ObjectKind::Prefix, "prod")]
        )
        .await,
        vec![addr]
    );
    let both = Grants::new().on(f.contractor, WRITE, [net, addr]);
    f.applied(
        &both,
        vec![f.assign(Ref::Id(net), ObjectKind::Prefix, "prod")],
    )
    .await;
}

#[kartenn_testing::pg_test]
async fn structure_changes_need_the_right_only_when_values_change(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let none = Grants::new();
    // Edge case: a container without assignment between `{prod}` and `{dev}` changes no
    // resolved value → no right needed (REQ-003-18 acceptance).
    f.applied(
        &none,
        vec![prefix(
            1,
            Ref::Id(f.domain),
            "10.5.0.0/20",
            Nature::Container,
        )],
    )
    .await;
    // A new address inheriting `{dev}` is a new object → accepted.
    f.applied(
        &none,
        vec![Operation::CreateAddress {
            local: None,
            domain: Ref::Id(f.domain),
            address: "10.5.0.2".into(),
            usage: AddressUsage::Host,
        }],
    )
    .await;
    // Edge case: closing the `{dev}` container under `{prod}` moves everything below to
    // `{prod}` → refused, listing them.
    let dev = f.id("10.5.0.0/16").await;
    let close = || {
        vec![Operation::Close {
            kind: ObjectKind::Prefix,
            id: Ref::Id(dev),
            expected: None,
            cascade: false,
        }]
    };
    let below = sorted(vec![
        f.id("10.5.0.0/20").await,
        f.id("10.5.0.0/24").await,
        f.id("10.5.0.1").await,
        f.id("10.5.0.2").await,
    ]);
    assert_eq!(f.forbidden(&none, close()).await, below);
    // With the right to propose only, a proposal (REQ-004-41).
    let propose = Grants::new().everywhere(f.contractor, PROPOSE);
    match f
        .submit(&f.with_objects(&propose), f.contractor, close())
        .await
    {
        Outcome::Pending { kind, .. } => assert_eq!(kind, PendingKind::Proposal),
        other => panic!("expected a proposal, got {other:?}"),
    }
    f.applied(&Grants::new().everywhere(f.contractor, WRITE), close())
        .await;
}

#[kartenn_testing::pg_test]
async fn new_objects_need_the_direct_right_only(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // Edge case: a changeset creating a network and its addresses and assigning the
    // network: no effect check on new objects, but the assignment needs the right.
    let operations = || {
        vec![
            prefix(1, Ref::Id(f.domain), "10.6.0.0/24", Nature::Network),
            Operation::CreateAddress {
                local: None,
                domain: Ref::Id(f.domain),
                address: "10.6.0.1".into(),
                usage: AddressUsage::Host,
            },
            f.assign(Ref::Local(1), ObjectKind::Prefix, "dev"),
        ]
    };
    let refused = f.forbidden(&Grants::new(), operations()).await;
    assert_eq!(refused.len(), 1, "only the assigned network");
    f.applied(&Grants::new().everywhere(f.contractor, WRITE), operations())
        .await;
}
