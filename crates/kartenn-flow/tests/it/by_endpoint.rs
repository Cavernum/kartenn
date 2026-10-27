//! The flows read through one of their endpoints (spec 027, REQ-027-1 to 4): a holder of
//! `flow.read` on any perimeter reads the flows of an endpoint it reads, the other shown by
//! value; writes and filters by object keep to the objects it reads.

use axum::http::{Method, StatusCode};
use kartenn_service::{ObjectKind, Operation, Ref, operations::RawInput};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::requests::{Env, flow, later, policy};
use crate::rest::{call, checked, ids, opening, patch};

fn machines(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// The world of the cases of the spec: Alice reads the machine `srv-app-01` (interface
/// `eth0`, `192.0.2.10`, network `192.0.2.0/24`), not the network `198.51.100.0/24` nor
/// the machine `srv-db-01` (interface `eth1`, `198.51.100.20` and `2001:db8::20`); the
/// flows are those of `acme`, an entity she holds no `flow.read` on.
struct World {
    env: Env,
    eth0: Uuid,
    eth1: Uuid,
    network: Uuid,
    /// `eth0` → `198.51.100.0/24`.
    outgoing: Uuid,
    /// `198.51.100.0/24` → `eth0`.
    incoming: Uuid,
    /// `eth0` → `eth1`.
    to_eth1: Uuid,
    /// `flow.read`, `flow.edit` on `other`; the reads of `srv-app-01`.
    alice: Uuid,
}

async fn id_of(pool: &PgPool, sql: &str, key: &str) -> Uuid {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// An entity of slug `slug`.
async fn entity(env: &Env, slug: &str) -> Uuid {
    env.applied(vec![Operation::CreateEntity {
        local: None,
        slug: slug.into(),
        parent: None,
    }])
    .await;
    id_of(
        &env.pool,
        "SELECT id FROM entity WHERE slug = $1 AND valid_period @> now()",
        slug,
    )
    .await
}

/// The machine `name` owned by `owner`, its interface `interface` holding `addresses`:
/// the interface.
async fn machine(env: &Env, name: &str, owner: Uuid, interface: &str, addresses: &[Uuid]) -> Uuid {
    let mut operations = vec![
        machines(
            "create_machine",
            json!({ "local": 1, "name": name, "kind": "physical", "owner": { "id": owner } }),
        ),
        machines(
            "create_interface",
            json!({ "local": 2, "machine": { "local": 1 }, "name": interface }),
        ),
    ];
    for address in addresses {
        operations.push(machines(
            "attach_address",
            json!({ "interface": { "local": 2 }, "address": { "id": address } }),
        ));
    }
    env.applied(operations).await;
    sqlx::query_scalar(
        "SELECT i.id FROM machines.machine m
           JOIN machines.interface i ON i.machine_id = m.id AND i.valid_period @> now()
          WHERE m.name = $1 AND m.valid_period @> now()",
    )
    .bind(name)
    .fetch_one(&env.pool)
    .await
    .unwrap()
}

/// Grants `entries` to `identity` on the scope `kind` `id` (global when `None`).
async fn grant(env: &Env, identity: Uuid, entries: &[&str], kind: &str, id: Option<Uuid>) {
    let pool = &env.pool;
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, $2, jsonb_build_object('fr', 'r'), tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .bind(format!("r_{}", role.simple()))
    .execute(pool)
    .await
    .unwrap();
    for entry in entries {
        sqlx::query(
            "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
             VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(role)
        .bind(entry)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(kartenn_testing::BIND_IDENTITY)
        .bind(identity)
        .bind(role)
        .bind(kind)
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

/// A flow `ssh` of `acme` from `source` to `destination` (`(kind, id)`), applied.
async fn flow_between(env: &Env, source: (&str, Uuid), destination: (&str, Uuid)) -> Uuid {
    let mut fields = env.fields(Uuid::nil(), Uuid::nil()).await;
    fields["source"] = json!({ "kind": source.0, "id": { "id": source.1 } });
    fields["destination"] = json!({ "kind": destination.0, "id": { "id": destination.1 } });
    env.applied(vec![flow("request", fields)]).await;
    sqlx::query_scalar(
        "SELECT id FROM flow.flow WHERE source_id = $1 AND destination_id = $2 LIMIT 1",
    )
    .bind(source.1)
    .bind(destination.1)
    .fetch_one(&env.pool)
    .await
    .unwrap()
}

async fn world(pool: &PgPool) -> World {
    let env = Env::new(pool).await;
    // The reverse direction, dev to prod, allowed too.
    let admin = id_of(
        pool,
        "SELECT id FROM policy.policy_flow_type WHERE key = $1",
        "admin",
    )
    .await;
    env.applied(vec![policy(
        "rule.create",
        json!({ "key": "dev-prod", "labels": { "fr": "Dev vers prod" }, "effect": "allow",
                "flow_types": [{ "id": admin }],
                "source": { "policy.environment": ["dev"] },
                "destination": { "policy.environment": ["prod"] } }),
    )])
    .await;
    let app = env.address("192.0.2.0/24", "192.0.2.10", "prod").await;
    let db = env.address("198.51.100.0/24", "198.51.100.20", "dev").await;
    let db6 = env.address("2001:db8::/64", "2001:db8::20", "dev").await;
    let network = id_of(
        pool,
        "SELECT id FROM prefix WHERE prefix = $1::cidr",
        "198.51.100.0/24",
    )
    .await;
    let mine = id_of(
        pool,
        "SELECT id FROM prefix WHERE prefix = $1::cidr",
        "192.0.2.0/24",
    )
    .await;
    let other = entity(&env, "other").await;
    let beta = entity(&env, "beta").await;
    let eth0 = machine(&env, "srv-app-01", env.acme, "eth0", &[app]).await;
    let eth1 = machine(&env, "srv-db-01", beta, "eth1", &[db, db6]).await;
    let outgoing = flow_between(&env, ("interface", eth0), ("prefix", network)).await;
    let incoming = flow_between(&env, ("prefix", network), ("interface", eth0)).await;
    let to_eth1 = flow_between(&env, ("interface", eth0), ("interface", eth1)).await;
    let alice = env.person().await;
    grant(
        &env,
        alice,
        &["flow.read", "flow.edit"],
        "entity",
        Some(other),
    )
    .await;
    grant(
        &env,
        alice,
        &["core.address.read", "core.prefix.read"],
        "prefix",
        Some(mine),
    )
    .await;
    grant(
        &env,
        alice,
        &["machines.read", "machines.interface.read"],
        "entity",
        Some(env.acme),
    )
    .await;
    World {
        env,
        eth0,
        eth1,
        network,
        outgoing,
        incoming,
        to_eth1,
        alice,
    }
}

/// REQ-027-1 (P1, P3): Alice, `flow.read` elsewhere, reads the flows of `srv-app-01` in
/// both directions, on its page and in `/v1/flows`; Bob, `flow.read` without the read of
/// an endpoint, gets the 404 of an absent flow; without `flow.read`, nothing, even
/// reading both endpoints.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_027_1_a_flow_read_through_an_endpoint(pool: PgPool) {
    let w = world(&pool).await;
    let alice = w.env.app(w.alice);
    let (status, page) = call(
        &alice,
        Method::GET,
        &format!("/v1/interfaces/{}/flows", w.eth0),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let mut expected = vec![
        w.outgoing.to_string(),
        w.incoming.to_string(),
        w.to_eth1.to_string(),
    ];
    expected.sort();
    assert_eq!(ids(&page), expected, "{page}");
    let (_, page) = call(&alice, Method::GET, "/v1/flows", None).await;
    assert_eq!(ids(&page), expected, "{page}");
    let (status, _) = call(
        &alice,
        Method::GET,
        &format!("/v1/flows/{}", w.incoming),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Bob: `flow.read` elsewhere, no endpoint read.
    let bob = w.env.person().await;
    let other = id_of(
        &pool,
        "SELECT id FROM entity WHERE slug = $1 AND valid_period @> now()",
        "other",
    )
    .await;
    grant(&w.env, bob, &["flow.read"], "entity", Some(other)).await;
    let bob = w.env.app(bob);
    let (status, _) = call(
        &bob,
        Method::GET,
        &format!("/v1/flows/{}", w.outgoing),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, page) = call(&bob, Method::GET, "/v1/flows", None).await;
    assert!(ids(&page).is_empty(), "{page}");

    // Carol reads both endpoints but holds no `flow.read`.
    let carol = w.env.person().await;
    grant(
        &w.env,
        carol,
        &[
            "core.address.read",
            "core.prefix.read",
            "machines.read",
            "machines.interface.read",
        ],
        "global",
        None,
    )
    .await;
    let carol = w.env.app(carol);
    let (_, page) = call(&carol, Method::GET, "/v1/flows", None).await;
    assert!(ids(&page).is_empty(), "{page}");
    let (status, _) = call(
        &carol,
        Method::GET,
        &format!("/v1/flows/{}", w.outgoing),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The test of a connection counts the flows read through an endpoint (REQ-011-26):
    // Dave reads the addresses, not the entity of the flow.
    let dave = w.env.person().await;
    grant(&w.env, dave, &["flow.read"], "entity", Some(other)).await;
    grant(
        &w.env,
        dave,
        &[
            "policy.verdict.read",
            "core.address.read",
            "core.prefix.read",
        ],
        "global",
        None,
    )
    .await;
    let dave = w.env.app(dave);
    let (app, db) = (
        id_of(
            &pool,
            "SELECT id FROM address WHERE address = $1::inet",
            "192.0.2.10",
        )
        .await,
        id_of(
            &pool,
            "SELECT id FROM address WHERE address = $1::inet",
            "198.51.100.20",
        )
        .await,
    );
    let answer = checked(
        &dave,
        &format!("address:{app}"),
        &format!("address:{db}"),
        "",
    )
    .await;
    assert_eq!(answer["pairs"][0]["open"], true, "{answer}");
    let cited: Vec<String> = opening(&answer).into_iter().map(|(id, _)| id).collect();
    assert!(cited.contains(&w.outgoing.to_string()), "{answer}");
}

/// REQ-027-2 (P2): the endpoint Alice does not read is shown by its value, `readable:
/// false`, without identifier, name nor machine; the rest of the flow stays; the
/// explanation is reduced (REQ-010-25). A reader of the entity sees the flow as before.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_027_2_the_endpoint_out_of_scope(pool: PgPool) {
    let w = world(&pool).await;
    let alice = w.env.app(w.alice);
    let (status, flow) = call(
        &alice,
        Method::GET,
        &format!("/v1/flows/{}", w.outgoing),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{flow}");
    assert_eq!(
        flow["destination"],
        json!({ "kind": "prefix", "value": "198.51.100.0/24", "readable": false }),
        "{flow}"
    );
    assert!(flow.get("source").is_none(), "{flow}");
    assert!(flow["fields"].get("destination_id").is_none(), "{flow}");
    assert_eq!(flow["fields"]["source_id"], w.eth0.to_string());
    assert!(!flow.to_string().contains(&w.network.to_string()), "{flow}");
    assert_eq!(flow["state"], "active");
    assert_eq!(flow["summary"], "allowed");
    assert!(flow["fields"]["service_id"].is_string());

    let (status, page) = call(&alice, Method::GET, "/v1/flows", None).await;
    assert_eq!(status, StatusCode::OK);
    let listed = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == w.to_eth1.to_string())
        .unwrap()
        .clone();
    assert_eq!(
        listed["destination"],
        json!({ "kind": "interface", "values": ["198.51.100.20", "2001:db8::20"],
                "readable": false }),
        "{listed}"
    );
    let text = listed.to_string();
    assert!(
        !text.contains("eth1") && !text.contains("srv-db-01"),
        "{text}"
    );
    assert!(!text.contains(&w.eth1.to_string()), "{text}");

    let (status, page) = call(
        &alice,
        Method::GET,
        &format!("/v1/flows/{}/evaluations", w.outgoing),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page["items"][0]["source"].is_null(), "{page}");
    assert!(page["items"][0]["destination"].is_null(), "{page}");
    assert!(page["items"][0]["decisive"].is_null(), "{page}");
    assert_eq!(page["items"][0]["verdict"], "allowed");

    // A reader of the entity of the flow: the endpoints in `fields`, no value.
    let reader = w.env.person().await;
    grant(&w.env, reader, &["flow.read"], "entity", Some(w.env.acme)).await;
    let reader = w.env.app(reader);
    let (_, flow) = call(
        &reader,
        Method::GET,
        &format!("/v1/flows/{}", w.outgoing),
        None,
    )
    .await;
    assert!(flow.get("destination").is_none(), "{flow}");
    assert_eq!(flow["fields"]["destination_id"], w.network.to_string());
}

/// REQ-027-3: the writes keep to REQ-011-17 (the 404 of an absent flow); a filter by
/// object keeps to the objects Alice reads, a filter by value meets the endpoints shown
/// by value.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_027_3_writes_and_filters(pool: PgPool) {
    let w = world(&pool).await;
    let alice = w.env.app(w.alice);
    let (status, answer) = patch(
        &alice,
        w.outgoing,
        &json!({ "end": later(400), "motive": "prolonger" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{answer}");

    let (status, page) = call(
        &alice,
        Method::GET,
        "/v1/flows?endpoint=198.51.100.0/24",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(ids(&page).contains(&w.outgoing.to_string()), "{page}");
    let (_, page) = call(
        &alice,
        Method::GET,
        &format!("/v1/flows?endpoint=prefix:{}", w.network),
        None,
    )
    .await;
    assert!(ids(&page).is_empty(), "{page}");
    let (_, page) = call(
        &alice,
        Method::GET,
        &format!("/v1/prefixes/{}/flows", w.network),
        None,
    )
    .await;
    assert!(ids(&page).is_empty(), "{page}");
    let (_, page) = call(
        &alice,
        Method::GET,
        &format!("/v1/flows?endpoint=interface:{}", w.eth0),
        None,
    )
    .await;
    assert_eq!(ids(&page).len(), 3, "{page}");
    let (_, page) = call(
        &alice,
        Method::GET,
        "/v1/flows?endpoint=2001:db8::/32",
        None,
    )
    .await;
    assert_eq!(ids(&page), [w.to_eth1.to_string()], "{page}");
    let (status, _) = call(
        &alice,
        Method::GET,
        "/v1/flows?endpoint=198.51.100.0/33",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// REQ-027-4: the pages a reader through the endpoints reads, measured by `bench-flow`:
/// the flows and the non-conformities of the flows it reads through an endpoint.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_027_4_the_pages_of_a_reader_by_endpoints(pool: PgPool) {
    let w = world(&pool).await;
    // The network becomes `prod`: the flow `eth0` → network is no longer allowed.
    let environment = w.env.property("policy.environment").await;
    w.env
        .applied(vec![Operation::AssignProperty {
            object: Ref::Id(w.network),
            kind: ObjectKind::Prefix,
            property: Ref::Id(environment),
            values: vec![RawInput::Text("prod".into())],
        }])
        .await;
    let alice = w.env.app(w.alice);
    let (status, page) = call(&alice, Method::GET, "/v1/flow-nonconformities", None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let flows: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["flow"].as_str())
        .collect();
    assert!(flows.contains(&w.outgoing.to_string().as_str()), "{page}");

    // A stranger to the endpoints, `flow.read` elsewhere: no non-conformity.
    let bob = w.env.person().await;
    let other = id_of(
        &pool,
        "SELECT id FROM entity WHERE slug = $1 AND valid_period @> now()",
        "other",
    )
    .await;
    grant(&w.env, bob, &["flow.read"], "entity", Some(other)).await;
    let (_, page) = call(
        &w.env.app(bob),
        Method::GET,
        "/v1/flow-nonconformities",
        None,
    )
    .await;
    assert!(page["items"].as_array().unwrap().is_empty(), "{page}");

    // The flows paged: the second page follows the first, by the cursor.
    let (status, page) = call(&alice, Method::GET, "/v1/flows?page_size=2", None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let mut seen = ids(&page);
    assert_eq!(seen.len(), 2, "{page}");
    let next = page["links"]["next"].as_str().unwrap().to_owned();
    let (_, page) = call(&alice, Method::GET, &next, None).await;
    seen.extend(ids(&page));
    assert_eq!(seen.len(), 3, "{page}");
}
