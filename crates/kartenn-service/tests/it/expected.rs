//! Sources of expected values: default values, deviations and suggestions (spec 003,
//! REQ-003-21 to 24; task 003-13).

use std::collections::BTreeMap;
use std::sync::Arc;

use kartenn_core::objects::Nature;
use kartenn_core::properties::{Expected, ExpectedMode};
use kartenn_core::values::Cardinality;
use kartenn_service::StoreError;
use kartenn_service::authz::Unrestricted;
use kartenn_service::expected::{
    Expected as Answer, ExpectedContext, ExpectedValueSource, Sources, Suggestion,
    reevaluate_outdated, suggest, suggest_for,
};
use kartenn_service::module::{BoxFuture, ModuleTx};
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

impl ByPrefix {
    fn answer(&self, property: &str, object: &ObjectView) -> Answer {
        let Some(value) = object.value.as_deref() else {
            return Answer::None;
        };
        if property != "custom.environment" {
            return Answer::None;
        }
        let inside = value.starts_with(self.prefix);
        Answer::One(Suggestion {
            values: vec![RawInput::Text(if inside { "dev" } else { "prod" }.into())],
            explanation: format!("objects under {} are dev", self.prefix),
            detail: None,
        })
    }
}

impl ExpectedValueSource for ByPrefix {
    fn id(&self) -> &'static str {
        "by-prefix"
    }

    fn revision<'a>(
        &'a self,
        _ctx: ExpectedContext,
        _tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<String, StoreError>> {
        Box::pin(async move { Ok(self.version.to_owned()) })
    }

    fn expected<'a>(
        &'a self,
        property: &'a str,
        object: &'a ObjectView,
        _ctx: ExpectedContext,
        _tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Answer, StoreError>> {
        Box::pin(async move { Ok(self.answer(property, object)) })
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
            value_type: TypeInput::ENUM,
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
            labels: None,
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
                    labels: std::collections::BTreeMap::new(),
                },
                Operation::AddEnumValue {
                    local: None,
                    property: Ref::Local(1),
                    key: "dev".into(),
                    labels: std::collections::BTreeMap::new(),
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

#[kartenn_testing::pg_test]
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

#[kartenn_testing::pg_test]
async fn a_declared_source_must_exist(pool: PgPool) {
    let f = Fixture::new(&pool, ExpectedMode::Suggest, false).await;
    let declaration = DeclarationInput {
        key: "custom.other".into(),
        value_type: TypeInput::ENUM,
        cardinality: Cardinality::Single,
        carriers: vec!["prefix".into()],
        inherit: None,
        module: None,
        obligations: Vec::new(),
        expected: Some(Expected {
            source: "nowhere".into(),
            mode: ExpectedMode::Check,
        }),
        labels: None,
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

#[kartenn_testing::pg_test]
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

#[kartenn_testing::pg_test]
async fn suggestions_write_nothing(pool: PgPool) {
    let f = Fixture::new(&pool, ExpectedMode::Suggest, false).await;
    f.applied(&sources("1"), vec![f.container(1, "10.66.0.0/16")])
        .await;
    let object = f.prefix_id("10.66.0.0/16").await;
    // REQ-003-23: for an existing object, nothing written (suggest mode writes no default).
    let s = suggest(&f.store, &sources("1"), f.env, object, None)
        .await
        .unwrap();
    let s = s.single().expect("a suggestion");
    assert_eq!(s.values, vec![RawInput::Text("dev".into())]);
    assert_eq!(f.assignment("10.66.0.0/16").await, None);
    // For an object being entered, from its view.
    let mut tx = kartenn_service::Store::begin(&f.store).await.unwrap();
    let declaration = kartenn_service::WriteTx::declaration(&mut tx, f.env, None)
        .await
        .unwrap();
    let draft = ObjectView {
        id: Uuid::nil(),
        kind: "prefix".into(),
        domain: f.domain,
        value: Some("10.1.0.0/16".into()),
        properties: BTreeMap::new(),
    };
    let s = suggest_for(
        &sources("1"),
        declaration.as_ref(),
        Some(&draft),
        ExpectedContext::default(),
        kartenn_service::WriteTx::module_tx(&mut tx),
    )
    .await
    .unwrap();
    kartenn_service::WriteTx::rollback(tx).await.unwrap();
    let s = s.single().expect("a suggestion");
    assert_eq!(s.values, vec![RawInput::Text("prod".into())]);
}

#[kartenn_testing::pg_test]
async fn a_new_source_version_reevaluates_the_deviations(pool: PgPool) {
    let f = Fixture::new(&pool, ExpectedMode::Check, false).await;
    // Version 1 expects `dev` under 10.66: both containers match their expectation.
    let v1 = sources("1");
    let warnings = f
        .applied(
            &v1,
            vec![
                f.container(1, "10.66.0.0/16"),
                f.assign(Ref::Local(1), "dev"),
                f.container(2, "10.67.0.0/16"),
                f.assign(Ref::Local(2), "prod"),
            ],
        )
        .await;
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(reevaluate_outdated(&f.store, &v1, 1).await.unwrap(), 2);
    assert_eq!(
        reevaluate_outdated(&f.store, &v1, 1).await.unwrap(),
        0,
        "up to date"
    );
    // Version 2 expects `dev` under 10.67 instead: both containers deviate now.
    let v2 = Sources::none().with(Arc::new(ByPrefix {
        version: "2",
        prefix: "10.67.",
    }));
    assert_eq!(reevaluate_outdated(&f.store, &v2, 1).await.unwrap(), 2);
    let open: Vec<Uuid> = sqlx::query_scalar(
        "SELECT object_id FROM anomaly
         WHERE rule_id = 'prop.resolution.expected_mismatch' AND closed_at IS NULL ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let mut expected = vec![
        f.prefix_id("10.66.0.0/16").await,
        f.prefix_id("10.67.0.0/16").await,
    ];
    expected.sort();
    assert_eq!(open, expected);
    // Each batch is journaled as a system changeset.
    let batches: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM changeset WHERE motive = 'expected re-evaluation by-prefix@2'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(batches, 2);
}

/// Two candidates for `custom.environment`: `prod` and `dev` (REQ-016-5).
struct Ambiguous;

impl ExpectedValueSource for Ambiguous {
    fn id(&self) -> &'static str {
        "by-prefix"
    }

    fn revision<'a>(
        &'a self,
        _ctx: ExpectedContext,
        _tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<String, StoreError>> {
        Box::pin(async { Ok("1".to_owned()) })
    }

    fn expected<'a>(
        &'a self,
        _property: &'a str,
        _object: &'a ObjectView,
        _ctx: ExpectedContext,
        _tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Answer, StoreError>> {
        let candidate = |value: &str| Suggestion {
            values: vec![RawInput::Text(value.into())],
            explanation: format!("{value} is as likely"),
            detail: None,
        };
        Box::pin(async move {
            Ok(Answer::Candidates(vec![
                candidate("prod"),
                candidate("dev"),
            ]))
        })
    }
}

#[kartenn_testing::pg_test]
async fn two_candidates_no_default_no_mismatch(pool: PgPool) {
    let f = Fixture::new(&pool, ExpectedMode::Default, false).await;
    let ambiguous = Sources::none().with(Arc::new(Ambiguous));
    // REQ-016-5: no default value with two candidates…
    f.applied(&ambiguous, vec![f.container(1, "10.66.0.0/16")])
        .await;
    assert_eq!(f.assignment("10.66.0.0/16").await, None);
    // … both suggested…
    let object = f.prefix_id("10.66.0.0/16").await;
    let found = suggest(&f.store, &ambiguous, f.env, object, None)
        .await
        .unwrap();
    assert!(found.single().is_none());
    assert_eq!(
        found
            .candidates()
            .iter()
            .map(|c| c.values.clone())
            .collect::<Vec<_>>(),
        [
            vec![RawInput::Text("prod".into())],
            vec![RawInput::Text("dev".into())]
        ]
    );
    // … and no deviation: in check mode, a value with two candidates deviates from none,
    // where one candidate other than the value does.
    sqlx::query("UPDATE property_declaration SET expected_mode = 'check' WHERE id = $1")
        .bind(f.env)
        .execute(&pool)
        .await
        .unwrap();
    let warnings = f
        .applied(&ambiguous, vec![f.assign(Ref::Id(object), "prod")])
        .await;
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        f.applied(&sources("1"), vec![f.assign(Ref::Id(object), "prod")])
            .await,
        vec!["prop.resolution.expected_mismatch"]
    );
}

/// Expects the value of `custom.rule` on the domain of the object, read in the
/// transaction under way (REQ-016-4).
struct FromRule;

impl ExpectedValueSource for FromRule {
    fn id(&self) -> &'static str {
        "by-prefix"
    }

    fn revision<'a>(
        &'a self,
        _ctx: ExpectedContext,
        _tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<String, StoreError>> {
        Box::pin(async { Ok("1".to_owned()) })
    }

    fn expected<'a>(
        &'a self,
        _property: &'a str,
        object: &'a ObjectView,
        _ctx: ExpectedContext,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Answer, StoreError>> {
        Box::pin(async move {
            let connection = tx
                .connection()
                .downcast_mut::<sqlx::PgConnection>()
                .expect("the connection of kartenn-store-pg");
            let rule: Option<String> = sqlx::query_scalar(
                "SELECT a.v_text FROM property_assignment a
                 JOIN property_declaration d ON d.id = a.property_id AND d.key = 'custom.rule'
                 WHERE a.object_id = $1 AND a.valid_period @> now()",
            )
            .bind(object.domain)
            .fetch_optional(&mut *connection)
            .await
            .map_err(|e| StoreError(e.to_string()))?;
            Ok(rule.map_or(Answer::None, |rule| {
                Answer::One(Suggestion {
                    values: vec![RawInput::Text(rule)],
                    explanation: "the rule of the domain".into(),
                    detail: Some("rule".into()),
                })
            }))
        })
    }
}

#[kartenn_testing::pg_test]
async fn a_rule_written_in_the_changeset_applies_to_its_creations(pool: PgPool) {
    let f = Fixture::new(&pool, ExpectedMode::Default, false).await;
    let from_rule = Sources::none().with(Arc::new(FromRule));
    f.applied(
        &from_rule,
        vec![Operation::DeclareProperty {
            local: Some(1),
            declaration: DeclarationInput {
                key: "custom.rule".into(),
                value_type: TypeInput::Text {
                    min_len: 1,
                    max_len: 10,
                    pattern: None,
                },
                cardinality: Cardinality::Single,
                carriers: vec!["domain".into()],
                inherit: None,
                module: None,
                obligations: Vec::new(),
                expected: None,
                labels: None,
            },
        }],
    )
    .await;
    let rule: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'custom.rule'")
            .fetch_one(&pool)
            .await
            .unwrap();
    // REQ-016-4: the rule and the object it applies to, in one changeset; the provenance
    // names what gave the value (REQ-016-5).
    f.applied(
        &from_rule,
        vec![
            Operation::AssignProperty {
                object: Ref::Id(f.domain),
                kind: ObjectKind::Domain,
                property: Ref::Id(rule),
                values: vec![RawInput::Text("dev".into())],
            },
            f.container(1, "10.1.0.0/16"),
        ],
    )
    .await;
    assert_eq!(
        f.assignment("10.1.0.0/16").await,
        Some(("dev".into(), "source:by-prefix@1#rule".into()))
    );
}

/// Expects `prod` everywhere, counting its calls (REQ-016-4).
struct Counting {
    revision: &'static str,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}

impl ExpectedValueSource for Counting {
    fn id(&self) -> &'static str {
        "by-prefix"
    }

    fn revision<'a>(
        &'a self,
        _ctx: ExpectedContext,
        _tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<String, StoreError>> {
        Box::pin(async move { Ok(self.revision.to_owned()) })
    }

    fn expected<'a>(
        &'a self,
        _property: &'a str,
        _object: &'a ObjectView,
        _ctx: ExpectedContext,
        _tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Answer, StoreError>> {
        Box::pin(async {
            Ok(Answer::One(Suggestion {
                values: vec![RawInput::Text("prod".into())],
                explanation: "all prod".into(),
                detail: None,
            }))
        })
    }

    fn expected_many<'a>(
        &'a self,
        _property: &'a str,
        objects: &'a [ObjectView],
        _ctx: ExpectedContext,
        _tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Vec<Answer>, StoreError>> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Box::pin(async move {
            Ok(objects
                .iter()
                .map(|_| {
                    Answer::One(Suggestion {
                        values: vec![RawInput::Text("prod".into())],
                        explanation: "all prod".into(),
                        detail: None,
                    })
                })
                .collect())
        })
    }
}

#[kartenn_testing::pg_test]
async fn ten_thousand_objects_in_batches(pool: PgPool) {
    let f = Fixture::new(&pool, ExpectedMode::Check, false).await;
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counting = |revision| {
        Sources::none().with(Arc::new(Counting {
            revision,
            calls: Arc::clone(&calls),
        }))
    };
    let v1 = counting("1");
    for half in 0..2_u32 {
        let operations = (0..5_000_u32)
            .map(|i| {
                let n = half * 5_000 + i;
                f.container(n + 1, &format!("10.{}.{}.0/24", n / 256, n % 256))
            })
            .collect();
        f.applied(&v1, operations).await;
    }
    // REQ-016-4: one call per changeset, then one per batch of a re-evaluation.
    assert_eq!(calls.swap(0, std::sync::atomic::Ordering::Relaxed), 2);
    let evaluated = reevaluate_outdated(&f.store, &counting("2"), 1_000)
        .await
        .unwrap();
    assert_eq!(evaluated, 10_000);
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 10);
}
