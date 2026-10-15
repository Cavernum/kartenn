//! Property declarations through changesets and module synchronisation (spec 003,
//! REQ-003-1 to 6; task 003-7).

use std::collections::BTreeSet;

use kartenn_core::objects::Nature;
use kartenn_core::properties::{Declarant, Declaration, InheritTree, PropertyKey, TypeSpec};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{ModuleDeclarations, SYSTEM_AUTHOR, sync_modules};
use kartenn_service::operations::{DeclarationInput, PeriodSpec, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
}

impl Fixture {
    async fn new(pool: &PgPool) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'admin') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        Self {
            pool: pool.clone(),
            store: PgStore::new(pool.clone()),
            author,
        }
    }

    async fn submit(&self, operations: Vec<Operation>) -> Outcome {
        self.submit_at(operations, None).await
    }

    /// Submits from `start` (microseconds since the epoch) on, or from now.
    async fn submit_at(&self, operations: Vec<Operation>, start: Option<i64>) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: start.map(|start| PeriodSpec { start, end: None }),
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>) {
        let outcome = self.submit(operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    async fn refused(&self, operations: Vec<Operation>) -> Vec<&'static str> {
        match self.submit(operations).await {
            Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
            other => panic!("expected refused, got {other:?}"),
        }
    }

    async fn property(&self, key: &str) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM property_declaration WHERE key = $1 AND valid_period @> now()",
        )
        .bind(key)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn current_enum_keys(&self, property: Uuid) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT key FROM property_enum_value WHERE property_id = $1 AND valid_period @> now() ORDER BY key",
        )
        .bind(property)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    /// Writes an assignment directly: assignment operations come with 003-8.
    async fn assign_text(&self, property: Uuid, value: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO property_assignment (object_id, object_type, property_id, kind, v_text, provenance, valid_period, changeset_id)
             VALUES (gen_random_uuid(), 'prefix', $1, 'value', $2, 'input', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())
             RETURNING object_id",
        )
        .bind(property)
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn input(key: &str, value_type: TypeInput) -> DeclarationInput {
    DeclarationInput {
        key: key.into(),
        value_type,
        cardinality: Cardinality::Single,
        carriers: vec!["prefix".into(), "address".into()],
        inherit: Some("addressing".into()),
        module: None,
        obligations: Vec::new(),
        expected: None,
    }
}

fn declare(local: u32, input: DeclarationInput) -> Operation {
    Operation::DeclareProperty {
        local: Some(local),
        declaration: input,
    }
}

fn add_value(property: Ref, key: &str) -> Operation {
    Operation::AddEnumValue {
        local: None,
        property,
        key: key.into(),
    }
}

