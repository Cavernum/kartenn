//! The coherence of the machines (spec 010, REQ-010-17; task 010-14): the anomaly
//! `policy.machine.environment_mismatch`, opened and closed on the machines a changeset
//! touches only.

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::operations::{ModuleKind, RawInput};
use kartenn_service::{ObjectKind, Operation, Outcome, Ref};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::support::env::{Env, op};

const MISMATCH: &str = "policy.machine.environment_mismatch";

fn machines(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

fn module(kind: &str) -> ObjectKind {
    ObjectKind::Module(ModuleKind::parse(kind).unwrap())
}

/// The network `prefix`, classified `environment`, and its address `address`.
async fn classified(env: &Env, prefix: &str, environment: &str, address: &str) -> Uuid {
    let property = env.property("policy.environment").await;
    env.applied(vec![
        Operation::CreatePrefix {
            local: Some(1),
            domain: Ref::Id(env.domain),
            prefix: prefix.into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Prefix,
            property: Ref::Id(property),
            values: vec![RawInput::Text(environment.into())],
        },
        Operation::CreateAddress {
            local: None,
            domain: Ref::Id(env.domain),
            address: address.into(),
            usage: AddressUsage::Host,
        },
    ])
    .await;
    sqlx::query_scalar("SELECT id FROM address WHERE address = $1::inet")
        .bind(address)
        .fetch_one(&env.pool)
        .await
        .unwrap()
}

/// The machine `name`, one interface per address, named `eth0`, `eth1`…; its id and the
/// ids of its interfaces.
async fn machine(env: &Env, name: &str, addresses: &[Uuid]) -> (Uuid, Vec<Uuid>) {
    let acme = env.id("entity", "acme").await;
    let mut operations = vec![machines(
        "create_machine",
        json!({ "local": 1, "name": name, "kind": "physical", "owner": { "id": acme } }),
    )];
    for (n, address) in addresses.iter().enumerate() {
        let local = 10 + n;
        operations.push(machines(
            "create_interface",
            json!({ "local": local, "machine": { "local": 1 }, "name": format!("eth{n}") }),
        ));
        operations.push(machines(
            "attach_address",
            json!({ "interface": { "local": local }, "address": { "id": address } }),
        ));
    }
    env.applied(operations).await;
    let id: Uuid = sqlx::query_scalar(
        "SELECT id FROM machines.machine WHERE name = $1 AND valid_period @> now()",
    )
    .bind(name)
    .fetch_one(&env.pool)
    .await
    .unwrap();
    let interfaces = sqlx::query_scalar(
        "SELECT id FROM machines.interface WHERE machine_id = $1 AND valid_period @> now()
          ORDER BY name",
    )
    .bind(id)
    .fetch_all(&env.pool)
    .await
    .unwrap();
    (id, interfaces)
}

/// Assigns the closed-list value `value` of `key` to `object` of `kind`.
async fn assign(env: &Env, object: Uuid, kind: &str, key: &str, value: &str) -> Outcome {
    let property = env.property(key).await;
    env.applied(vec![Operation::AssignProperty {
        object: Ref::Id(object),
        kind: module(kind),
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.into())],
    }])
    .await
}

/// Whether an anomaly of the coherence is open on `machine`.
async fn open(env: &Env, machine: Uuid) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM anomaly
                         WHERE object_id = $1 AND rule_id = $2 AND closed_at IS NULL)",
    )
    .bind(machine)
    .bind(MISMATCH)
    .fetch_one(&env.pool)
    .await
    .unwrap()
}

/// The rules of the warnings of an applied `outcome`.
fn warnings(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Applied { warnings, .. } => warnings.iter().map(|w| w.rule.id).collect(),
        other => panic!("not applied: {other:?}"),
    }
}

async fn environments(env: &Env) {
    env.applied(vec![
        op("environment.create", json!({ "key": "prod" })),
        op("environment.create", json!({ "key": "admin" })),
        op("environment.create", json!({ "key": "dev" })),
    ])
    .await;
}

