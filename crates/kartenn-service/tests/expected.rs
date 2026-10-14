//! Sources of expected values: default values, deviations and suggestions (spec 003,
//! REQ-003-21 to 24; task 003-13).

use std::collections::BTreeMap;
use std::sync::Arc;

use kartenn_core::objects::Nature;
use kartenn_core::properties::{Expected, ExpectedMode};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::expected::{ExpectedValueSource, Sources, Suggestion, suggest, suggest_for};
use kartenn_service::operations::{DeclarationInput, ObligationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::ObjectView;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit_with};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

/// Expects `dev` inside `prefix` and `prod` elsewhere, for `custom.environment`.
struct ByPrefix {
    version: &'static str,
    prefix: &'static str,
}

impl ExpectedValueSource for ByPrefix {
    fn id(&self) -> &str {
        "by-prefix"
    }

    fn version(&self) -> &str {
        self.version
    }

    fn expected(&self, property: &str, object: &ObjectView) -> Option<Suggestion> {
        if property != "custom.environment" {
            return None;
        }
        let inside = object.value.as_deref()?.starts_with(self.prefix);
        Some(Suggestion {
            values: vec![RawInput::Text(if inside { "dev" } else { "prod" }.into())],
            explanation: format!("objects under {} are dev", self.prefix),
        })
    }
}

fn sources(version: &'static str) -> Sources {
    Sources::none().with(Arc::new(ByPrefix {
        version,
        prefix: "10.66.",
    }))
}

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    domain: Uuid,
    env: Uuid,
}

impl Fixture {
    /// Domain `prod` and `custom.environment` (`prod`, `dev`) on prefixes, with the
    /// source `by-prefix` in `mode` and, if `mandatory`, an obligation on the domain.
    async fn new(pool: &PgPool, mode: ExpectedMode, mandatory: bool) -> Self {
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
            env: Uuid::nil(),
        };
        f.applied(
            &sources("1"),
            vec![Operation::CreateDomain {
                local: None,
                slug: "prod".into(),
            }],
        )
        .await;
        f.domain = sqlx::query_scalar("SELECT id FROM addressing_domain")
            .fetch_one(pool)
            .await
            .unwrap();
        let declaration = DeclarationInput {
            key: "custom.environment".into(),
            value_type: TypeInput::Enum,
            cardinality: Cardinality::Single,
            carriers: vec!["prefix".into()],
            inherit: Some("addressing".into()),
            module: None,
            obligations: if mandatory {
                vec![ObligationInput::Domain { id: f.domain }]
            } else {
                Vec::new()
            },
            expected: Some(Expected {
                source: "by-prefix".into(),
                mode,
            }),
        };
        f.applied(
            &sources("1"),
            vec![
                Operation::DeclareProperty {
                    local: Some(1),
                    declaration,
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
            ],
        )
        .await;
        f.env = sqlx::query_scalar("SELECT id FROM property_declaration")
            .fetch_one(pool)
            .await
            .unwrap();
        f
    }

    async fn submit(&self, sources: &Sources, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit_with(
            &self.store,
            &Unrestricted,
            sources,
            &request,
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
    }

    /// Applies; returns the identifiers of the warnings.
    async fn applied(&self, sources: &Sources, operations: Vec<Operation>) -> Vec<&'static str> {
        match self.submit(sources, operations).await {
            Outcome::Applied { warnings, .. } => warnings.iter().map(|v| v.rule.id).collect(),
            other => panic!("expected applied, got {other:?}"),
        }
    }

    fn container(&self, local: u32, value: &str) -> Operation {
        Operation::CreatePrefix {
            local: Some(local),
            domain: Ref::Id(self.domain),
            prefix: value.into(),
            nature: Nature::Container,
            point_to_point: false,
            anycast: false,
        }
    }

    fn assign(&self, object: Ref, value: &str) -> Operation {
        Operation::AssignProperty {
            object,
            kind: ObjectKind::Prefix,
            property: Ref::Id(self.env),
            values: vec![RawInput::Text(value.into())],
        }
    }

    /// Key and provenance of the current assignment of a prefix.
    async fn assignment(&self, prefix: &str) -> Option<(String, String)> {
        sqlx::query_as(
            "SELECT e.key, a.provenance FROM property_assignment a
             JOIN prefix p ON p.id = a.object_id AND p.prefix = $1::cidr
             JOIN property_enum_value e ON e.id = a.v_uuid
             WHERE a.valid_period @> now()",
        )
        .bind(prefix)
        .fetch_optional(&self.pool)
        .await
        .unwrap()
    }

