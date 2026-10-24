//! The environment of the tests of the module through the write pipeline: the modules the
//! policy needs (VLAN, machines) and the policy migrated, the declarations of the core and
//! of the modules synchronised, the entity `acme` and the domain `lab` it owns.

use std::sync::Arc;

use kartenn_policy::Policy;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::module::ModuleRegistry;
use kartenn_service::operations::{PeriodSpec, RawInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use serde_json::Value;
use sqlx::PgPool;
use sqlx::types::Uuid;

pub(crate) struct Env {
    pub(crate) pool: PgPool,
    pub(crate) store: PgStore,
    pub(crate) author: Uuid,
    pub(crate) domain: Uuid,
    /// The modules of the environment, as an instance serves them.
    pub(crate) registry: Arc<ModuleRegistry>,
}

impl Env {
    pub(crate) async fn new(pool: &PgPool) -> Self {
        let registry = Arc::new(
            ModuleRegistry::new(vec![
                Arc::new(kartenn_vlan::Vlan),
                Arc::new(kartenn_machines::Machines),
                Arc::new(Policy),
            ])
            .unwrap(),
        );
        kartenn_store_pg::modules::migrate(pool, &registry)
            .await
            .unwrap();
        let store = PgStore::new(pool.clone()).with_modules(registry.clone());
        let declarations: Vec<_> = std::iter::once(core_declarations_for(&registry))
            .chain(registry.modules().filter_map(|m| m.declarations()))
            .collect();
        for outcome in sync_modules(&store, &Unrestricted, &declarations, MAX_OPERATIONS)
            .await
            .unwrap()
        {
            assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        }
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let mut env = Self {
            pool: pool.clone(),
            store,
            registry,
            author,
            domain: Uuid::nil(),
        };
        env.applied(vec![Operation::CreateEntity {
            local: Some(1),
            slug: "acme".into(),
            parent: None,
        }])
        .await;
        let (owner, acme) = (
            env.property("core.owner").await,
            env.id("entity", "acme").await,
        );
        env.applied(vec![
            Operation::CreateDomain {
                local: Some(2),
                slug: "lab".into(),
            },
            Operation::AssignProperty {
                object: Ref::Local(2),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(acme.to_string())],
            },
        ])
        .await;
        env.domain = env.id("addressing_domain", "lab").await;
        env
    }

    /// The request of `operations` from `start` on (microseconds), or from now.
    pub(crate) fn request(
        &self,
        operations: Vec<Operation>,
        start: Option<i64>,
    ) -> ChangesetRequest {
        ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: start.map(|start| PeriodSpec { start, end: None }),
            operations,
            simulate: false,
            idempotency_key: None,
            apply_own: true,
        }
    }

    pub(crate) async fn submit(&self, operations: Vec<Operation>) -> Outcome {
        self.submit_from(operations, None).await
    }

    pub(crate) async fn submit_from(
        &self,
        operations: Vec<Operation>,
        start: Option<i64>,
    ) -> Outcome {
        submit(
            &self.store,
            &Unrestricted,
            &self.request(operations, start),
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
    }

    pub(crate) async fn applied(&self, operations: Vec<Operation>) -> Outcome {
        let outcome = self.submit(operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        outcome
    }

    pub(crate) async fn property(&self, key: &str) -> Uuid {
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = $1")
            .bind(key)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    pub(crate) async fn id(&self, table: &str, slug: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE slug = $1 AND valid_period @> now()"
        )))
        .bind(slug)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The revision of the policy now.
    pub(crate) async fn revision(&self) -> i64 {
        sqlx::query_scalar("SELECT policy FROM policy.policy_revision")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

/// `policy.<name>` from its JSON fields.
pub(crate) fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("policy.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// The identifiers of the rules that refused `outcome`.
pub(crate) fn rules(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        other => panic!("not refused: {other:?}"),
    }
}

/// The messages of the refusal `outcome`, joined.
pub(crate) fn messages(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Refused { violations, .. } => violations
            .iter()
            .map(|v| v.message.clone())
            .collect::<Vec<_>>()
            .join("; "),
        other => panic!("not refused: {other:?}"),
    }
}
