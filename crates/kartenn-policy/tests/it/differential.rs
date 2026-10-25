//! The contribution of the policy to the differential of a changeset and its approval
//! triggers (spec 010, REQ-010-18 to 20; task 010-9).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::authz::Unrestricted;
use kartenn_service::module::DiffRow;
use kartenn_service::operations::{ModuleTrigger, RawInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ObjectKind, Operation, Outcome, Ref, submit};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::support::env::{Env, op};

const SECTION: &str = "policy.combinations";
const TEXT: &str = "Interdit entre ces zones.";

/// The environments, the flow type `admin` and its three services.
async fn catalogue(env: &Env) {
    env.applied(
        ["prod", "dev", "global"]
            .iter()
            .map(|k| op("environment.create", json!({ "key": k })))
            .chain([op(
                "flow_type.create",
                json!({ "local": 1, "key": "admin", "labels": { "fr": "Administration" } }),
            )])
            .chain([("ssh", 22), ("rdp", 3389), ("winrm", 5985)].map(|(key, port)| {
                op(
                    "service.create",
                    json!({ "key": key, "labels": { "fr": key.to_uppercase() }, "flow_type": { "local": 1 },
                            "entries": [{ "transport": "tcp", "ports": port }] }),
                )
            }))
            .collect(),
    )
    .await;
}

