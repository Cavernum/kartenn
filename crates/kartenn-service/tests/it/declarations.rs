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
        labels: None,
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
        labels: std::collections::BTreeMap::new(),
    }
}

fn text(max_len: usize) -> TypeInput {
    TypeInput::Text {
        min_len: 0,
        max_len,
        pattern: None,
    }
}

#[kartenn_testing::pg_test]
async fn an_administrator_declares_custom_properties(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // REQ-003-1, 2: `custom.*`, built-in types, closed-list values in the same changeset.
    f.applied(vec![
        declare(1, input("custom.environment", TypeInput::ENUM)),
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
            input("policy.environment", TypeInput::ENUM)
        )])
        .await,
        vec!["prop.declaration.namespace"]
    );
    // A kind no build knows; the VLANs are kinds of the core (spec 026).
    let mut on_rack = input("custom.label", text(10));
    on_rack.carriers = vec!["rack".into()];
    assert_eq!(
        f.refused(vec![declare(1, on_rack)]).await,
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

#[kartenn_testing::pg_test]
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
            cascade: false,
        }])
        .await,
        vec!["prop.declaration.incompatible"]
    );
}

#[kartenn_testing::pg_test]
async fn closing_values_and_properties(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        declare(1, input("custom.environment", TypeInput::ENUM)),
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
        cascade: false,
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
                spec: TypeSpec::ENUM,
                cardinality: Cardinality::Set,
                carriers: carriers.iter().map(|c| (*c).to_owned()).collect(),
                inherit: Some(InheritTree::Addressing),
                obligations: BTreeSet::new(),
                expected: None,
            },
            values.iter().map(|v| (*v).to_owned()).collect(),
        )],
        labels: std::collections::BTreeMap::new(),
    }
}

#[kartenn_testing::pg_test]
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

/// R10, REQ-018-11: the synchronisation writes the labels of the code when they are
/// missing or its own, and never replaces an administrator's.
#[kartenn_testing::pg_test]
async fn an_administrator_label_survives_the_module_sync(pool: PgPool) {
    use kartenn_core::labels::Compiled;
    let f = Fixture::new(&pool).await;
    let labelled = |property: (&'static str, &'static str), prod: (&'static str, &'static str)| {
        ModuleDeclarations {
            labels: [
                (
                    "policy.environment".to_owned(),
                    Compiled::new(property.0, property.1),
                ),
                (
                    "policy.environment/prod".to_owned(),
                    Compiled::new(prod.0, prod.1),
                ),
            ]
            .into(),
            ..policy(&["prod", "dev"], &["prefix"])
        }
    };
    let sync = |module: ModuleDeclarations| {
        let store = f.store.clone();
        async move {
            sync_modules(&store, &Unrestricted, &[module], MAX_OPERATIONS)
                .await
                .unwrap()
        }
    };
    let labels = |sql: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, serde_json::Value>(sql)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let (property, prod) = (
        "SELECT labels FROM property_declaration
          WHERE key = 'policy.environment' AND valid_period @> now()",
        "SELECT labels FROM property_enum_value WHERE key = 'prod' AND valid_period @> now()",
    );
    sync(labelled(
        ("Environnement", "Environment"),
        ("Production", "Production"),
    ))
    .await;
    assert_eq!(
        labels(property).await,
        serde_json::json!({"fr": "Environnement", "en": "Environment"})
    );
    assert_eq!(
        labels(prod).await,
        serde_json::json!({"fr": "Production", "en": "Production"})
    );
    // The module's labels follow its code.
    sync(labelled(
        ("Environnement", "Environment"),
        ("Production", "Live"),
    ))
    .await;
    assert_eq!(labels(prod).await["en"], "Live");
    // An administrator relabels the property, in French only.
    let env = f.property("policy.environment").await;
    f.applied(vec![Operation::UpdateProperty {
        id: Ref::Id(env),
        declaration: DeclarationInput {
            key: "policy.environment".into(),
            value_type: TypeInput::ENUM,
            cardinality: Cardinality::Set,
            carriers: vec!["prefix".into()],
            inherit: Some("addressing".into()),
            module: Some("policy".into()),
            obligations: Vec::new(),
            expected: None,
            labels: Some([("fr".to_owned(), "Milieu".to_owned())].into()),
        },
    }])
    .await;
    let outcomes = sync(labelled(("Environnement", "Env."), ("Production", "Live"))).await;
    assert!(outcomes.is_empty(), "{outcomes:?}");
    assert_eq!(
        labels(property).await,
        serde_json::json!({"fr": "Milieu"}),
        "the administrator's labels stay"
    );
    assert!(
        sync(labelled(
            ("Environnement", "Environment"),
            ("Production", "Live")
        ))
        .await
        .is_empty(),
        "an administrator's labels are no longer the module's"
    );
}

