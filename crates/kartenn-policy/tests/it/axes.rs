//! Environments, axes and the attributes of their values (spec 010, REQ-010-1 to 3, 26, 29,
//! 30; task 010-7), through the write pipeline.

use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
use kartenn_service::operations::RawInput;
use kartenn_service::{ObjectKind, Operation, Outcome, Ref};
use kartenn_store_pg::explain::explain;
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::support::env::{Env, messages, op, rules};

/// The environments `keys`, created with their attributes in one changeset.
async fn environments(env: &Env, keys: &[&str]) {
    env.applied(
        keys.iter()
            .map(|k| {
                op(
                    "environment.create",
                    json!({ "key": k, "label": k.to_uppercase() }),
                )
            })
            .collect(),
    )
    .await;
}

/// The assignment of the environments `values` to `object`.
fn assign(env_property: Uuid, object: Ref, kind: ObjectKind, values: &[&str]) -> Operation {
    Operation::AssignProperty {
        object,
        kind,
        property: Ref::Id(env_property),
        values: values.iter().map(|v| RawInput::Text((*v).into())).collect(),
    }
}

async fn row(pool: &PgPool, sql: &str, key: &str) -> Uuid {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The current instant of the database, in microseconds.
async fn now_micros(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT (extract(epoch FROM now()) * 1e6)::bigint")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// REQ-010-1: a network assigned `{rec}` replaces the `{prod, global}` of its container,
/// and its address resolves `{rec}`; an environment on a range is refused (REQ-003-4).
#[kartenn_testing::pg_test]
async fn acceptance_of_req_010_1(pool: PgPool) {
    let env = Env::new(&pool).await;
    environments(&env, &["prod", "global", "rec"]).await;
    let property = env.property("policy.environment").await;
    let domain = Ref::Id(env.domain);
    env.applied(vec![
        Operation::CreatePrefix {
            local: Some(1),
            domain,
            prefix: "10.0.0.0/8".into(),
            nature: Nature::Container,
            point_to_point: false,
            anycast: false,
        },
        Operation::CreatePrefix {
            local: Some(2),
            domain,
            prefix: "10.1.0.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
        Operation::CreateAddress {
            local: Some(3),
            domain,
            address: "10.1.0.5".into(),
            usage: AddressUsage::Host,
        },
        assign(
            property,
            Ref::Local(1),
            ObjectKind::Prefix,
            &["prod", "global"],
        ),
        assign(property, Ref::Local(2), ObjectKind::Prefix, &["rec"]),
    ])
    .await;
    let address = row(
        &pool,
        "SELECT id FROM address WHERE host(address) = $1",
        "10.1.0.5",
    )
    .await;
    let container = row(
        &pool,
        "SELECT id FROM prefix WHERE prefix = $1::cidr",
        "10.0.0.0/8",
    )
    .await;
    let explained = explain(&pool, address, property, OffsetDateTime::now_utc(), None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(explained.value.as_deref(), Some(r#"["rec"]"#));
    let (replaced, before) = explained.replaces.expect("the inherited value replaced");
    assert_eq!(replaced, container);
    assert_eq!(before.as_deref(), Some(r#"["global", "prod"]"#));

    let range = env
        .submit(vec![
            Operation::CreateRange {
                local: Some(4),
                domain,
                first: "10.1.0.100".into(),
                last: "10.1.0.120".into(),
                usage: RangeUsage::Dhcp,
            },
            assign(property, Ref::Local(4), ObjectKind::Range, &["rec"]),
        ])
        .await;
    assert!(matches!(range, Outcome::Refused { .. }), "{range:?}");
}

/// REQ-010-2: an environment created on a date is not offered before it; closed while a
/// node cites it, refused; closed when nothing cites it, its attributes go with it.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_010_2(pool: PgPool) {
    let env = Env::new(&pool).await;
    let day = 86_400_000_000_i64;
    let now = now_micros(&pool).await;
    env.submit_from(
        vec![op(
            "environment.create",
            json!({ "key": "preprod", "label": "Préproduction" }),
        )],
        Some(now + 30 * day),
    )
    .await;
    let property = env.property("policy.environment").await;
    let early = env
        .submit_from(
            vec![assign(
                property,
                Ref::Id(env.domain),
                ObjectKind::Domain,
                &["preprod"],
            )],
            Some(now + 15 * day),
        )
        .await;
    assert!(matches!(early, Outcome::Refused { .. }), "{early:?}");

    environments(&env, &["sandbox", "spare"]).await;
    env.applied(vec![assign(
        property,
        Ref::Id(env.domain),
        ObjectKind::Domain,
        &["sandbox"],
    )])
    .await;
    let cited = env
        .submit(vec![op("environment.close", json!({ "key": "sandbox" }))])
        .await;
    assert!(matches!(cited, Outcome::Refused { .. }), "{cited:?}");

    env.applied(vec![op("environment.close", json!({ "key": "spare" }))])
        .await;
    let living: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM policy.policy_value
          WHERE value_key = 'spare' AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(living, 0, "the attributes are closed with the value");
}

/// REQ-010-29: the examples designate their axes on request; `policy.environment` is an
/// axis already; a property of another type, an unknown one, or one designated twice is
/// refused.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_010_29(pool: PgPool) {
    let env = Env::new(&pool).await;
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM policy.policy_axis")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, 0, "the examples are not imposed");
    env.applied(vec![op("examples.install", json!({}))]).await;
    let axes: Vec<(String, String)> = sqlx::query_as(
        "SELECT property_key, carrier FROM policy.policy_axis
          WHERE valid_period @> now() ORDER BY property_key",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        axes,
        [
            ("policy.application", "machine"),
            ("policy.conduit_role", "machine"),
            ("policy.criticality", "machine"),
            ("policy.exposure", "tree"),
            ("policy.owner", "machine"),
            ("policy.sensitivity", "machine"),
        ]
        .map(|(k, c)| (k.to_owned(), c.to_owned()))
    );

    let imposed = env
        .submit(vec![op(
            "axis.designate",
            json!({ "property": "policy.environment", "carrier": "tree" }),
        )])
        .await;
    assert_eq!(rules(&imposed), ["policy.axis.invalid"]);
    for (property, why) in [
        ("machines.fqdn", "neither a closed list"),
        ("custom.nothing", "not declared"),
        ("policy.exposure", "taken"),
    ] {
        let refused = env
            .submit(vec![op(
                "axis.designate",
                json!({ "property": property, "carrier": "machine" }),
            )])
            .await;
        assert_eq!(rules(&refused), ["policy.axis.invalid"], "{property}");
        assert!(
            messages(&refused).contains(why),
            "{property}: {}",
            messages(&refused)
        );
    }
}

/// REQ-010-30: a value of an axis is described, transverse and ranked, then modified; a
/// value that no axis has is refused.
#[kartenn_testing::pg_test]
async fn a_value_is_described_then_modified(pool: PgPool) {
    let env = Env::new(&pool).await;
    environments(&env, &["global"]).await;
    env.applied(vec![op(
        "value.describe",
        json!({ "property": "policy.environment", "value": "global",
                "transverse": true, "rank": 3, "description": "Partagé." }),
    )])
    .await;
    let described: (bool, Option<i32>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT transverse, rank, label, description FROM policy.policy_value
          WHERE value_key = 'global' AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        described,
        (
            true,
            Some(3),
            Some("GLOBAL".into()),
            Some("Partagé.".into())
        )
    );
    env.applied(vec![op("examples.install", json!({}))]).await;
    env.applied(vec![op(
        "value.describe",
        json!({ "property": "policy.criticality", "value": "vitale", "rank": 2 }),
    )])
    .await;
    for (property, value) in [
        ("policy.environment", "nowhere"),
        ("policy.personal_data", "true"),
    ] {
        let refused = env
            .submit(vec![op(
                "value.describe",
                json!({ "property": property, "value": value }),
            )])
            .await;
        assert_eq!(rules(&refused), ["policy.value.unknown"], "{property}");
    }
}

/// REQ-010-26: each changeset that writes the policy raises its revision once, however
/// many objects it writes; a changeset that only reads leaves it.
#[kartenn_testing::pg_test]
async fn the_revision_rises_once_per_changeset(pool: PgPool) {
    let env = Env::new(&pool).await;
    let start = env.revision().await;
    environments(&env, &["a", "b", "c"]).await;
    assert_eq!(env.revision().await, start + 1);
    env.applied(vec![op("examples.install", json!({}))]).await;
    assert_eq!(env.revision().await, start + 2);
    let property = env.property("policy.environment").await;
    env.applied(vec![assign(
        property,
        Ref::Id(env.domain),
        ObjectKind::Domain,
        &["a"],
    )])
    .await;
    assert_eq!(
        env.revision().await,
        start + 2,
        "an assignment is not the policy"
    );
}

/// The constraints of the tables are the last line of defence (design « Données »): an
/// axis on an unknown carrier, a rule without flow selector or a conditional deny are
/// refused by the database itself.
#[kartenn_testing::pg_test]
async fn the_tables_keep_their_constraints(pool: PgPool) {
    Env::new(&pool).await;
    for sql in [
        "INSERT INTO policy.policy_axis (id, property_key, carrier, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'policy.exposure', 'cloud', tstzrange(now(), NULL), gen_random_uuid())",
        "INSERT INTO policy.policy_rule (id, key, label, effect, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'r', 'R', 'allow', tstzrange(now(), NULL), gen_random_uuid())",
        "INSERT INTO policy.policy_rule (id, key, label, effect, flow_type_ids, required_pattern_ids,
                                         valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'r', 'R', 'deny', ARRAY[gen_random_uuid()],
                 ARRAY[gen_random_uuid()], tstzrange(now(), NULL), gen_random_uuid())",
    ] {
        assert!(sqlx::query(sql).execute(&pool).await.is_err(), "{sql}");
    }
}

/// REQ-010-3: a machine declares environments among the values of `policy.environment`
/// (the shared list, N7), never inherited; a value outside the list is refused.
#[kartenn_testing::pg_test]
async fn a_machine_declares_environments_of_the_shared_list(pool: PgPool) {
    let env = Env::new(&pool).await;
    environments(&env, &["prod", "rec"]).await;
    let acme = env.id("entity", "acme").await;
    let create: Operation = serde_json::from_value(json!({
        "op": "machines.create_machine",
        "local": 1,
        "name": "srv01",
        "kind": "physical",
        "owner": { "id": acme },
    }))
    .unwrap();
    let declared = env.property("policy.declared_environment").await;
    let kind = ObjectKind::from_name("machines.machine").unwrap();
    env.applied(vec![
        create,
        assign(declared, Ref::Local(1), kind, &["prod"]),
    ])
    .await;
    let machine: Uuid = sqlx::query_scalar("SELECT id FROM machines.machine WHERE name = 'srv01'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let outside = env
        .submit(vec![assign(declared, Ref::Id(machine), kind, &["staging"])])
        .await;
    assert!(matches!(outside, Outcome::Refused { .. }), "{outside:?}");
}