#[kartenn_testing::pg_test]
async fn acceptance_of_req_010_17(pool: PgPool) {
    let env = Env::new(&pool).await;
    environments(&env).await;
    let service = classified(&env, "10.1.0.0/24", "prod", "10.1.0.10").await;
    let management = classified(&env, "10.9.0.0/24", "admin", "10.9.0.10").await;
    let (web, interfaces) = machine(&env, "web", &[service, management]).await;
    // Declared {prod}, an administration address {admin} without the role: an anomaly.
    let declared = assign(
        &env,
        web,
        "machines.machine",
        "policy.declared_environment",
        "prod",
    )
    .await;
    assert_eq!(warnings(&declared), [MISMATCH]);
    assert!(open(&env, web).await);
    // The address on an interface of the role administration (exempt by default): closed.
    assign(
        &env,
        interfaces[1],
        "machines.interface",
        "machines.interface_role",
        "administration",
    )
    .await;
    assert!(!open(&env, web).await);
    // The role taken away (service): open again.
    assign(
        &env,
        interfaces[1],
        "machines.interface",
        "machines.interface_role",
        "service",
    )
    .await;
    assert!(open(&env, web).await);
}

#[kartenn_testing::pg_test]
async fn an_undeclared_machine_diverges_on_two_environments(pool: PgPool) {
    let env = Env::new(&pool).await;
    environments(&env).await;
    let a = classified(&env, "10.1.0.0/24", "prod", "10.1.0.10").await;
    let b = classified(&env, "10.2.0.0/24", "dev", "10.2.0.10").await;
    let c = classified(&env, "10.3.0.0/24", "prod", "10.3.0.10").await;
    let d = classified(&env, "10.4.0.0/24", "prod", "10.4.0.10").await;
    let (same, _) = machine(&env, "same", &[a, c]).await;
    assert!(!open(&env, same).await);
    let (mixed, _) = machine(&env, "mixed", &[d, b]).await;
    assert!(open(&env, mixed).await);
    // The network of the dev address classified prod: the differential of the properties
    // brings the machine back, its anomaly closes.
    let environment = env.property("policy.environment").await;
    let dev_network: Uuid =
        sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = '10.2.0.0/24'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    env.applied(vec![Operation::AssignProperty {
        object: Ref::Id(dev_network),
        kind: ObjectKind::Prefix,
        property: Ref::Id(environment),
        values: vec![RawInput::Text("prod".into())],
    }])
    .await;
    assert!(!open(&env, mixed).await);
}

#[kartenn_testing::pg_test]
async fn an_unrelated_changeset_raises_nothing(pool: PgPool) {
    let env = Env::new(&pool).await;
    environments(&env).await;
    let a = classified(&env, "10.1.0.0/24", "prod", "10.1.0.10").await;
    let b = classified(&env, "10.2.0.0/24", "dev", "10.2.0.10").await;
    let (mixed, _) = machine(&env, "mixed", &[a, b]).await;
    assert!(open(&env, mixed).await);
    let unrelated = env
        .applied(vec![Operation::CreateAddress {
            local: None,
            domain: Ref::Id(env.domain),
            address: "10.1.0.20".into(),
            usage: AddressUsage::Host,
        }])
        .await;
    assert!(warnings(&unrelated).is_empty(), "{unrelated:?}");
    assert!(open(&env, mixed).await);
}

#[kartenn_testing::pg_test]
async fn the_first_exempt_role_administered_replaces_the_defaults(pool: PgPool) {
    let env = Env::new(&pool).await;
    environments(&env).await;
    let service = classified(&env, "10.1.0.0/24", "prod", "10.1.0.10").await;
    let management = classified(&env, "10.9.0.0/24", "admin", "10.9.0.10").await;
    let (web, interfaces) = machine(&env, "web", &[service, management]).await;
    assign(
        &env,
        interfaces[1],
        "machines.interface",
        "machines.interface_role",
        "administration",
    )
    .await;
    assert!(!open(&env, web).await);
    // The list administered with `backup` only: administration is no longer exempt.
    env.applied(vec![op("exempt_role.create", json!({ "role": "backup" }))])
        .await;
    assert!(open(&env, web).await);
}