    async fn prefix_id(&self, prefix: &str) -> Uuid {
        sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = $1::cidr")
            .bind(prefix)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn default_values_are_written_with_their_provenance(pool: PgPool) {
    let f = Fixture::new(&pool, ExpectedMode::Default, true).await;
    // REQ-003-22: created without a value, under an obligation: the default fills it.
    f.applied(&sources("1"), vec![f.container(1, "10.66.0.0/16")])
        .await;
    f.applied(&sources("1"), vec![f.container(1, "10.0.0.0/8")])
        .await;
    assert_eq!(
        f.assignment("10.66.0.0/16").await,
        Some(("dev".into(), "source:by-prefix@1".into()))
    );
    assert_eq!(
        f.assignment("10.0.0.0/8").await,
        Some(("prod".into(), "source:by-prefix@1".into()))
    );
    // An explicit assignment wins over the default.
    f.applied(
        &sources("1"),
        vec![
            f.container(1, "10.67.0.0/16"),
            f.assign(Ref::Local(1), "dev"),
        ],
    )
    .await;
    assert_eq!(
        f.assignment("10.67.0.0/16").await,
        Some(("dev".into(), "input".into()))
    );
    // A new version of the source changes nothing already written.
    f.applied(&sources("2"), vec![f.container(1, "10.68.0.0/16")])
        .await;
    assert_eq!(
        f.assignment("10.66.0.0/16").await,
        Some(("dev".into(), "source:by-prefix@1".into()))
    );
    assert_eq!(
        f.assignment("10.68.0.0/16").await.map(|a| a.1),
        Some("source:by-prefix@2".into())
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_declared_source_must_exist(pool: PgPool) {
    let f = Fixture::new(&pool, ExpectedMode::Suggest, false).await;
    let declaration = DeclarationInput {
        key: "custom.other".into(),
        value_type: TypeInput::Enum,
        cardinality: Cardinality::Single,
        carriers: vec!["prefix".into()],
        inherit: None,
        module: None,
        obligations: Vec::new(),
        expected: Some(Expected {
            source: "nowhere".into(),
            mode: ExpectedMode::Check,
        }),
    };
    match f
        .submit(
            &sources("1"),
            vec![Operation::DeclareProperty {
                local: None,
                declaration,
            }],
        )
        .await
    {
        Outcome::Refused { violations, .. } => {
            assert_eq!(violations[0].rule.id, "prop.declaration.invalid");
        }
        other => panic!("expected refused, got {other:?}"),
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn deviations_become_anomalies(pool: PgPool) {
    let f = Fixture::new(&pool, ExpectedMode::Check, false).await;
    // REQ-003-24: declared `prod` where `dev` is expected → warning, anomaly on the object.
    let warnings = f
        .applied(
            &sources("1"),
            vec![
                f.container(1, "10.66.0.0/16"),
                f.assign(Ref::Local(1), "prod"),
            ],
        )
        .await;
    assert_eq!(warnings, vec!["prop.resolution.expected_mismatch"]);
    let object = f.prefix_id("10.66.0.0/16").await;
    let open = || {
        sqlx::query_scalar::<_, String>(
            "SELECT message FROM anomaly WHERE object_id = $1 AND rule_id = $2 AND closed_at IS NULL",
        )
        .bind(object)
        .bind("prop.resolution.expected_mismatch")
        .fetch_optional(&f.pool)
    };
    let details = open().await.unwrap().expect("an open anomaly");
    assert!(details.contains("objects under 10.66."), "{details}");
    // The resolved value stays `prod` (D23); correcting it closes the anomaly.
    assert_eq!(
        f.assignment("10.66.0.0/16").await.map(|a| a.0),
        Some("prod".into())
    );
    let warnings = f
        .applied(&sources("1"), vec![f.assign(Ref::Id(object), "dev")])
        .await;
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(open().await.unwrap(), None);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn suggestions_write_nothing(pool: PgPool) {
    let f = Fixture::new(&pool, ExpectedMode::Suggest, false).await;
    f.applied(&sources("1"), vec![f.container(1, "10.66.0.0/16")])
        .await;
    let object = f.prefix_id("10.66.0.0/16").await;
    // REQ-003-23: for an existing object, nothing written (suggest mode writes no default).
    let s = suggest(&f.store, &sources("1"), f.env, object, None)
        .await
        .unwrap()
        .expect("a suggestion");
    assert_eq!(s.values, vec![RawInput::Text("dev".into())]);
    assert_eq!(f.assignment("10.66.0.0/16").await, None);
    // For an object being entered, from its view.
    let mut tx = kartenn_service::Store::begin(&f.store).await.unwrap();
    let declaration = kartenn_service::WriteTx::declaration(&mut tx, f.env, None)
        .await
        .unwrap();
    kartenn_service::WriteTx::rollback(tx).await.unwrap();
    let draft = ObjectView {
        id: Uuid::nil(),
        kind: "prefix".into(),
        domain: f.domain,
        value: Some("10.1.0.0/16".into()),
        properties: BTreeMap::new(),
    };
    let s = suggest_for(&sources("1"), declaration.as_ref(), Some(&draft)).expect("a suggestion");
    assert_eq!(s.values, vec![RawInput::Text("prod".into())]);
}