#[kartenn_testing::pg_test]
async fn a_value_is_offered_only_from_its_opening(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        declare(1, input("custom.environment", TypeInput::ENUM)),
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

#[kartenn_testing::pg_test]
async fn entity_and_site_trees_inherit_among_their_objects(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // 003-18: a carrier outside the tree is refused, the tree's own kind accepted.
    for tree in ["entity", "site"] {
        let mut declaration = input("custom.cost_centre", text(20));
        declaration.inherit = Some(tree.into());
        assert_eq!(
            f.refused(vec![declare(1, declaration.clone())]).await,
            vec!["prop.declaration.invalid"],
            "{tree}"
        );
        declaration.key = format!("custom.cost_centre_{tree}");
        declaration.carriers = vec![tree.into()];
        f.applied(vec![declare(1, declaration)]).await;
    }
}

/// An administrator's obligation in the domain `domain` (REQ-008-28).
fn obligation(property: Uuid, domain: Uuid, required: bool) -> Operation {
    let scope = kartenn_service::operations::ObligationInput::Domain { id: domain };
    let property = Ref::Id(property);
    if required {
        Operation::AddObligation { property, scope }
    } else {
        Operation::RemoveObligation { property, scope }
    }
}

/// The domain `slug`, created.
async fn domain(f: &Fixture, slug: &str) -> Uuid {
    f.applied(vec![Operation::CreateDomain {
        local: None,
        slug: slug.into(),
    }])
    .await;
    sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = $1 AND valid_period @> now()")
        .bind(slug)
        .fetch_one(&f.pool)
        .await
        .unwrap()
}

/// The prefix `prefix` in `domain`.
fn prefix_in(domain: Uuid, prefix: &str) -> Operation {
    Operation::CreatePrefix {
        local: None,
        domain: Ref::Id(domain),
        prefix: prefix.into(),
        nature: Nature::Container,
        point_to_point: false,
        anycast: false,
    }
}

/// The current obligations of `property`, by origin.
async fn obligations(pool: &PgPool, property: Uuid) -> Vec<(String, Option<Uuid>)> {
    sqlx::query_as(
        "SELECT origin, scope_id FROM property_obligation
         WHERE property_id = $1 AND valid_period @> now() ORDER BY origin, scope_id",
    )
    .bind(property)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[kartenn_testing::pg_test]
async fn admin_obligation_survives_module_sync(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let sync = |module: ModuleDeclarations| {
        let store = f.store.clone();
        async move {
            sync_modules(&store, &Unrestricted, &[module], MAX_OPERATIONS)
                .await
                .unwrap()
        }
    };
    sync(policy(&["prod", "dev"], &["prefix", "address"])).await;
    let env = f.property("policy.environment").await;
    let lab = domain(&f, "lab").await;
    // An administrator requires the module's property in the domain.
    f.applied(vec![obligation(env, lab, true)]).await;
    assert_eq!(
        obligations(&pool, env).await,
        [("admin".to_owned(), Some(lab))]
    );
    // Enforced like any obligation: a prefix of the domain needs a value.
    assert_eq!(
        f.refused(vec![prefix_in(lab, "10.0.0.0/8")]).await,
        vec!["prop.assignment.required"]
    );
    // The same code: nothing to synchronise, the obligation is not the module's.
    assert!(
        sync(policy(&["prod", "dev"], &["prefix", "address"]))
            .await
            .is_empty()
    );
    // New code rewrites the declaration: the obligation stays.
    let outcomes = sync(policy(
        &["prod", "dev", "sandbox"],
        &["prefix", "address", "range"],
    ))
    .await;
    assert!(
        matches!(outcomes[..], [Outcome::Applied { .. }]),
        "{outcomes:?}"
    );
    assert_eq!(
        obligations(&pool, env).await,
        [("admin".to_owned(), Some(lab))]
    );
    // Removed by an administrator only.
    f.applied(vec![obligation(env, lab, false)]).await;
    assert!(obligations(&pool, env).await.is_empty());
    f.applied(vec![prefix_in(lab, "10.0.0.0/8")]).await;
}

#[kartenn_testing::pg_test]
async fn incompatible_obligation_refused(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    sync_modules(
        &f.store,
        &Unrestricted,
        &[policy(&["prod"], &["prefix", "address"])],
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let env = f.property("policy.environment").await;
    let lab = domain(&f, "lab").await;
    f.applied(vec![prefix_in(lab, "10.0.0.0/8")]).await;
    // A prefix of the domain has no value: the obligation would not hold (REQ-003-16).
    assert_eq!(
        f.refused(vec![obligation(env, lab, true)]).await,
        vec!["prop.assignment.required"]
    );
    assert!(obligations(&pool, env).await.is_empty());
    // An obligation already in force, and one no administrator added.
    let empty = domain(&f, "empty").await;
    f.applied(vec![obligation(env, empty, true)]).await;
    assert_eq!(
        f.refused(vec![obligation(env, empty, true)]).await,
        vec!["prop.declaration.incompatible"]
    );
    assert_eq!(
        f.refused(vec![obligation(env, lab, false)]).await,
        vec!["prop.declaration.incompatible"]
    );
    // A scope that is not a domain over the period.
    assert_eq!(
        f.refused(vec![obligation(env, Uuid::now_v7(), true)]).await,
        vec!["prop.declaration.invalid"]
    );
}

/// A closed list `key` borrowing the values of `owner` (REQ-016-2).
fn borrowing(key: &str, owner: &str) -> DeclarationInput {
    input(
        key,
        TypeInput::Enum {
            values_of: Some(owner.into()),
        },
    )
}

fn assign(object: Uuid, property: Uuid, value: &str) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(object),
        kind: ObjectKind::Prefix,
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.into())],
    }
}

