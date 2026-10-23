//! Labels written with their owning object (spec 018, REQ-018-9, R22; task 018-7):
//! declarations, closed-list values and roles carry labels by language, written by
//! changeset; a write of labels increments the revision of the labels.

use std::collections::BTreeMap;

use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{DeclarationInput, RawLabels, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
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
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>) -> Uuid {
        match self.submit(operations).await {
            Outcome::Applied { changeset, .. } => changeset,
            other => panic!("expected applied, got {other:?}"),
        }
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

    /// The labels of the value `key` now, with the changeset that wrote them.
    async fn value(&self, key: &str) -> (Value, Uuid) {
        sqlx::query_as(
            "SELECT labels, changeset_id FROM property_enum_value
              WHERE key = $1 AND valid_period @> now()",
        )
        .bind(key)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn revision(&self) -> i64 {
        sqlx::query_scalar("SELECT revision FROM label_revision")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// The revisions the outbox announced, in order.
    async fn announced(&self) -> Vec<i64> {
        sqlx::query_scalar(
            "SELECT (payload->'data'->>'revision')::bigint FROM outbox
              WHERE payload->>'type' = 'org.kartenn.labels.changed' ORDER BY seq",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }
}

fn labels(pairs: &[(&str, &str)]) -> RawLabels {
    pairs
        .iter()
        .map(|(l, t)| ((*l).to_owned(), (*t).to_owned()))
        .collect::<BTreeMap<_, _>>()
}

fn environment(local: u32, labels: Option<RawLabels>) -> Operation {
    Operation::DeclareProperty {
        local: Some(local),
        declaration: DeclarationInput {
            key: "custom.stage".into(),
            value_type: TypeInput::Enum { values_of: None },
            cardinality: Cardinality::Single,
            carriers: vec!["prefix".into()],
            inherit: None,
            module: None,
            obligations: Vec::new(),
            expected: None,
            labels,
        },
    }
}

fn add_value(property: Ref, key: &str, labels: RawLabels) -> Operation {
    Operation::AddEnumValue {
        local: None,
        property,
        key: key.into(),
        labels,
    }
}

fn relabel(property: Uuid, key: &str, labels: RawLabels) -> Operation {
    Operation::UpdateEnumValue {
        property: Ref::Id(property),
        key: key.into(),
        labels,
    }
}

/// REQ-018-9: `update_enum_value` writes a new version of the value under its changeset;
/// the former labels stay in the history.
#[kartenn_testing::pg_test]
async fn relabelling_a_value_is_a_changeset(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        environment(1, None),
        add_value(Ref::Local(1), "prod", labels(&[("fr", "Production")])),
    ])
    .await;
    let property = f.property("custom.stage").await;
    let (first, _) = f.value("prod").await;
    assert_eq!(first, json!({"fr": "Production"}));
    let changeset = f
        .applied(vec![relabel(
            property,
            "prod",
            labels(&[("fr", "Production"), ("EN", "Production line")]),
        )])
        .await;
    let (now, by) = f.value("prod").await;
    assert_eq!(now, json!({"fr": "Production", "en": "Production line"}));
    assert_eq!(by, changeset, "the new version is the changeset's");
    let former: Vec<Value> = sqlx::query_scalar(
        "SELECT labels FROM property_enum_value_history WHERE key = 'prod' ORDER BY sys_start",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(former, [json!({"fr": "Production"})]);
}

/// REQ-003-5, REQ-018-9: a value is relabelled by its key, which never changes; a key
/// that is no value of the list, or labels without the default language, are refused.
#[kartenn_testing::pg_test]
async fn a_value_key_never_changes(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        environment(1, Some(labels(&[("fr", "Étape")]))),
        add_value(Ref::Local(1), "prod", RawLabels::new()),
    ])
    .await;
    let property = f.property("custom.stage").await;
    assert_eq!(
        f.refused(vec![relabel(
            property,
            "production",
            labels(&[("fr", "P")])
        )])
        .await,
        ["prop.enum_value.invalid"]
    );
    assert_eq!(
        f.refused(vec![relabel(property, "Prod", labels(&[("fr", "P")]))])
            .await,
        ["prop.enum_value.invalid"]
    );
    assert_eq!(
        f.refused(vec![relabel(property, "prod", labels(&[("en", "P")]))])
            .await,
        ["core.label.invalid"]
    );
    assert_eq!(
        f.refused(vec![add_value(
            Ref::Id(property),
            "dev",
            labels(&[("fr", "")])
        )])
        .await,
        ["core.label.invalid"]
    );
    let keys: Vec<String> =
        sqlx::query_scalar("SELECT key FROM property_enum_value WHERE property_id = $1")
            .bind(property)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(keys, ["prod"]);
    let declared: Value =
        sqlx::query_scalar("SELECT labels FROM property_declaration WHERE key = 'custom.stage'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(declared, json!({"fr": "Étape"}));
}

/// R22: a changeset that writes labels increments the revision once and announces it in
/// the outbox; one that writes none, or the same labels, leaves it.
#[kartenn_testing::pg_test]
async fn a_label_write_bumps_the_revision(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let start = f.revision().await;
    f.applied(vec![
        environment(1, Some(labels(&[("fr", "Étape")]))),
        add_value(Ref::Local(1), "prod", labels(&[("fr", "Production")])),
    ])
    .await;
    assert_eq!(f.revision().await, start + 1, "one increment per changeset");
    let property = f.property("custom.stage").await;
    f.applied(vec![add_value(Ref::Id(property), "dev", RawLabels::new())])
        .await;
    assert_eq!(f.revision().await, start + 1, "no label written");
    f.applied(vec![relabel(
        property,
        "prod",
        labels(&[("fr", "Production")]),
    )])
    .await;
    assert_eq!(f.revision().await, start + 1, "the same labels");
    f.applied(vec![Operation::SetRole {
        key: "auditor".into(),
        labels: labels(&[("fr", "Auditeur"), ("en", "Auditor")]),
        entries: vec!["core.audit.read".into()],
    }])
    .await;
    assert_eq!(f.revision().await, start + 2, "a role relabelled");
    assert_eq!(f.announced().await, [start + 1, start + 2]);
}
