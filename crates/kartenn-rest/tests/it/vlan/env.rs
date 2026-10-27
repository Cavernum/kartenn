//! The environment of the tests of the VLANs of the core through the write pipeline (spec
//! 026): a build without any module, the properties of the core declared, the entities
//! `acme` and `other`, the site `paris` and the domain `lab` owned by `acme`.

use std::sync::Arc;

use kartenn_service::authz::{Authorizer, Subject, Unrestricted};
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::module::ModuleRegistry;
use kartenn_service::operations::{PeriodSpec, RawInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::write::PgStore;
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
        // REQ-026-1: no module, the VLANs are in the core.
        let registry = Arc::new(ModuleRegistry::new(Vec::new()).unwrap());
        let store = PgStore::new(pool.clone()).with_modules(registry.clone());
        // As the deployment: the declarations of the core.
        for outcome in sync_modules(
            &store,
            &Unrestricted,
            &[core_declarations_for(&registry)],
            MAX_OPERATIONS,
        )
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
            approval_self: true,
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
    pub(crate) async fn owner(&self, object: Uuid) -> Option<Uuid> {
        sqlx::query_scalar("SELECT kartenn_owner($1, now())")
            .bind(object)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// A person granted `core.vlan_group.read` on the entity `entity`.
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
             VALUES ($1, 'core.vlan_group.read', tstzrange(now() - interval '1 day', NULL),
                     gen_random_uuid())",
        )
        .bind(role)
        .execute(&self.pool)
        .await
        .unwrap();
        sqlx::query(kartenn_testing::BIND_IDENTITY)
            .bind(identity)
            .bind(role)
            .bind("entity")
            .bind(entity)
            .execute(&self.pool)
            .await
            .unwrap();
        identity
    }

    /// Whether `reader` may read the group `group`.
    pub(crate) async fn reads(&self, reader: Uuid, group: Uuid) -> bool {
        PgAuthorizer::new(self.pool.clone())
            .with_modules(self.registry.clone())
            .authorize_many(&Subject::from(reader), "core.vlan_group.read", &[group])
            .await
            .unwrap()
            .is_empty()
    }
}

/// The operation of the VLANs of the core `name` from its JSON fields.
pub(crate) fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = name.into();
    serde_json::from_value(object).unwrap()
}

pub(crate) fn rules(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        other => panic!("not refused: {other:?}"),
    }
}

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
