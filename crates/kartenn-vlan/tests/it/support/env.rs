//! The environment of the tests of the module through the write pipeline: the module
//! migrated, the properties of the core declared for its types, the entities `acme` and
//! `other`, the site `paris` and the domain `lab` owned by `acme`.

use std::sync::Arc;

use kartenn_service::authz::{Authorizer, Subject, Unrestricted};
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::module::ModuleRegistry;
use kartenn_service::operations::{PeriodSpec, RawInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::write::PgStore;
use kartenn_vlan::Vlan;
use serde_json::Value;
use sqlx::PgPool;
use sqlx::types::Uuid;

pub(crate) struct Env {
    pub(crate) pool: PgPool,
    pub(crate) registry: Arc<ModuleRegistry>,
    pub(crate) store: PgStore,
    pub(crate) author: Uuid,
}

impl Env {
    pub(crate) async fn new(pool: &PgPool) -> Self {
        let registry = Arc::new(ModuleRegistry::new(vec![Arc::new(Vlan)]).unwrap());
        kartenn_store_pg::modules::migrate(pool, &registry)
            .await
            .unwrap();
        let store = PgStore::new(pool.clone()).with_modules(registry.clone());
        // As the deployment: the core completed by the types of the module, then the
        // declarations of the module.
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
        let env = Self {
            pool: pool.clone(),
            registry,
            store,
            author,
        };
        env.applied(vec![
            Operation::CreateEntity {
                local: Some(1),
                slug: "acme".into(),
                parent: None,
            },
            Operation::CreateEntity {
                local: Some(2),
                slug: "other".into(),
                parent: None,
            },
            Operation::CreateSite {
                local: Some(3),
                slug: "paris".into(),
                parent: None,
            },
        ])
        .await;
        let (owner, acme) = (
            env.property("core.owner").await,
            env.id("entity", "acme").await,
        );
        // A domain has its owner from its creation (REQ-003-16).
        env.applied(vec![
            Operation::CreateDomain {
                local: Some(4),
                slug: "lab".into(),
            },
            Operation::AssignProperty {
                object: Ref::Local(4),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(acme.to_string())],
            },
        ])
        .await;
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

    /// The resolved owner of `object` now.
    #[allow(dead_code)] // not every test binary reads owners
    pub(crate) async fn owner(&self, object: Uuid) -> Option<Uuid> {
        sqlx::query_scalar("SELECT kartenn_owner($1, now())")
            .bind(object)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// A person granted `vlan.group.read` on the entity `entity`.
    #[allow(dead_code)] // not every test binary reads under rights
    pub(crate) async fn reader_on(&self, entity: Uuid) -> Uuid {
        let identity: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'r') RETURNING id",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let role = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO role (id, key, labels, valid_period, changeset_id)
             VALUES ($1, $2, jsonb_build_object('fr', 'r'), tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(role)
        .bind(format!("r_{}", role.simple()))
        .execute(&self.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
             VALUES ($1, 'vlan.group.read', tstzrange(now() - interval '1 day', NULL),
                     gen_random_uuid())",
        )
        .bind(role)
        .execute(&self.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                                  valid_period, changeset_id)
             VALUES (gen_random_uuid(), 'identity', $1, $2, 'entity', $3,
                     tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(identity)
        .bind(role)
        .bind(entity)
        .execute(&self.pool)
        .await
        .unwrap();
        identity
    }

    /// Whether `reader` may read the group `group`.
    #[allow(dead_code)] // not every test binary reads under rights
    pub(crate) async fn reads(&self, reader: Uuid, group: Uuid) -> bool {
        PgAuthorizer::new(self.pool.clone())
            .with_modules(self.registry.clone())
            .authorize_many(&Subject::from(reader), "vlan.group.read", &[group])
            .await
            .unwrap()
            .is_empty()
    }
}

/// `vlan.<name>` from its JSON fields.
pub(crate) fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("vlan.{name}").into();
    serde_json::from_value(object).unwrap()
}

#[allow(dead_code)] // not every test binary reads the rules
pub(crate) fn rules(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        other => panic!("not refused: {other:?}"),
    }
}

#[allow(dead_code)] // not every test binary reads the messages
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