fn text(max_len: usize) -> TypeInput {
    TypeInput::Text {
        min_len: 0,
        max_len,
        pattern: None,
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_administrator_declares_custom_properties(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // REQ-003-1, 2: `custom.*`, built-in types, closed-list values in the same changeset.
    f.applied(vec![
        declare(1, input("custom.environment", TypeInput::Enum)),
        add_value(Ref::Local(1), "prod"),
        add_value(Ref::Local(1), "dev"),
    ])
    .await;
    let env = f.property("custom.environment").await;
    assert_eq!(f.current_enum_keys(env).await, vec!["dev", "prod"]);
    assert_eq!(
        f.refused(vec![declare(1, input("custom.environment", text(10)))])
            .await,
        vec!["prop.declaration.duplicate"]
    );
    assert_eq!(
        f.refused(vec![declare(
            1,
            input("policy.environment", TypeInput::Enum)
        )])
        .await,
        vec!["prop.declaration.namespace"]
    );
    let mut on_vlan = input("custom.label", text(10));
    on_vlan.carriers = vec!["vlan".into()];
    assert_eq!(
        f.refused(vec![declare(1, on_vlan)]).await,
        vec!["prop.declaration.invalid"]
    );
    assert_eq!(
        f.refused(vec![add_value(Ref::Id(env), "prod")]).await,
        vec!["prop.enum_value.invalid"],
        "one key per property"
    );
    assert_eq!(
        f.refused(vec![add_value(Ref::Id(env), "Prod")]).await,
        vec!["prop.enum_value.invalid"]
    );
    f.applied(vec![declare(1, input("custom.cost_centre", text(10)))])
        .await;
    let cost = f.property("custom.cost_centre").await;
    assert_eq!(
        f.refused(vec![add_value(Ref::Id(cost), "a")]).await,
        vec!["prop.enum_value.invalid"],
        "only a closed list has values"
    );
}

#[test]
fn the_api_cannot_declare_on_behalf_of_a_module() {
    // P1, P12: neither the declarant nor a module type comes from the API.
    let json = r#"{"op": "declare_property", "local": 1, "key": "custom.x", "type": "enum",
                   "cardinality": "single", "carriers": ["prefix"], "module": "policy"}"#;
    let op: Operation = serde_json::from_str(json).unwrap();
    let Operation::DeclareProperty { declaration, .. } = op else {
        panic!("{op:?}")
    };
    assert_eq!(declaration.module, None);
    let module_type = r#"{"op": "declare_property", "local": 1, "key": "custom.x",
                          "type": "module", "id": "policy.flow", "cardinality": "single",
                          "carriers": ["prefix"]}"#;
    assert!(serde_json::from_str::<Operation>(module_type).is_err());
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn declarations_evolve_only_compatibly(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![declare(1, input("custom.cost_centre", text(20)))])
        .await;
    let cost = f.property("custom.cost_centre").await;
    let update = |value_type| Operation::UpdateProperty {
        id: Ref::Id(cost),
        declaration: input("custom.cost_centre", value_type),
    };
    // Unused: any compatible change goes, and the former version stays in history.
    f.applied(vec![update(text(15))]).await;
    let versions: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM property_declaration WHERE id = $1)
              + (SELECT count(*) FROM property_declaration_history WHERE id = $1)",
    )
    .bind(cost)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(versions >= 2, "{versions}");
    // REQ-003-6: a stricter bound than an existing value is refused, listing the object.
    let object = f.assign_text(cost, "CC-0042-NORD").await;
    let outcome = f.submit(vec![update(text(5))]).await;
    let Outcome::Refused { violations, .. } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(violations[0].rule.id, "prop.declaration.incompatible");
    assert!(violations[0].subjects.contains(&object));
    // Type change while assigned: refused; the key never changes.
    assert_eq!(
        f.refused(vec![update(TypeInput::Int { min: 0, max: 9 })])
            .await,
        vec!["prop.declaration.incompatible"]
    );
    assert_eq!(
        f.refused(vec![Operation::UpdateProperty {
            id: Ref::Id(cost),
            declaration: input("custom.other", text(15)),
        }])
        .await,
        vec!["prop.declaration.incompatible"]
    );
    // Closing the property while assigned: refused (P8).
    assert_eq!(
        f.refused(vec![Operation::Close {
            kind: ObjectKind::Property,
            id: Ref::Id(cost),
            expected: None,
        }])
        .await,
        vec!["prop.declaration.incompatible"]
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn closing_values_and_properties(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        declare(1, input("custom.environment", TypeInput::Enum)),
        add_value(Ref::Local(1), "prod"),
        add_value(Ref::Local(1), "sandbox"),
    ])
    .await;
    let env = f.property("custom.environment").await;
    let value = |key: &'static str| {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM property_enum_value WHERE property_id = $1 AND key = $2",
        )
        .bind(env)
        .bind(key)
        .fetch_one(&f.pool)
    };
    let sandbox = value("sandbox").await.unwrap();
    sqlx::query(
        "INSERT INTO property_assignment (object_id, object_type, property_id, kind, v_uuid, provenance, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'prefix', $1, 'value', $2, 'input', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(env)
    .bind(sandbox)
    .execute(&pool)
    .await
    .unwrap();
    let close = |kind, id| Operation::Close {
        kind,
        id: Ref::Id(id),
        expected: None,
    };
    // REQ-003-6: an assigned value cannot be closed; an unassigned one can.
    assert_eq!(
        f.refused(vec![close(ObjectKind::EnumValue, sandbox)]).await,
        vec!["prop.declaration.incompatible"]
    );
    sqlx::query("DELETE FROM property_assignment")
        .execute(&pool)
        .await
        .unwrap();
    f.applied(vec![close(ObjectKind::EnumValue, sandbox)]).await;
    assert_eq!(f.current_enum_keys(env).await, vec!["prod"]);
    // Closing the property closes its values.
    f.applied(vec![close(ObjectKind::Property, env)]).await;
    assert!(f.current_enum_keys(env).await.is_empty());
}

