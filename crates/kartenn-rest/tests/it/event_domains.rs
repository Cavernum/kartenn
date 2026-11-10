//! The domains of a changeset published on the stream (spec 022, REQ-022-4; spec 021, A40,
//! task 22j): the client reads again only the trees of the domains an event `changes`
//! names, so every operation that writes a prefix, a range or an address, of the core or
//! of a module, names its domain. The event copies the domains of its row of the outbox
//! (`events::translate`): the row is checked here.

use std::time::Duration;

use crate::vlan::env::{Env, op};
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ObjectKind, Operation, Outcome, Ref, submit};
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

/// The domains the row of the outbox of `changeset` names.
async fn named(pool: &PgPool, changeset: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar(
        "SELECT domain_ids FROM outbox
          WHERE changeset_id = $1 AND payload ->> 'type' = 'org.kartenn.changeset.applied'",
    )
    .bind(changeset)
    .fetch_one(pool)
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

    // A group of VLAN moved from one domain to another (task 22n): both are written,
    // locked and named; renamed in place, the one it is anchored on.
    env.applied(vec![op(
        "create_vlan_group",
        json!({"slug": "moved", "anchor": {"domain": {"id": lab}}, "owner": {"id": acme}}),
    )])
    .await;
    let moved = env.id("vlan_group", "moved").await;
    let to_annex = changeset(
        &env.applied(vec![op(
            "update_vlan_group",
            json!({"id": {"id": moved}, "anchor": {"domain": {"id": annex}}}),
        )])
        .await,
    );
    let mut both = vec![lab, annex];
    both.sort_unstable();
    assert_eq!(named(&pool, to_annex).await, both);
    let renamed = changeset(
        &env.applied(vec![op(
            "update_vlan_group",
            json!({"id": {"id": moved}, "slug": "annex-vlans"}),
        )])
        .await,
    );
    assert_eq!(named(&pool, renamed).await, vec![annex]);

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

/// The domains created and owned by `acme`, by slug.
async fn domains(env: &Env, slugs: &[&str]) -> Vec<Uuid> {
    let acme = env.id("entity", "acme").await;
    let owner = env.property("core.owner").await;
    let mut operations = Vec::new();
    for (local, slug) in (1..).zip(slugs) {
        operations.push(core(
            json!({"op": "create_domain", "local": local, "slug": slug}),
        ));
        operations.push(Operation::AssignProperty {
            object: Ref::Local(local),
            kind: ObjectKind::Domain,
            property: Ref::Id(owner),
            values: vec![RawInput::Text(acme.to_string())],
        });
    }
    env.applied(operations).await;
    let mut ids = Vec::new();
    for slug in slugs {
        ids.push(env.id("addressing_domain", slug).await);
    }
    ids
}

/// The domain a group of VLANs is anchored on now.
async fn anchored(pool: &PgPool, group: Uuid) -> Option<Uuid> {
    sqlx::query_scalar(
        "SELECT anchor_id FROM vlan_group
          WHERE id = $1 AND anchor_kind = 'domain' AND valid_period @> now()",
    )
    .bind(group)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// Two moves of one group out of `lab` at once (task 22o, ADR 0015, precision of
/// 2026-11-10): both read `lab` as its anchor, then wait for the lock of `lab`. The second
/// to take it finds the group moved by the first: it replays, locks and names the domain
/// the group really leaves, never `lab` it no longer writes.
#[kartenn_testing::pg_test]
async fn a_group_moved_twice_at_once_locks_the_domain_it_really_leaves(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let targets = domains(&env, &["annex", "third"]).await;
    env.applied(vec![op(
        "create_vlan_group",
        json!({"slug": "contested", "anchor": {"domain": {"id": lab}}}),
    )])
    .await;
    let group = env.id("vlan_group", "contested").await;
    env.applied(vec![op(
        "create_vlan",
        json!({"group": {"id": group}, "vid": 10, "name": "users"}),
    )])
    .await;

    // Another writer holds the lock of `lab` (same key as the store, ADR 0015).
    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lab.as_u64_pair().0.cast_signed())
        .execute(&mut *holder)
        .await
        .unwrap();
    let mut moves = Vec::new();
    for target in &targets {
        let store = env.store.clone();
        let request = env.request(
            vec![op(
                "update_vlan_group",
                json!({"id": {"id": group}, "anchor": {"domain": {"id": target}}}),
            )],
            None,
        );
        moves.push(tokio::spawn(async move {
            submit(&store, &Unrestricted, &request, MAX_OPERATIONS)
                .await
                .unwrap()
        }));
        // The moves queue on the lock of `lab` one after the other.
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(
        moves.iter().all(|m| !m.is_finished()),
        "both wait for `lab`"
    );
    holder.rollback().await.unwrap();
    let mut applied = Vec::new();
    for m in moves {
        let outcome = tokio::time::timeout(Duration::from_secs(10), m)
            .await
            .expect("both moves end once `lab` is free")
            .unwrap();
        applied.push(changeset(&outcome));
    }

    // One left `lab`, the other the domain the first moved the group to.
    let mut names = Vec::new();
    for c in &applied {
        names.push(named(&pool, *c).await);
    }
    let first = names
        .iter()
        .position(|n| n.contains(&lab))
        .expect("one move leaves `lab`");
    let second = 1 - first;
    let mut left = vec![lab, targets[first]];
    left.sort_unstable();
    assert_eq!(names[first], left);
    let mut replayed = vec![targets[first], targets[second]];
    replayed.sort_unstable();
    assert_eq!(names[second], replayed, "the second names what it writes");
    // The anchoring holds: the group, and so its VLAN, is in the domain of the last move.
    assert_eq!(anchored(&pool, group).await, Some(targets[second]));
    let vlan = current(&pool, "vlan", "vid", "10").await;
    let vlan_domain: Option<Uuid> = sqlx::query_scalar(
        "SELECT g.anchor_id FROM vlan v
           JOIN vlan_group g ON g.id = v.group_id AND g.valid_period @> now()
          WHERE v.id = $1 AND v.valid_period @> now()",
    )
    .bind(vlan)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(vlan_domain, Some(targets[second]));
}

/// A policy that holds every changeset for review.
fn hold_all() -> Operation {
    Operation::SetApprovalPolicy {
        name: "all".into(),
        min_operations: Some(1),
        min_diff_objects: None,
        property_keys: Vec::new(),
        domains: Vec::new(),
        scopes: Vec::new(),
        author_roles: Vec::new(),
        author_groups: Vec::new(),
        bound_roles: Vec::new(),
        module_triggers: Vec::new(),
        enabled: true,
    }
}

/// A move whose transaction starts before another, but takes `lab` after it (task 22o,
/// review): its completion reads the group at the start of its transaction, `lab`, an
/// instant before the version the other committed from its later start. Under its locks
/// the last state is read: the group left `lab`, the move replays and leaves `annex`.
/// The order is fixed by a lock held, never by a delay: the validation of the pending move
/// opens its transaction, then waits for the row of its changeset.
#[kartenn_testing::pg_test]
async fn a_move_that_started_first_and_locked_last_replays(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let targets = domains(&env, &["annex", "third"]).await;
    let (annex, third) = (targets[0], targets[1]);
    env.applied(vec![op(
        "create_vlan_group",
        json!({"slug": "contested", "anchor": {"domain": {"id": lab}}}),
    )])
    .await;
    let group = env.id("vlan_group", "contested").await;
    let to = |domain: Uuid| {
        op(
            "update_vlan_group",
            json!({"id": {"id": group}, "anchor": {"domain": {"id": domain}}}),
        )
    };
    // The move to `third` waits for review; then the policy is switched off.
    env.applied(vec![hold_all()]).await;
    let pending = match env.submit(vec![to(third)]).await {
        Outcome::Pending { changeset, .. } => changeset,
        other => panic!("not pending: {other:?}"),
    };
    sqlx::query("UPDATE approval_policy SET enabled = false")
        .execute(&pool)
        .await
        .unwrap();
    let validator: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'v') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT state FROM changeset WHERE id = $1 FOR UPDATE")
        .bind(pending)
        .execute(&mut *holder)
        .await
        .unwrap();
    let validation = tokio::spawn({
        let store = env.store.clone();
        async move {
            kartenn_service::review::validate_pending(
                &store,
                &Unrestricted,
                &kartenn_service::expected::Sources::none(),
                pending,
                &validator.into(),
                MAX_OPERATIONS,
            )
            .await
            .unwrap()
        }
    });
    // Its transaction is open once it waits for the row of its changeset.
    let mut waiting = false;
    for _ in 0..500 {
        waiting = sqlx::query_scalar(
            "SELECT count(*) > 0 FROM pg_stat_activity
              WHERE wait_event_type = 'Lock' AND query LIKE 'SELECT state FROM changeset%'
                AND pid <> pg_backend_pid()",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if waiting {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(waiting, "the validation waits for its changeset");
    // A later move takes `lab` first and commits.
    let moved = changeset(&env.applied(vec![to(annex)]).await);
    let mut left = vec![lab, annex];
    left.sort_unstable();
    assert_eq!(named(&pool, moved).await, left);
    holder.rollback().await.unwrap();

    let outcome = tokio::time::timeout(Duration::from_secs(10), validation)
        .await
        .expect("the validation ends once its changeset is free")
        .unwrap();
    assert_eq!(changeset(&outcome), pending);
    let mut replayed = vec![annex, third];
    replayed.sort_unstable();
    assert_eq!(
        named(&pool, pending).await,
        replayed,
        "it locks what it leaves"
    );
    assert_eq!(anchored(&pool, group).await, Some(third));
}