async fn admin(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "SELECT id FROM policy.policy_flow_type WHERE key = 'admin' AND valid_period @> now()",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A rule of the flow type `admin` from `source` to `destination` environments.
fn rule(admin: Uuid, key: &str, effect: &str, source: &str, destination: &str) -> Operation {
    let mut body = json!({
        "key": key, "labels": { "fr": key }, "effect": effect,
        "flow_types": [{ "id": admin }],
        "source": { "policy.environment": [source] },
        "destination": { "policy.environment": [destination] },
    });
    if effect == "deny" {
        body["texts"] = json!({ "fr": { "body": TEXT } });
    }
    op("rule.create", body)
}

/// The rows of the section of the module in a simulation of `operations`.
async fn simulated(env: &Env, operations: Vec<Operation>) -> Vec<DiffRow> {
    let mut request = env.request(operations, None);
    request.simulate = true;
    let outcome = submit(&env.store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    let Outcome::Simulated { sections, .. } = outcome else {
        panic!("not simulated: {outcome:?}");
    };
    sections
        .into_iter()
        .find(|s| s.name == SECTION)
        .map(|s| s.rows)
        .unwrap_or_default()
}

fn summary(rows: &[DiffRow], field: &str) -> u64 {
    rows.iter()
        .find(|r| r.kind == "policy.summary")
        .and_then(|r| r.after.as_ref())
        .and_then(|a| a[field].as_u64())
        .unwrap_or_default()
}

fn flips(rows: &[DiffRow]) -> impl Iterator<Item = (&Value, &Value)> {
    rows.iter()
        .filter(|r| r.kind == "policy.combination")
        .filter_map(|r| Some((r.before.as_ref()?, r.after.as_ref()?)))
}

/// REQ-010-18: an allow from `global` to `dev` opens a combination per admin service,
/// decided by the rule; a deny from `prod` to `dev` closes combinations, the deny cited.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_010_18(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let admin = admin(&pool).await;
    let allow = || rule(admin, "global-dev", "allow", "global", "dev");

    let rows = simulated(&env, vec![allow()]).await;
    assert!(summary(&rows, "opens") >= 3, "{rows:?}");
    assert_eq!(summary(&rows, "closes"), 0);
    for service in ["ssh", "rdp", "winrm"] {
        assert!(
            flips(&rows).any(|(before, after)| after["service"] == service
                && before["verdict"] == "denied"
                && after["verdict"] == "allowed"
                && after["source"]["policy.environment"] == json!(["global"])
                && after["destination"]["policy.environment"] == json!(["dev"])
                && after["decisive"] == json!({ "rule": "global-dev" })),
            "{service}: {rows:?}"
        );
    }

    // Applied: the same section, stored.
    let Outcome::Applied { changeset, .. } = env.applied(vec![allow()]).await else {
        unreachable!()
    };
    let counts = kartenn_store_pg::sections::section_counts(&pool, changeset)
        .await
        .unwrap();
    assert!(
        counts.iter().any(|(name, n)| name == SECTION && *n >= 4),
        "{counts:?}"
    );

    let rows = simulated(
        &env,
        vec![rule(admin, "no-prod-dev", "deny", "prod", "dev")],
    )
    .await;
    assert!(summary(&rows, "closes") >= 1, "{rows:?}");
    assert_eq!(summary(&rows, "opens"), 0);
    assert!(
        flips(&rows)
            .filter(|(_, after)| after["verdict"] == "denied")
            .all(|(_, after)| after["decisive"] == json!({ "rule": "no-prod-dev" }))
    );
}

/// REQ-010-20: a policy on `policy.opens ≥ 1` holds a changeset that opens a combination;
/// one that only adds a flow type is applied.
#[kartenn_testing::pg_test]
async fn a_policy_on_opens_holds_the_changeset(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let admin = admin(&pool).await;
    env.applied(vec![Operation::SetApprovalPolicy {
        name: "opens".into(),
        min_operations: None,
        min_diff_objects: None,
        property_keys: Vec::new(),
        domains: Vec::new(),
        scopes: Vec::new(),
        author_roles: Vec::new(),
        author_groups: Vec::new(),
        bound_roles: Vec::new(),
        module_triggers: vec![ModuleTrigger {
            measure: "policy.opens".into(),
            threshold: 1,
        }],
        enabled: true,
    }])
    .await;
    let quiet = env
        .submit(vec![op(
            "flow_type.create",
            json!({ "key": "files", "labels": { "fr": "Fichiers" } }),
        )])
        .await;
    assert!(matches!(quiet, Outcome::Applied { .. }), "{quiet:?}");
    let held = env
        .submit(vec![rule(admin, "global-dev", "allow", "global", "dev")])
        .await;
    assert!(matches!(held, Outcome::Pending { .. }), "{held:?}");
}

/// REQ-010-19: a network reclassified on an axis counts its heirs whose resolved value
/// changes; the node itself is not among them.
#[kartenn_testing::pg_test]
async fn a_reclassified_node_counts_its_heirs(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let domain = Ref::Id(env.domain);
    let mut operations = vec![Operation::CreatePrefix {
        local: Some(1),
        domain,
        prefix: "10.1.0.0/24".into(),
        nature: Nature::Network,
        point_to_point: false,
        anycast: false,
    }];
    for i in 1..=3 {
        operations.push(Operation::CreateAddress {
            local: Some(1 + i),
            domain,
            address: format!("10.1.0.{i}"),
            usage: AddressUsage::Host,
        });
    }
    env.applied(operations).await;
    let network: Uuid = sqlx::query_scalar(
        "SELECT id FROM prefix WHERE prefix = '10.1.0.0/24'::cidr AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let property = env.property("policy.environment").await;
    let rows = simulated(
        &env,
        vec![Operation::AssignProperty {
            object: Ref::Id(network),
            kind: ObjectKind::Prefix,
            property: Ref::Id(property),
            values: vec![RawInput::Text("prod".into())],
        }],
    )
    .await;
    let reclassified: Vec<&DiffRow> = rows
        .iter()
        .filter(|r| r.kind == "policy.reclassified")
        .collect();
    assert_eq!(reclassified.len(), 1, "{rows:?}");
    assert_eq!(reclassified[0].object, network);
    assert_eq!(
        reclassified[0].after,
        Some(json!({ "axis": "policy.environment", "heirs": 3 }))
    );
    // No write of the policy: no summary of combinations.
    assert!(rows.iter().all(|r| r.kind != "policy.summary"));
}
