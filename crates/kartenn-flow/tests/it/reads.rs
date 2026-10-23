//! Reads of the registry (task 011-7; REQ-011-14, 15, 16, 25): one test per filter, the
//! flows of an object, the evaluations at a past date, and the non-conformities a
//! requester sees.

use kartenn_core::objects::Nature;
use kartenn_flow::model::EndpointKind;
use kartenn_flow::read::{
    AxisValue, Dates, FlowFilter, FlowView, NonconformityFilter, evaluations, flows,
    nonconformities, of_object,
};
use kartenn_flow::state::FlowState;
use kartenn_service::{ObjectKind, Operation, Ref};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::requests::{Env, flow};

async fn now(pool: &PgPool) -> OffsetDateTime {
    sqlx::query_scalar("SELECT now()")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn at(pool: &PgPool) -> Dates {
    Dates {
        valid_at: now(pool).await,
        as_of: None,
    }
}

async fn list(pool: &PgPool, filter: FlowFilter) -> Vec<Uuid> {
    let dates = at(pool).await;
    let mut conn = pool.acquire().await.unwrap();
    flows(&mut conn, dates, &filter, None, 100)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|v| v.id)
        .collect()
}

async fn one(pool: &PgPool, sql: &str) -> Uuid {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn flow_between(pool: &PgPool, source: Uuid, destination: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM flow.flow WHERE source_id = $1 AND destination_id = $2")
        .bind(source)
        .bind(destination)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn machines(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// Two flows `prod → dev` of `acme`: `first` from 10.1.0.1 to 10.2.0.1, `second` from
/// 10.1.0.2 to 10.3.0.1, the second with the external reference `CHG-2`.
struct Two {
    env: Env,
    first: Uuid,
    second: Uuid,
    source: Uuid,
}

async fn two(pool: &PgPool) -> Two {
    let env = Env::new(pool).await;
    let source = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let to_first = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    env.applied(vec![Operation::CreateAddress {
        local: None,
        domain: Ref::Id(env.domain),
        address: "10.1.0.2".into(),
        usage: kartenn_core::objects::AddressUsage::Host,
    }])
    .await;
    let other = one(pool, "SELECT id FROM address WHERE address = '10.1.0.2'").await;
    let to_second = env.address("10.3.0.0/24", "10.3.0.1", "dev").await;
    env.applied(vec![flow("request", env.fields(source, to_first).await)])
        .await;
    let mut fields = env.fields(other, to_second).await;
    fields["external_refs"] = json!([{ "system": "ticketing", "id": "CHG-2" }]);
    env.applied(vec![flow("request", fields)]).await;
    let first = flow_between(pool, source, to_first).await;
    let second = flow_between(pool, other, to_second).await;
    Two {
        env,
        first,
        second,
        source,
    }
}

#[kartenn_testing::pg_test]
async fn filter_by_endpoint_service_type_kind_entity_requester_ref(pool: PgPool) {
    let t = two(&pool).await;
    let all = list(&pool, FlowFilter::default()).await;
    assert_eq!(all.len(), 2);
    let by_endpoint = FlowFilter {
        endpoint: Some((EndpointKind::Address, t.source)),
        ..FlowFilter::default()
    };
    assert_eq!(list(&pool, by_endpoint).await, [t.first]);
    let ssh = one(
        &pool,
        "SELECT id FROM policy.policy_service WHERE key = 'ssh'",
    )
    .await;
    let admin = one(
        &pool,
        "SELECT id FROM policy.policy_flow_type WHERE key = 'admin'",
    )
    .await;
    for filter in [
        FlowFilter {
            service: Some(ssh),
            ..FlowFilter::default()
        },
        FlowFilter {
            flow_type: Some(admin),
            ..FlowFilter::default()
        },
        FlowFilter {
            kind: Some("requested".into()),
            ..FlowFilter::default()
        },
        FlowFilter {
            requester: Some(t.env.author),
            ..FlowFilter::default()
        },
    ] {
        assert_eq!(list(&pool, filter).await.len(), 2);
    }
    let acme = one(&pool, "SELECT id FROM entity WHERE slug = 'acme'").await;
    let by_entity = FlowFilter {
        entity: Some(acme),
        ..FlowFilter::default()
    };
    assert_eq!(list(&pool, by_entity).await.len(), 2);
    let elsewhere = FlowFilter {
        entity: Some(Uuid::max()),
        ..FlowFilter::default()
    };
    assert!(list(&pool, elsewhere).await.is_empty());
    let by_ref = FlowFilter {
        external_ref: Some((
            one(
                &pool,
                "SELECT id FROM property_enum_value WHERE key = 'ticketing'",
            )
            .await,
            "CHG-2".into(),
        )),
        ..FlowFilter::default()
    };
    assert_eq!(list(&pool, by_ref).await, [t.second]);
}

#[kartenn_testing::pg_test]
async fn filter_by_state_summary_outcome_decisive_waiver_and_end(pool: PgPool) {
    let t = two(&pool).await;
    for filter in [
        FlowFilter {
            states: vec![FlowState::Active],
            ..FlowFilter::default()
        },
        FlowFilter {
            summary: Some("allowed".into()),
            ..FlowFilter::default()
        },
        FlowFilter {
            outcome: Some("automatic".into()),
            ..FlowFilter::default()
        },
        FlowFilter {
            waived: Some(false),
            ..FlowFilter::default()
        },
    ] {
        assert_eq!(list(&pool, filter).await.len(), 2);
    }
    let noncompliant = FlowFilter {
        states: vec![FlowState::Noncompliant],
        ..FlowFilter::default()
    };
    assert!(list(&pool, noncompliant).await.is_empty());
    let rule = one(
        &pool,
        "SELECT id FROM policy.policy_rule WHERE key = 'prod-dev'",
    )
    .await;
    let by_rule = FlowFilter {
        decisive: Some(rule),
        ..FlowFilter::default()
    };
    assert_eq!(list(&pool, by_rule).await.len(), 2);
    // Flows without end never end (REQ-011-24).
    let ending = FlowFilter {
        ending_before: Some(now(&pool).await + time::Duration::days(365)),
        ..FlowFilter::default()
    };
    assert!(list(&pool, ending).await.is_empty());
    let _ = t;
}

/// REQ-011-14, 15: the subtree of a prefix, the value of an axis, and a machine through an
/// address attached to its interface.
#[kartenn_testing::pg_test]
async fn filter_by_subtree_axis_and_machine(pool: PgPool) {
    let t = two(&pool).await;
    t.env
        .applied(vec![Operation::CreatePrefix {
            local: None,
            domain: Ref::Id(t.env.domain),
            prefix: "10.0.0.0/8".into(),
            nature: Nature::Container,
            point_to_point: false,
            anycast: false,
        }])
        .await;
    let container = one(&pool, "SELECT id FROM prefix WHERE prefix = '10.0.0.0/8'").await;
    let network = one(&pool, "SELECT id FROM prefix WHERE prefix = '10.2.0.0/24'").await;
    assert_eq!(
        list(
            &pool,
            FlowFilter {
                within_prefix: Some(container),
                ..FlowFilter::default()
            }
        )
        .await
        .len(),
        2
    );
    let dates = at(&pool).await;
    let mut conn = pool.acquire().await.unwrap();
    let of_network: Vec<Uuid> =
        of_object(&mut conn, dates, EndpointKind::Prefix, network, None, 10)
            .await
            .unwrap()
            .into_iter()
            .filter_map(|v: FlowView| v.id)
            .collect();
    assert_eq!(of_network, [t.first]);
    drop(conn);
    let prod = FlowFilter {
        axis: Some(AxisValue {
            property: "policy.environment".into(),
            value: "prod".into(),
        }),
        ..FlowFilter::default()
    };
    assert_eq!(list(&pool, prod).await.len(), 2);
    let elsewhere = FlowFilter {
        axis: Some(AxisValue {
            property: "policy.environment".into(),
            value: "rec".into(),
        }),
        ..FlowFilter::default()
    };
    assert!(list(&pool, elsewhere).await.is_empty());
    let acme = one(&pool, "SELECT id FROM entity WHERE slug = 'acme'").await;
    t.env
        .applied(vec![machines(
            "create_machine",
            json!({ "local": 7, "name": "web1", "kind": "physical", "owner": { "id": acme } }),
        )])
        .await;
    let machine = one(&pool, "SELECT id FROM machines.machine WHERE name = 'web1'").await;
    t.env
        .applied(vec![machines(
            "create_interface",
            json!({ "machine": { "id": machine }, "name": "eth0" }),
        )])
        .await;
    let interface = one(
        &pool,
        "SELECT id FROM machines.interface WHERE name = 'eth0'",
    )
    .await;
    t.env
        .applied(vec![machines(
            "attach_address",
            json!({ "interface": { "id": interface }, "address": { "id": t.source },
                    "primary": true }),
        )])
        .await;
    let by_machine = FlowFilter {
        machine: Some(machine),
        ..FlowFilter::default()
    };
    assert_eq!(list(&pool, by_machine).await, [t.first]);
}

/// REQ-011-16: the evaluations of a flow at a past assertion are those stored then, not
/// recomputed: before the rule closed they are allowed, after it denied.
#[kartenn_testing::pg_test]
async fn evaluations_at_a_past_assertion(pool: PgPool) {
    let t = two(&pool).await;
    let before = now(&pool).await;
    let rule = one(
        &pool,
        "SELECT id FROM policy.policy_rule WHERE key = 'prod-dev'",
    )
    .await;
    t.env
        .applied(vec![Operation::Close {
            kind: ObjectKind::from_name("policy.rule").unwrap(),
            id: Ref::Id(rule),
            expected: None,
            cascade: false,
        }])
        .await;
    let later = now(&pool).await;
    let mut conn = pool.acquire().await.unwrap();
    let past = evaluations(
        &mut conn,
        Dates {
            valid_at: later,
            as_of: Some(before),
        },
        t.first,
    )
    .await
    .unwrap();
    assert_eq!(past.len(), 1);
    assert_eq!(past[0].verdict, "allowed");
    let current = evaluations(
        &mut conn,
        Dates {
            valid_at: later,
            as_of: None,
        },
        t.first,
    )
    .await
    .unwrap();
    assert_eq!(current[0].verdict, "denied");
}

/// REQ-011-25: a broken flow is a non-conformity of its requester, with the rule in cause
/// when there is one; another requester does not see it.
#[kartenn_testing::pg_test]
async fn a_requester_sees_their_nonconformities_only(pool: PgPool) {
    let t = two(&pool).await;
    let rule = one(
        &pool,
        "SELECT id FROM policy.policy_rule WHERE key = 'prod-dev'",
    )
    .await;
    t.env
        .applied(vec![Operation::Close {
            kind: ObjectKind::from_name("policy.rule").unwrap(),
            id: Ref::Id(rule),
            expected: None,
            cascade: false,
        }])
        .await;
    let dates = at(&pool).await;
    let mut conn = pool.acquire().await.unwrap();
    let mine = nonconformities(
        &mut conn,
        dates,
        &NonconformityFilter {
            requester: Some(t.env.author),
            ..NonconformityFilter::default()
        },
        100,
    )
    .await
    .unwrap();
    assert_eq!(mine.len(), 2);
    assert!(mine.iter().all(|n| n.state == "noncompliant"));
    let theirs = nonconformities(
        &mut conn,
        dates,
        &NonconformityFilter {
            requester: Some(Uuid::max()),
            ..NonconformityFilter::default()
        },
        100,
    )
    .await
    .unwrap();
    assert!(theirs.is_empty());
    let noncompliant = FlowFilter {
        states: vec![FlowState::Noncompliant],
        ..FlowFilter::default()
    };
    drop(conn);
    assert_eq!(list(&pool, noncompliant).await.len(), 2);
}