/// A prefix of a new domain `slug`.
async fn a_prefix(f: &Fixture, slug: &str) -> Uuid {
    let domain = domain(f, slug).await;
    f.applied(vec![prefix_in(domain, "10.0.0.0/8")]).await;
    sqlx::query_scalar("SELECT id FROM prefix WHERE domain_id = $1")
        .bind(domain)
        .fetch_one(&f.pool)
        .await
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn a_declaration_borrows_the_values_of_another(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let net = a_prefix(&f, "lab").await;
    // REQ-016-2: the values stay the owner's; a value created for the owner is assignable
    // to the borrower in the same changeset.
    f.applied(vec![
        declare(1, input("custom.environment", TypeInput::ENUM)),
        declare(2, borrowing("custom.declared", "custom.environment")),
        add_value(Ref::Local(1), "prod"),
    ])
    .await;
    let (env, declared) = (
        f.property("custom.environment").await,
        f.property("custom.declared").await,
    );
    f.applied(vec![
        add_value(Ref::Id(env), "dev"),
        assign(net, declared, "dev"),
    ])
    .await;
    assert_eq!(f.current_enum_keys(env).await, vec!["dev", "prod"]);
    assert!(f.current_enum_keys(declared).await.is_empty());
    let value: Uuid = sqlx::query_scalar(
        "SELECT v_uuid FROM property_assignment WHERE object_id = $1 AND property_id = $2",
    )
    .bind(net)
    .bind(declared)
    .fetch_one(&pool)
    .await
    .unwrap();
    let owner: Uuid =
        sqlx::query_scalar("SELECT property_id FROM property_enum_value WHERE id = $1")
            .bind(value)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owner, env);
    // A key the owner lacks, and a value added to the borrower: refused.
    assert_eq!(
        f.refused(vec![assign(net, declared, "test")]).await,
        vec!["prop.assignment.invalid_value"]
    );
    assert_eq!(
        f.refused(vec![add_value(Ref::Id(declared), "test")]).await,
        vec!["prop.declaration.values_of"]
    );
}

#[kartenn_testing::pg_test]
async fn a_borrowed_value_cannot_close_while_cited(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let net = a_prefix(&f, "lab").await;
    f.applied(vec![
        declare(1, input("custom.environment", TypeInput::ENUM)),
        declare(2, borrowing("custom.declared", "custom.environment")),
        add_value(Ref::Local(1), "prod"),
    ])
    .await;
    let declared = f.property("custom.declared").await;
    f.applied(vec![assign(net, declared, "prod")]).await;
    let prod: Uuid = sqlx::query_scalar("SELECT id FROM property_enum_value WHERE key = 'prod'")
        .fetch_one(&pool)
        .await
        .unwrap();
    // Cited by the borrower only: the owner's value stays open (REQ-016-2).
    assert_eq!(
        f.refused(vec![Operation::Close {
            kind: ObjectKind::EnumValue,
            id: Ref::Id(prod),
            expected: None,
            cascade: false,
        }])
        .await,
        vec!["prop.declaration.incompatible"]
    );
}

#[kartenn_testing::pg_test]
async fn a_chain_of_borrowings_is_refused(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        declare(1, input("custom.environment", TypeInput::ENUM)),
        declare(2, borrowing("custom.declared", "custom.environment")),
        declare(3, input("custom.label", text(10))),
        declare(4, input("custom.other", TypeInput::ENUM)),
    ])
    .await;
    // Borrowing from a borrower, from nothing, from another type (REQ-016-2).
    for owner in ["custom.declared", "custom.absent", "custom.label"] {
        assert_eq!(
            f.refused(vec![declare(1, borrowing("custom.chained", owner))])
                .await,
            vec!["prop.declaration.values_of"],
            "{owner}"
        );
    }
    // A lender that would borrow in turn.
    let env = f.property("custom.environment").await;
    assert_eq!(
        f.refused(vec![Operation::UpdateProperty {
            id: Ref::Id(env),
            declaration: borrowing("custom.environment", "custom.other"),
        }])
        .await,
        vec!["prop.declaration.values_of"]
    );
}
