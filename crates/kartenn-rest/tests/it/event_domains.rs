//! The domains of a changeset published on the stream (spec 022, REQ-022-4; spec 021, A40,
//! task 22j): the client reads again only the trees of the domains an event `changes`
//! names, so every operation that writes a prefix, a range or an address, of the core or
//! of a module, names its domain. The event copies the domains of its row of the outbox
//! (`events::translate`): the row is checked here.

use crate::vlan::env::{Env, op};
use kartenn_service::operations::RawInput;
use kartenn_service::{ObjectKind, Operation, Outcome, Ref};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

/// An operation of the core from its JSON fields.
fn core(fields: Value) -> Operation {
    serde_json::from_value(fields).unwrap()
}

fn changeset(outcome: &Outcome) -> Uuid {
    match outcome {
        Outcome::Applied { changeset, .. } => *changeset,
        other => panic!("not applied: {other:?}"),
    }
}

/// The current object of `table` whose `column`, as text, is `value`.
async fn current(pool: &PgPool, table: &str, column: &str, value: &str) -> Uuid {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT id FROM {table} WHERE {column}::text = $1 AND valid_period @> now()"
    )))
    .bind(value)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The assignment to the prefix `prefix` of `key`, the object `value`.
async fn assign(env: &Env, prefix: Uuid, key: &str, value: Uuid) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(prefix),
        kind: ObjectKind::Prefix,
        property: Ref::Id(env.property(key).await),
        values: vec![RawInput::Text(value.to_string())],
    }
}

/// The last number of the outbox.
async fn top(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT coalesce(max(seq), 0) FROM outbox")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Every object placed in a domain that an applied changeset after `from` names, with
/// whether its row of the outbox names that domain: `(changeset, kind, object, named)`.
async fn placed(pool: &PgPool, from: i64) -> Vec<(Uuid, String, Uuid, bool)> {
    sqlx::query_as(
        "SELECT DISTINCT o.changeset_id, c.kind, c.id, c.domain_id = ANY(o.domain_ids)
           FROM outbox o
          CROSS JOIN LATERAL jsonb_array_elements_text(o.payload -> 'data' -> 'objects') AS x(id)
           JOIN carrier_object c ON c.id = x.id::uuid
          WHERE o.seq > $1 AND o.payload ->> 'type' = 'org.kartenn.changeset.applied'
            AND c.kind IN ('prefix', 'range', 'address') AND c.domain_id IS NOT NULL",
    )
    .bind(from)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[kartenn_testing::pg_test]
async fn every_written_domain_is_named(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let acme = env.id("entity", "acme").await;
    let owner = env.property("core.owner").await;
    env.applied(vec![
        core(json!({"op": "create_domain", "local": 1, "slug": "annex"})),
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Domain,
            property: Ref::Id(owner),
            values: vec![RawInput::Text(acme.to_string())],
        },
    ])
    .await;
    let annex = env.id("addressing_domain", "annex").await;
    let from = top(&pool).await;
    // Creations in two domains at once.
    env.applied(vec![
        core(json!({"op": "create_prefix", "domain": {"id": lab},
                    "prefix": "192.0.2.0/24", "nature": "network"})),
        core(json!({"op": "create_range", "domain": {"id": lab},
                    "first": "192.0.2.10", "last": "192.0.2.20", "usage": "static"})),
        core(json!({"op": "create_address", "domain": {"id": lab},
                    "address": "192.0.2.1", "usage": "gateway"})),
        core(json!({"op": "create_prefix", "domain": {"id": annex},
                    "prefix": "198.51.100.0/24", "nature": "container"})),
        core(json!({"op": "create_prefix", "domain": {"id": annex},
                    "prefix": "2001:db8::/48", "nature": "container"})),
    ])
    .await;
    let network = current(&pool, "prefix", "prefix", "192.0.2.0/24").await;
    let range = current(&pool, "ip_range", "host(lower(range))", "192.0.2.10").await;
    let address = current(&pool, "address", "host(address)", "192.0.2.1").await;
    let container = current(&pool, "prefix", "prefix", "198.51.100.0/24").await;
    let six = current(&pool, "prefix", "prefix", "2001:db8::/48").await;
    // Modifications.
    let updated = changeset(
        &env.applied(vec![
            core(json!({"op": "update_range", "id": {"id": range}, "usage": "reserved"})),
            core(json!({"op": "update_address", "id": {"id": address}, "usage": "host"})),
            core(json!({"op": "update_prefix", "id": {"id": container}, "nature": "network"})),
        ])
        .await,
    );
    // An assignment of a property alone, then one beside operations of the VLANs (a
    // module of the core, REQ-026-1).
    env.applied(vec![assign(&env, network, "core.owner", acme).await])
        .await;
    env.applied(vec![
        op(
            "create_vlan_group",
            json!({"local": 1, "slug": "lab-vlans", "anchor": {"domain": {"id": lab}},
                   "owner": {"id": acme}}),
        ),
        op(
            "create_vlan",
            json!({"group": {"local": 1}, "vid": 10, "name": "v10"}),
        ),
    ])
    .await;
    let vlan = current(&pool, "vlan", "vid", "10").await;
    env.applied(vec![
        op("update_vlan", json!({"id": {"id": vlan}, "name": "users"})),
        assign(&env, network, "core.vlan", vlan).await,
        core(json!({"op": "create_prefix", "domain": {"id": annex},
                    "prefix": "203.0.113.0/24", "nature": "network"})),
    ])
    .await;
    // A restoration (the operation of an inverse changeset).
    env.applied(vec![core(
        json!({"op": "restore", "kind": "range", "id": {"id": range}, "before": updated}),
    )])
    .await;
    // Closures, the last in cascade.
    env.applied(vec![
        core(json!({"op": "close", "kind": "address", "id": {"id": address}})),
        core(json!({"op": "close", "kind": "prefix", "id": {"id": six}})),
    ])
    .await;
    env.applied(vec![core(
        json!({"op": "close", "kind": "prefix", "id": {"id": network}, "cascade": true}),
    )])
    .await;

    let placed = placed(&pool, from).await;
    let unnamed: Vec<_> = placed.iter().filter(|(.., named)| !named).collect();
    assert!(unnamed.is_empty(), "domains not named: {unnamed:?}");
    let kinds: std::collections::BTreeSet<&str> =
        placed.iter().map(|(_, k, ..)| k.as_str()).collect();
    assert_eq!(kinds, ["address", "prefix", "range"].into(), "{placed:?}");
    let changesets: std::collections::BTreeSet<Uuid> = placed.iter().map(|(c, ..)| *c).collect();
    // The creation of the VLANs names no prefix: the seven others.
    assert_eq!(changesets.len(), 7, "every changeset checked: {placed:?}");
}
