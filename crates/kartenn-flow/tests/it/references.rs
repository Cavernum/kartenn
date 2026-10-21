//! The references of the endpoints (task 011-4, REQ-011-3): closing an address that a
//! living flow names is refused, the flow listed; the cascade closes the flow with it.
//! The flow is written as the operations of task 011-3 will write it: its row and its
//! record in `module_object`.

use std::sync::Arc;

use kartenn_core::objects::AddressUsage;
use kartenn_flow::Flows;
use kartenn_machines::Machines;
use kartenn_policy::Policy;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::module::ModuleRegistry;
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use kartenn_vlan::Vlan;
use sqlx::PgPool;
use sqlx::types::Uuid;

struct Env {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
}

impl Env {
    /// The modules migrated and declared, the entity `acme` and the domain `lab` it owns.
    async fn new(pool: &PgPool) -> Self {
        let registry = Arc::new(
            ModuleRegistry::new(vec![
                Arc::new(Vlan),
                Arc::new(Machines),
                Arc::new(Policy),
                Arc::new(Flows),
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
        let env = Self {
            pool: pool.clone(),
            store,
            author,
        };
        env.applied(vec![Operation::CreateEntity {
            local: Some(1),
            slug: "acme".into(),
            parent: None,
        }])
        .await;
        let owner: Uuid =
            sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'core.owner'")
                .fetch_one(pool)
                .await
                .unwrap();
        let acme = env.slugged("entity", "acme").await;
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
        env
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

    async fn applied(&self, operations: Vec<Operation>) -> Outcome {
        let outcome = self.submit(operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        outcome
    }

    async fn slugged(&self, table: &str, slug: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE slug = $1 AND valid_period @> now()"
        )))
        .bind(slug)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The address `value` of the domain `lab`.
    async fn address(&self, value: &str) -> Uuid {
        let lab = self.slugged("addressing_domain", "lab").await;
        let network: Operation = serde_json::from_value(serde_json::json!({
            "op": "create_prefix", "domain": {"id": lab}, "prefix": "10.0.0.0/24",
            "nature": "network",
        }))
        .unwrap();
        self.applied(vec![network]).await;
        self.applied(vec![Operation::CreateAddress {
            local: None,
            domain: Ref::Id(lab),
            address: value.into(),
            usage: AddressUsage::Host,
        }])
        .await;
        sqlx::query_scalar(
            "SELECT id FROM address WHERE address = $1::inet AND valid_period @> now()",
        )
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// A living flow from the address `source`, written as the module writes one.
    async fn flow_from(&self, source: Uuid) -> Uuid {
        let (id, changeset) = (Uuid::now_v7(), Uuid::now_v7());
        let acme = self.slugged("entity", "acme").await;
        sqlx::query(
            "INSERT INTO flow.flow (id, kind, source_kind, source_id, destination_kind,
                                    destination_id, service_id, external_refs, requester_id,
                                    entity_id, valid_period, changeset_id)
             VALUES ($1, 'requested', 'address', $2, 'prefix', gen_random_uuid(),
                     gen_random_uuid(), '[{\"system\": \"00000000-0000-0000-0000-000000000001\",
                     \"id\": \"CHG-1042\"}]', $3, $4, tstzrange(now() - interval '1 hour', NULL), $5)",
        )
        .bind(id)
        .bind(source)
        .bind(self.author)
        .bind(acme)
        .bind(changeset)
        .execute(&self.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO module_object (id, module, kind, valid_period, changeset_id)
             VALUES ($1, 'flow', 'flow.flow', tstzrange(now() - interval '1 hour', NULL), $2)",
        )
        .bind(id)
        .bind(changeset)
        .execute(&self.pool)
        .await
        .unwrap();
        id
    }

    async fn lives(&self, table: &str, id: Uuid) -> bool {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT EXISTS (SELECT 1 FROM {table} WHERE id = $1 AND valid_period @> now())"
        )))
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn close_address(id: Uuid, cascade: bool) -> Operation {
    Operation::Close {
        kind: ObjectKind::Address,
        id: Ref::Id(id),
        expected: None,
        cascade,
    }
}

#[kartenn_testing::pg_test]
async fn closing_a_cited_address_is_refused_with_its_flows(pool: PgPool) {
    let env = Env::new(&pool).await;
    let address = env.address("10.0.0.1").await;
    let flow = env.flow_from(address).await;
    let refused = env.submit(vec![close_address(address, false)]).await;
    let Outcome::Refused { violations, .. } = &refused else {
        panic!("not refused: {refused:?}");
    };
    assert!(
        violations
            .iter()
            .any(|v| v.rule.id == "core.object.dependents" && v.subjects.contains(&flow)),
        "{violations:?}"
    );
    assert!(env.lives("address", address).await);
    assert!(env.lives("flow.flow", flow).await);
}

#[kartenn_testing::pg_test]
async fn the_cascade_closes_the_flow_with_its_endpoint(pool: PgPool) {
    let env = Env::new(&pool).await;
    let address = env.address("10.0.0.1").await;
    let flow = env.flow_from(address).await;
    let applied = env.applied(vec![close_address(address, true)]).await;
    assert!(!env.lives("address", address).await);
    assert!(!env.lives("flow.flow", flow).await);
    assert!(!env.lives("module_object", flow).await);
    let Outcome::Applied { cascaded, .. } = applied else {
        unreachable!()
    };
    assert!(cascaded.iter().any(|(_, id)| *id == flow), "{cascaded:?}");
}

#[kartenn_testing::pg_test]
async fn an_ended_flow_does_not_hold_its_endpoint(pool: PgPool) {
    let env = Env::new(&pool).await;
    let address = env.address("10.0.0.1").await;
    let flow = env.flow_from(address).await;
    sqlx::query(
        "UPDATE flow.flow SET valid_period = tstzrange(lower(valid_period), now() - interval '1 minute')
          WHERE id = $1",
    )
    .bind(flow)
    .execute(&pool)
    .await
    .unwrap();
    env.applied(vec![close_address(address, false)]).await;
}
