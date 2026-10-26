//! The endpoints of a flow without machine (task 011-14, F21; REQ-011-2): a machine is
//! refused, its interface accepted; the flows a machine was an endpoint of are closed by
//! the migration job.

use kartenn_flow::migration::{MACHINE_MOTIVE, close_machine_flows};
use kartenn_service::Operation;
use kartenn_service::Outcome;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::SYSTEM_AUTHOR;
use kartenn_service::pipeline::MAX_OPERATIONS;
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::requests::{Env, flow, rules};

fn machines(name: &str, fields: serde_json::Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// The machine `name` of `acme`, its interface `eth0` holding `address`: (machine,
/// interface).
pub(crate) async fn machine(env: &Env, name: &str, address: Uuid) -> (Uuid, Uuid) {
    env.applied(vec![
        machines(
            "create_machine",
            json!({ "local": 1, "name": name, "kind": "physical", "owner": { "id": env.acme } }),
        ),
        machines(
            "create_interface",
            json!({ "local": 2, "machine": { "local": 1 }, "name": "eth0" }),
        ),
        machines(
            "attach_address",
            json!({ "interface": { "local": 2 }, "address": { "id": address } }),
        ),
    ])
    .await;
    sqlx::query_as(
        "SELECT m.id, i.id FROM machines.machine m
           JOIN machines.interface i ON i.machine_id = m.id AND i.valid_period @> now()
          WHERE m.name = $1 AND m.valid_period @> now()",
    )
    .bind(name)
    .fetch_one(&env.pool)
    .await
    .unwrap()
}

/// REQ-011-2 (F21): a machine as endpoint is refused, `flow.endpoint.machine`, whether
/// requested or written directly; its interface is accepted.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_2_a_machine_is_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let (web, eth0) = machine(&env, "web", prod).await;
    for name in ["request", "register"] {
        let mut fields = env.fields(prod, dev).await;
        fields["source"] = json!({ "kind": "machine", "id": { "id": web } });
        let outcome = env
            .submit_as(&Unrestricted, env.author, vec![flow(name, fields)])
            .await;
        let found = rules(&outcome);
        assert!(
            found.contains(&"flow.endpoint.machine"),
            "{name}: {found:?}"
        );
    }
    assert_eq!(env.flows().await, 0);
    let mut fields = env.fields(prod, dev).await;
    fields["source"] = json!({ "kind": "interface", "id": { "id": eth0 } });
    env.applied(vec![flow("request", fields)]).await;
    assert_eq!(env.flows().await, 1);
}

/// F21, task 011-14: the migration job closes, by one system changeset that names them,
/// the flows living or to come a machine is an endpoint of, and only them; a second run
/// submits nothing.
#[kartenn_testing::pg_test]
async fn the_migration_closes_the_flows_of_a_machine(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let (web, eth0) = machine(&env, "web", prod).await;
    let mut fields = env.fields(prod, dev).await;
    fields["source"] = json!({ "kind": "interface", "id": { "id": eth0 } });
    env.applied(vec![flow("request", fields)]).await;
    env.applied(vec![flow("request", env.fields(prod, dev).await)])
        .await;
    // A flow registered before F21, its source a machine.
    let legacy: Uuid = sqlx::query_scalar(
        "UPDATE flow.flow SET source_kind = 'machine', source_id = $1
          WHERE source_kind = 'interface' RETURNING id",
    )
    .bind(web)
    .fetch_one(&pool)
    .await
    .unwrap();
    let kept: Uuid = sqlx::query_scalar("SELECT id FROM flow.flow WHERE id <> $1")
        .bind(legacy)
        .fetch_one(&pool)
        .await
        .unwrap();

    let (closed, outcomes) = close_machine_flows(&env.store, &pool, MAX_OPERATIONS)
        .await
        .unwrap();
    assert_eq!(closed, [legacy]);
    let [Outcome::Applied { changeset, .. }] = outcomes.as_slice() else {
        panic!("{outcomes:?}");
    };
    let (author, motive): (Uuid, String) =
        sqlx::query_as("SELECT author_id, motive FROM changeset WHERE id = $1")
            .bind(changeset)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((author, motive.as_str()), (SYSTEM_AUTHOR, MACHINE_MOTIVE));
    let living = |id: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM flow.flow WHERE id = $1
                                  AND (upper(valid_period) IS NULL OR upper(valid_period) > now()))",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert!(!living(legacy).await, "the flow of the machine is closed");
    assert!(living(kept).await, "the flow between addresses is kept");
    let (again, outcomes) = close_machine_flows(&env.store, &pool, MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(again.is_empty() && outcomes.is_empty(), "{again:?}");
}
