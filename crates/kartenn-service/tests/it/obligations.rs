//! Obligations scoped by domain or object type, and explicit "no value" (spec 003,
//! REQ-003-13, 16; task 003-10).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{DeclarationInput, ObligationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    domain: Uuid,
}

impl Fixture {
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
        f.applied(vec![Operation::CreateDomain {
            local: None,
            slug: "prod".into(),
        }])
        .await;
        f.domain = sqlx::query_scalar("SELECT id FROM addressing_domain")
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

    async fn applied(&self, operations: Vec<Operation>) {
        let outcome = self.submit(operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    /// Refused: the rules and the objects of the first violation.
    async fn refused(&self, operations: Vec<Operation>) -> (Vec<&'static str>, Vec<Uuid>) {
        match self.submit(operations).await {
            Outcome::Refused { violations, .. } => (
                violations.iter().map(|v| v.rule.id).collect(),
                violations[0].subjects.clone(),
            ),
            other => panic!("expected refused, got {other:?}"),
        }
    }

    fn declaration(obligations: Vec<ObligationInput>) -> DeclarationInput {
        DeclarationInput {
            key: "custom.environment".into(),
            value_type: TypeInput::ENUM,
            cardinality: Cardinality::Single,
            carriers: vec!["prefix".into(), "address".into()],
            inherit: Some("addressing".into()),
            module: None,
            obligations,
            expected: None,
            labels: None,
        }
    }

    /// Declares the property with values `prod` and `dev`; returns its identifier.
    async fn declare(&self, obligations: Vec<ObligationInput>) -> Uuid {
        self.applied(vec![
            Operation::DeclareProperty {
                local: Some(1),
                declaration: Self::declaration(obligations),
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
        ])
        .await;
        sqlx::query_scalar("SELECT id FROM property_declaration")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    fn prefix(&self, local: u32, value: &str, nature: Nature) -> Operation {
        Operation::CreatePrefix {
            local: Some(local),
            domain: Ref::Id(self.domain),
            prefix: value.into(),
            nature,
            point_to_point: false,
            anycast: false,
        }
    }

    fn address(&self, local: u32, value: &str) -> Operation {
        Operation::CreateAddress {
            local: Some(local),
            domain: Ref::Id(self.domain),
            address: value.into(),
            usage: AddressUsage::Host,
        }
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
}

fn assign(object: Ref, kind: ObjectKind, property: Uuid, value: &str) -> Operation {
    Operation::AssignProperty {
        object,
        kind,
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.into())],
    }
}

#[kartenn_testing::pg_test]
async fn a_domain_obligation_needs_a_value_everywhere(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let env = f
        .declare(vec![ObligationInput::Domain { id: f.domain }])
        .await;
    // REQ-003-16: a container without a value is refused; with one, accepted.
    let (rules, _) = f
        .refused(vec![f.prefix(1, "10.0.0.0/8", Nature::Container)])
        .await;
    assert_eq!(rules, vec!["prop.assignment.required"]);
    f.applied(vec![
        f.prefix(1, "10.0.0.0/8", Nature::Container),
        assign(Ref::Local(1), ObjectKind::Prefix, env, "prod"),
    ])
    .await;
    let root = f.id("10.0.0.0/8").await;
    // A network below inherits its value: accepted.
    f.applied(vec![f.prefix(1, "10.1.2.0/24", Nature::Network)])
        .await;
    let net = f.id("10.1.2.0/24").await;
    // REQ-003-13 under an obligation: explicit "no value" is refused.
    let (rules, objects) = f
        .refused(vec![Operation::ClearProperty {
            object: Ref::Id(net),
            kind: ObjectKind::Prefix,
            property: Ref::Id(env),
        }])
        .await;
    assert_eq!(rules, vec!["prop.assignment.required"]);
    assert_eq!(objects, vec![net]);
    // Removing the container's value would leave both without one: refused, listing them.
    let (_, mut objects) = f
        .refused(vec![Operation::UnassignProperty {
            object: Ref::Id(root),
            kind: ObjectKind::Prefix,
            property: Ref::Id(env),
        }])
        .await;
    objects.sort();
    let mut expected = vec![root, net];
    expected.sort();
    assert_eq!(objects, expected);
    // Moving the value down first, in the same changeset, is accepted.
    f.applied(vec![
        assign(Ref::Id(net), ObjectKind::Prefix, env, "dev"),
        assign(Ref::Id(root), ObjectKind::Prefix, env, "dev"),
    ])
    .await;
}

#[kartenn_testing::pg_test]
async fn an_object_type_obligation_and_declaring_obligations(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // Objects exist before the obligation.
    f.applied(vec![
        f.prefix(1, "10.0.0.0/8", Nature::Container),
        f.prefix(2, "10.1.2.0/24", Nature::Network),
    ])
    .await;
    let root = f.id("10.0.0.0/8").await;
    // A new property mandatory on the domain: its prefixes have no value yet → refused.
    let (rules, objects) = f
        .refused(vec![Operation::DeclareProperty {
            local: None,
            declaration: Fixture::declaration(vec![ObligationInput::Domain { id: f.domain }]),
        }])
        .await;
    assert_eq!(rules, vec!["prop.assignment.required"]);
    assert_eq!(objects.len(), 2);
    // Mandatory on addresses only: no address yet, accepted.
    let env = f
        .declare(vec![ObligationInput::ObjectType {
            kind: "address".into(),
        }])
        .await;
    assert_eq!(
        f.refused(vec![f.address(1, "10.1.2.3")]).await.0,
        vec!["prop.assignment.required"]
    );
    f.applied(vec![
        f.address(1, "10.1.2.3"),
        assign(Ref::Local(1), ObjectKind::Address, env, "prod"),
    ])
    .await;
    // Adding the domain obligation later: refused while prefixes lack a value (REQ-003-6).
    let mut with_domain = Fixture::declaration(vec![
        ObligationInput::ObjectType {
            kind: "address".into(),
        },
        ObligationInput::Domain { id: f.domain },
    ]);
    let update = |declaration: DeclarationInput| Operation::UpdateProperty {
        id: Ref::Id(env),
        declaration,
    };
    let (rules, _) = f.refused(vec![update(with_domain.clone())]).await;
    assert_eq!(rules, vec!["prop.declaration.incompatible"]);
    f.applied(vec![assign(Ref::Id(root), ObjectKind::Prefix, env, "prod")])
        .await;
    f.applied(vec![update(with_domain.clone())]).await;
    // An obligation scoped by an entity that does not exist (003-18).
    with_domain
        .obligations
        .push(ObligationInput::Entity { id: Uuid::nil() });
    assert_eq!(
        f.refused(vec![update(with_domain)]).await.0,
        vec!["prop.declaration.invalid"]
    );
}
