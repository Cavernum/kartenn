//! The inference as a source of expected values (spec 010, REQ-010-40, 41, 45, 46, 50 to
//! 52; task 010-15): the rules written by changeset, the single expectation written as a
//! default value with its provenance, several candidates suggested but never written, and
//! the checks of the rules on the final state.

use std::sync::Arc;

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_policy::infer::InferenceSource;
use kartenn_service::authz::Unrestricted;
use kartenn_service::expected::{Expected, Sources, suggest};
use kartenn_service::pipeline::{MAX_OPERATIONS, submit_with};
use kartenn_service::{Operation, Outcome, Ref};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::support::env::{Env, op, rules};

fn sources() -> Sources {
    Sources::none().with(Arc::new(InferenceSource))
}

/// `operations` applied with the inference as a source of expected values.
async fn applied(env: &Env, operations: Vec<Operation>) -> Outcome {
    let outcome = submit_with(
        &env.store,
        &Unrestricted,
        &sources(),
        &env.request(operations, None),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    outcome
}

/// The environments `prod` and `rec`.
async fn environments(env: &Env) {
    env.applied(vec![
        op("environment.create", json!({ "key": "prod" })),
        op("environment.create", json!({ "key": "rec" })),
    ])
    .await;
}

fn rule(key: &str, value: &str, priority: i64, criteria: &Value) -> Operation {
    op(
        "inference_rule.create",
        json!({ "key": key, "axis": "policy.environment", "priority": priority,
                "criteria": criteria, "values": [value] }),
    )
}

fn container(env: &Env, prefix: &str) -> Operation {
    Operation::CreatePrefix {
        local: Some(1),
        domain: Ref::Id(env.domain),
        prefix: prefix.into(),
        nature: Nature::Container,
        point_to_point: false,
        anycast: false,
    }
}

async fn prefix_id(pool: &PgPool, prefix: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = $1::cidr AND valid_period @> now()")
        .bind(prefix)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The own assignment of `policy.environment` on `object`: its values and provenance.
async fn own(pool: &PgPool, object: Uuid) -> Option<(Vec<String>, String)> {
    sqlx::query_as(
        "SELECT array_agg(e.key ORDER BY e.key), a.provenance
           FROM property_assignment a
           JOIN property_declaration d ON d.id = a.property_id AND d.key = 'policy.environment'
           JOIN property_enum_value e ON e.id = a.v_uuid OR e.id = ANY(a.v_uuids)
          WHERE a.object_id = $1 AND a.valid_period @> now() AND a.kind = 'value'
          GROUP BY a.provenance",
    )
    .bind(object)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// REQ-010-52 and 50: a node created without its own value and that inherits none
/// receives the single expectation, with the provenance
/// `source:policy.inference@<revision>#<rule>`; a node created below it inherits that
/// value and receives no default of its own.
#[kartenn_testing::pg_test]
async fn a_single_expectation_is_written_as_a_default_with_its_provenance(pool: PgPool) {
    let env = Env::new(&pool).await;
    environments(&env).await;
    applied(
        &env,
        vec![rule(
            "r-prod",
            "prod",
            0,
            &json!([{ "within": ["10.0.0.0/8"] }]),
        )],
    )
    .await;
    applied(&env, vec![container(&env, "10.0.0.0/8")]).await;
    let top = prefix_id(&pool, "10.0.0.0/8").await;
    let (values, provenance) = own(&pool, top).await.expect("a default value");
    assert_eq!(values, ["prod"]);
    assert_eq!(provenance, "source:policy.inference@1#r-prod");
    // Below: a network and an address inherit the value, no default of their own.
    applied(
        &env,
        vec![
            Operation::CreatePrefix {
                local: Some(1),
                domain: Ref::Id(env.domain),
                prefix: "10.0.0.0/24".into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
            Operation::CreateAddress {
                local: Some(2),
                domain: Ref::Id(env.domain),
                address: "10.0.0.9".into(),
                usage: AddressUsage::Host,
            },
        ],
    )
    .await;
    let address: Uuid = sqlx::query_scalar(
        "SELECT id FROM address WHERE address = '10.0.0.9' AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        own(&pool, prefix_id(&pool, "10.0.0.0/24").await).await,
        None
    );
    assert_eq!(own(&pool, address).await, None);
    // The provenance read in the explanation of the address (REQ-003-14, REQ-010-50).
    let property = env.property("policy.environment").await;
    let explained = kartenn_store_pg::explain::explain(
        &pool,
        address,
        property,
        sqlx::types::time::OffsetDateTime::now_utc(),
        None,
    )
    .await
    .unwrap()
    .expect("an explanation");
    assert_eq!(explained.origin, Some(top));
    assert_eq!(
        explained.provenance.as_deref(),
        Some("source:policy.inference@1#r-prod")
    );
}

/// Acceptance of REQ-010-45: two rules of priority 10 on the same container, `{prod}` and
/// `{rec}`: the suggestion proposes both; the creation writes no default.
#[kartenn_testing::pg_test]
async fn a_tie_gives_every_candidate_and_no_default(pool: PgPool) {
    let env = Env::new(&pool).await;
    environments(&env).await;
    let within = json!([{ "within": ["10.0.0.0/8"] }]);
    applied(
        &env,
        vec![
            rule("r-prod", "prod", 10, &within),
            rule("r-rec", "rec", 10, &within),
        ],
    )
    .await;
    applied(&env, vec![container(&env, "10.0.0.0/8")]).await;
    let top = prefix_id(&pool, "10.0.0.0/8").await;
    assert_eq!(own(&pool, top).await, None);
    let property = env.property("policy.environment").await;
    let suggested = suggest(&env.store, &sources(), property, top, None)
        .await
        .unwrap();
    let Expected::Candidates(candidates) = suggested else {
        panic!("several candidates: {suggested:?}")
    };
    let mut details: Vec<_> = candidates.iter().filter_map(|c| c.detail.clone()).collect();
    details.sort();
    assert_eq!(details, ["r-prod", "r-rec"]);
}

/// The checks of a rule of inference on the final state (REQ-010-40, 42): an axis not
/// designated for the inference, a value that does not live, an expression that is not
/// anchored.
#[kartenn_testing::pg_test]
async fn a_rule_of_inference_is_checked_on_the_final_state(pool: PgPool) {
    let env = Env::new(&pool).await;
    environments(&env).await;
    for (operation, expected) in [
        (
            op(
                "inference_rule.create",
                json!({ "key": "r-x", "axis": "policy.exposure", "values": ["interne"] }),
            ),
            "policy.axis.invalid",
        ),
        (
            rule("r-y", "nowhere", 0, &json!([])),
            "policy.value.unknown",
        ),
        (
            rule("r-z", "prod", 0, &json!([{ "name": ["prd-"] }])),
            "policy.inference.regex_invalid",
        ),
    ] {
        let outcome = env.submit(vec![operation]).await;
        assert!(rules(&outcome).contains(&expected), "{outcome:?}");
    }
}

/// The owner of a node and its ancestors, inherited or its own (REQ-010-41, 54; the
/// reads of the chains reshaped for the preview, S36): a rule on
/// the parent entity reaches the nodes of the domain it owns, a stronger rule on a child
/// entity the subtree that names it, and a node without any rule is left alone.
#[kartenn_testing::pg_test]
async fn the_owners_of_many_nodes_are_read_with_their_lineage(pool: PgPool) {
    let env = Env::new(&pool).await;
    environments(&env).await;
    let acme = env.id("entity", "acme").await;
    env.applied(vec![Operation::CreateEntity {
        local: Some(1),
        slug: "beta".into(),
        parent: Some(Ref::Id(acme)),
    }])
    .await;
    let beta = env.id("entity", "beta").await;
    applied(
        &env,
        vec![
            rule("r-acme", "prod", 0, &json!([{ "entity": [acme] }])),
            rule(
                "r-beta",
                "rec",
                1,
                &json!([{ "entity": [beta] }, { "kind": ["container"] }]),
            ),
        ],
    )
    .await;
    let owner = env.property("core.owner").await;
    env.applied(vec![
        container(&env, "10.0.0.0/8"),
        Operation::CreatePrefix {
            local: Some(2),
            domain: Ref::Id(env.domain),
            prefix: "10.1.0.0/16".into(),
            nature: Nature::Container,
            point_to_point: false,
            anycast: false,
        },
        Operation::AssignProperty {
            object: Ref::Local(2),
            kind: kartenn_service::ObjectKind::Prefix,
            property: Ref::Id(owner),
            values: vec![kartenn_service::operations::RawInput::Text(
                beta.to_string(),
            )],
        },
        Operation::CreatePrefix {
            local: Some(3),
            domain: Ref::Id(env.domain),
            prefix: "10.1.2.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
    ])
    .await;
    let property = env.property("policy.environment").await;
    let expected_of = async |prefix: &str| {
        let node = prefix_id(&pool, prefix).await;
        let suggested = suggest(&env.store, &sources(), property, node, None)
            .await
            .unwrap();
        suggested.single().and_then(|s| s.detail.clone())
    };
    assert_eq!(expected_of("10.0.0.0/8").await.as_deref(), Some("r-acme"));
    assert_eq!(expected_of("10.1.0.0/16").await.as_deref(), Some("r-beta"));
    // The network below inherits beta: r-beta reached through its container.
    assert_eq!(expected_of("10.1.2.0/24").await.as_deref(), Some("r-beta"));
}
