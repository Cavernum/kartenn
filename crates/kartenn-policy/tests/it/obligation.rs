//! An axis made required on a domain (spec 010, REQ-010-21; task 010-10): the obligation
//! of an administrator of the core (REQ-003-6, 16; 008-20) on the property of the axis.

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::operations::{ObligationInput, RawInput};
use kartenn_service::{ObjectKind, Operation, Outcome, Ref};
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::support::env::{Env, op};

fn required(property: Uuid, domain: Uuid) -> Operation {
    Operation::AddObligation {
        property: Ref::Id(property),
        scope: ObligationInput::Domain { id: domain },
    }
}

fn assign(property: Uuid, object: Ref, kind: ObjectKind, value: &str) -> Operation {
    Operation::AssignProperty {
        object,
        kind,
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.into())],
    }
}

/// The objects a refusal names.
fn subjects(outcome: &Outcome) -> Vec<Uuid> {
    match outcome {
        Outcome::Refused { violations, .. } => violations
            .iter()
            .flat_map(|v| v.subjects.iter().copied())
            .collect(),
        other => panic!("not refused: {other:?}"),
    }
}

/// REQ-010-21: domain D with 12 addresses without environment: making
/// `policy.environment` required is refused with the 12 addresses; once they are
/// classified in the same changeset, accepted.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_010_21_required_environment(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.applied(vec![op("environment.create", json!({ "key": "prod" }))])
        .await;
    let domain = env.domain;
    let property = env.property("policy.environment").await;
    env.applied(
        std::iter::once(Operation::CreatePrefix {
            local: Some(100),
            domain: Ref::Id(domain),
            prefix: "10.9.0.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        })
        .chain((1..=12).map(|i| Operation::CreateAddress {
            local: Some(i),
            domain: Ref::Id(domain),
            address: format!("10.9.0.{i}"),
            usage: AddressUsage::Host,
        }))
        .collect(),
    )
    .await;
    let addresses: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM address WHERE domain_id = $1 AND valid_period @> now()")
            .bind(domain)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(addresses.len(), 12);

    let refused = env.submit(vec![required(property, domain)]).await;
    let named = subjects(&refused);
    assert!(
        addresses.iter().all(|a| named.contains(a)),
        "the 12 addresses: {refused:?}"
    );

    // Classified in the same changeset: accepted.
    let mut operations: Vec<Operation> = addresses
        .iter()
        .map(|a| assign(property, Ref::Id(*a), ObjectKind::Address, "prod"))
        .collect();
    operations.push(assign(
        property,
        Ref::Id(domain),
        ObjectKind::Domain,
        "prod",
    ));
    operations.push(required(property, domain));
    env.applied(operations).await;
}

/// REQ-010-21: D required on `policy.exposure` alone: a network without a resolved
/// exposure is refused, one without environment accepted.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_010_21_required_exposure(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.applied(vec![op("examples.install", json!({}))]).await;
    let domain = env.domain;
    let exposure = env.property("policy.exposure").await;
    env.applied(vec![
        assign(exposure, Ref::Id(domain), ObjectKind::Domain, "interne"),
        required(exposure, domain),
    ])
    .await;
    let network = |prefix: &str| Operation::CreatePrefix {
        local: Some(1),
        domain: Ref::Id(domain),
        prefix: prefix.into(),
        nature: Nature::Network,
        point_to_point: false,
        anycast: false,
    };
    // Inherited from the domain: resolved, accepted without any environment.
    env.applied(vec![network("10.2.0.0/24")]).await;
    // A network that stops the inheritance of its exposure (« no value »): refused.
    let refused = env
        .submit(vec![
            network("10.3.0.0/24"),
            Operation::ClearProperty {
                object: Ref::Local(1),
                kind: ObjectKind::Prefix,
                property: Ref::Id(exposure),
            },
        ])
        .await;
    assert!(matches!(refused, Outcome::Refused { .. }), "{refused:?}");
}