fn policy(values: &[&str], carriers: &[&str]) -> ModuleDeclarations {
    ModuleDeclarations {
        module: "policy".into(),
        version: "0.2.0".into(),
        properties: vec![(
            Declaration {
                key: PropertyKey::parse("policy.environment").unwrap(),
                declarant: Declarant::Module("policy".into()),
                spec: TypeSpec::Enum,
                cardinality: Cardinality::Set,
                carriers: carriers.iter().map(|c| (*c).to_owned()).collect(),
                inherit: Some(InheritTree::Addressing),
                obligations: BTreeSet::new(),
                expected: None,
            },
            values.iter().map(|v| (*v).to_owned()).collect(),
        )],
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn module_declarations_follow_the_code(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let sync = |module: ModuleDeclarations| {
        let store = f.store.clone();
        async move {
            sync_modules(&store, &Unrestricted, &[module], MAX_OPERATIONS)
                .await
                .unwrap()
        }
    };
    // P12: first deployment declares, in a changeset of the system author.
    let outcomes = sync(policy(&["prod", "dev"], &["prefix", "address"])).await;
    assert!(
        matches!(outcomes[..], [Outcome::Applied { .. }]),
        "{outcomes:?}"
    );
    let env = f.property("policy.environment").await;
    assert_eq!(f.current_enum_keys(env).await, vec!["dev", "prod"]);
    let (author, motive): (Uuid, String) = sqlx::query_as(
        "SELECT author_id, motive FROM changeset ORDER BY submitted_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(author, SYSTEM_AUTHOR);
    assert_eq!(motive, "module sync policy@0.2.0");
    // Same code: nothing to do.
    assert!(
        sync(policy(&["prod", "dev"], &["prefix", "address"]))
            .await
            .is_empty()
    );
    // An administrator's value survives; new code adds a value and a carrier.
    f.applied(vec![add_value(Ref::Id(env), "rec")]).await;
    let outcomes = sync(policy(
        &["prod", "dev", "sandbox"],
        &["prefix", "address", "range"],
    ))
    .await;
    assert!(
        matches!(outcomes[..], [Outcome::Applied { .. }]),
        "{outcomes:?}"
    );
    assert_eq!(f.property("policy.environment").await, env, "same property");
    assert_eq!(
        f.current_enum_keys(env).await,
        vec!["dev", "prod", "rec", "sandbox"]
    );
    let carriers: Vec<String> = sqlx::query_scalar(
        "SELECT carriers FROM property_declaration WHERE id = $1 AND valid_period @> now()",
    )
    .bind(env)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(carriers, vec!["address", "prefix", "range"]);
    // Gone from the code: closed.
    let empty = ModuleDeclarations {
        properties: vec![],
        ..policy(&[], &[])
    };
    sync(empty).await;
    let current: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM property_declaration WHERE id = $1 AND valid_period @> now()",
    )
    .bind(env)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(current, 0);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_value_is_offered_only_from_its_opening(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        declare(1, input("custom.environment", TypeInput::Enum)),
        add_value(Ref::Local(1), "prod"),
        Operation::CreateDomain {
            local: Some(2),
            slug: "lab".into(),
        },
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Local(2),
            prefix: "10.0.0.0/8".into(),
            nature: Nature::Container,
            point_to_point: false,
            anycast: false,
        },
    ])
    .await;
    let env = f.property("custom.environment").await;
    let prefix: Uuid = sqlx::query_scalar("SELECT id FROM prefix")
        .fetch_one(&pool)
        .await
        .unwrap();
    let now =
        i64::try_from(sqlx::types::time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000)
            .unwrap();
    let day = 86_400_000_000_i64;
    // REQ-003-5: `preprod` opens on day 10.
    let outcome = f
        .submit_at(
            vec![add_value(Ref::Id(env), "preprod")],
            Some(now + 10 * day),
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let assign = || {
        vec![Operation::AssignProperty {
            object: Ref::Id(prefix),
            kind: ObjectKind::Prefix,
            property: Ref::Id(env),
            values: vec![RawInput::Text("preprod".into())],
        }]
    };
    match f.submit_at(assign(), Some(now + day)).await {
        Outcome::Refused { violations, .. } => assert!(
            violations
                .iter()
                .any(|v| v.rule.id == "prop.assignment.invalid_value"),
            "{violations:?}"
        ),
        other => panic!("a value before its opening must be refused, got {other:?}"),
    }
    let outcome = f.submit_at(assign(), Some(now + 10 * day)).await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}
