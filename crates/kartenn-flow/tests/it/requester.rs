//! What a requester reads of its own rows, and counts on the home page, by where it holds
//! `flow.request` (REQ-011-17; tasks 006-38, 006-39): a right withdrawn or ended by its
//! date, a right held on another entity than the row's, the confinement of its issuer, a
//! request pending a review that names no entity.

use std::collections::BTreeSet;
use std::time::Duration;

use axum::Router;
use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use kartenn_rest::stats::{Freshness, Snapshots};
use kartenn_service::Operation;

use crate::requests::{Env, REQUEST};
use crate::rest::{body, call, items, key};

/// An issuer of the tests (a documentation domain).
const ISSUER: &str = "https://idp.example.org";

/// The rights of a requester but `flow.request`.
fn others() -> Vec<&'static str> {
    REQUEST
        .iter()
        .copied()
        .filter(|p| *p != "flow.request")
        .collect()
}

/// `GET /v1/stats` as served to `app`.
async fn stats(app: &Router) -> Value {
    let (status, counters) = call(app, Method::GET, "/v1/stats", None).await;
    assert_eq!(status, StatusCode::OK, "{counters}");
    counters
}

/// Withdraws `flow.request` from the roles `identity` is bound to on `entity` (every
/// binding when `None`), as an administrator editing the role would.
async fn withdraw(pool: &PgPool, identity: Uuid, entity: Option<Uuid>) {
    sqlx::query(
        "DELETE FROM role_permission
          WHERE permission = 'flow.request'
            AND role_id IN (SELECT b.role_id FROM binding b
                              JOIN group_member m ON m.group_id = b.group_id
                             WHERE m.identity_id = $1
                               AND ($2::uuid IS NULL OR b.scope_id = $2))",
    )
    .bind(identity)
    .bind(entity)
    .execute(pool)
    .await
    .unwrap();
}

/// Binds `identity` to a role of `flow.request` on `entity` until `until`.
async fn bind_until(pool: &PgPool, identity: Uuid, entity: Uuid, until: OffsetDateTime) {
    let role = Uuid::now_v7();
    sqlx::query(
        "WITH r AS (
            INSERT INTO role (id, key, labels, valid_period, changeset_id)
            VALUES ($1, 'r_' || replace($1::text, '-', ''), jsonb_build_object('fr', 'r'),
                    tstzrange(now() - interval '1 day', NULL), gen_random_uuid())
            RETURNING id),
              p AS (
            INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
            SELECT r.id, 'flow.request', tstzrange(now() - interval '1 day', NULL),
                   gen_random_uuid() FROM r),
              g AS (
            INSERT INTO rbac_group (id, key, labels, valid_period, changeset_id)
            VALUES (gen_random_uuid(), 't-' || replace(gen_random_uuid()::text, '-', ''),
                    '{\"fr\": \"t\"}', tstzrange(now() - interval '1 day', NULL),
                    gen_random_uuid())
            RETURNING id),
              m AS (
            INSERT INTO group_member (id, group_id, member_kind, identity_id, valid_period,
                                      changeset_id)
            SELECT gen_random_uuid(), g.id, 'identity', $2::uuid,
                   tstzrange(now() - interval '1 day', NULL), gen_random_uuid() FROM g)
         INSERT INTO binding (id, group_id, role_id, scope_kind, scope_id, valid_period,
                              changeset_id)
         SELECT gen_random_uuid(), g.id, r.id, 'entity', $3::uuid,
                tstzrange(now() - interval '1 day', $4), gen_random_uuid() FROM g, r",
    )
    .bind(role)
    .bind(identity)
    .bind(entity)
    .bind(until)
    .execute(pool)
    .await
    .unwrap();
}

/// A person holding the other rights of a requester everywhere, and `flow.request` on
/// `acme`; a stranger holding the other rights only.
async fn requester_and_stranger(env: &Env) -> (Uuid, Uuid) {
    let author = env.granted(&others()).await;
    env.grant(author, &["flow.request"], Some(env.acme)).await;
    (author, env.granted(&others()).await)
}

/// `n` flows applied on `entity` as requested through `app`, their identifiers.
async fn applied_flows(env: &Env, app: &Router, entity: Uuid, n: usize) -> Vec<String> {
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let mut ids = Vec::new();
    for _ in 0..n {
        let mut fields = body(env, prod, dev).await;
        fields["entity"] = json!({ "id": entity });
        let (status, answer) = call(app, Method::POST, "/v1/flows", Some(fields)).await;
        assert_eq!(status, StatusCode::CREATED, "{answer}");
        ids.push(answer["flow"]["id"].as_str().unwrap().to_owned());
    }
    ids
}

/// Task 006-39: the counters of the home page are kept per subject and per where it holds
/// `flow.request`. Once the right is withdrawn, the next read serves what a subject
/// without it is served, not the snapshot kept while it held it.
#[kartenn_testing::pg_test]
async fn the_home_counter_follows_a_withdrawn_flow_request(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (author, stranger) = requester_and_stranger(&env).await;
    let (mine, strange) = (env.app(author), env.app(stranger));
    applied_flows(&env, &mine, env.acme, 2).await;
    let before = stats(&mine).await;
    assert_eq!(before["objects"]["flow.flow"], 2, "{before}");
    assert_ne!(before["objects"], stats(&strange).await["objects"]);
    withdraw(&pool, author, Some(env.acme)).await;
    let (after, expected) = (stats(&mine).await, stats(&strange).await);
    assert_eq!(after["objects"], expected["objects"], "{after}");
    assert_eq!(after["modules"], expected["modules"], "{after}");
}

/// Task 006-39: a binding of `flow.request` ended by its date alone, nothing written: the
/// next read of the home page serves what a subject without it is served.
#[kartenn_testing::pg_test]
async fn the_home_counter_follows_a_binding_ended_by_its_date(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (author, stranger) = requester_and_stranger(&env).await;
    let (mine, strange) = (env.app(author), env.app(stranger));
    applied_flows(&env, &mine, env.acme, 2).await;
    // The right now comes from a binding that ends in two seconds.
    withdraw(&pool, author, Some(env.acme)).await;
    let until: OffsetDateTime = sqlx::query_scalar("SELECT now() + interval '2 seconds'")
        .fetch_one(&pool)
        .await
        .unwrap();
    bind_until(&pool, author, env.acme, until).await;
    let before = stats(&mine).await;
    assert_eq!(before["objects"]["flow.flow"], 2, "{before}");
    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let ended: bool = sqlx::query_scalar("SELECT now() >= $1")
            .bind(until)
            .fetch_one(&pool)
            .await
            .unwrap();
        if ended {
            break;
        }
    }
    let (after, expected) = (stats(&mine).await, stats(&strange).await);
    assert_eq!(after["objects"], expected["objects"], "{after}");
    assert_eq!(after["modules"], expected["modules"], "{after}");
}
/// The entity `beta`, beside `acme`.
async fn beta(env: &Env) -> Uuid {
    env.applied(vec![Operation::CreateEntity {
        local: Some(1),
        slug: "beta".into(),
        parent: None,
    }])
    .await;
    env.slugged("entity", "beta").await
}

/// The rows of a requester on an entity: an applied flow, a request pending a review, a
/// rejected request.
struct Rows {
    entity: Uuid,
    flow: String,
    pending: String,
    rejected: String,
}

/// The rows `author` requests through `app` on each of `entities`.
async fn rows_on(env: &Env, app: &Router, author: Uuid, entities: &[Uuid]) -> Vec<Rows> {
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let mut rows = Vec::new();
    for &entity in entities {
        let mut answers = Vec::new();
        for (source, destination, status) in [
            (prod, dev, StatusCode::CREATED),
            (dev, prod, StatusCode::ACCEPTED),
        ] {
            let mut fields = body(env, source, destination).await;
            fields["entity"] = json!({ "id": entity });
            let (got, answer) = call(app, Method::POST, "/v1/flows", Some(fields)).await;
            assert_eq!(got, status, "{answer}");
            answers.push(answer);
        }
        let rejected = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO flow.flow_rejection (changeset_id, request, evaluation, reason,
                                              reviewed_by, requester_id, entity_id, rejected_at)
             VALUES ($1, '{}', '{}', 'non', $2, $2, $3, now())",
        )
        .bind(rejected)
        .bind(author)
        .bind(entity)
        .execute(&env.pool)
        .await
        .unwrap();
        rows.push(Rows {
            entity,
            flow: answers[0]["flow"]["id"].as_str().unwrap().to_owned(),
            pending: answers[1]["changeset"]["id"].as_str().unwrap().to_owned(),
            rejected: rejected.to_string(),
        });
    }
    rows
}

/// The keys of the rows `app` is served on `uri`, on one page.
async fn served(app: &Router, uri: &str, key: fn(&Value) -> String) -> BTreeSet<String> {
    let (status, page) = call(app, Method::GET, uri, None).await;
    assert_eq!(status, StatusCode::OK, "{uri}: {page}");
    assert!(page["links"]["next"].is_null(), "{uri}: one page");
    items(&page).iter().map(key).collect()
}

/// `app` reads the rows of `rows` on `entity`, and none of the others: on the lists of the
/// flows and of the non-conformities, by identifier, on the home page.
async fn reads_only_on(app: &Router, rows: &[Rows], entity: Uuid) {
    let (read, unread): (Vec<&Rows>, Vec<&Rows>) = rows.iter().partition(|r| r.entity == entity);
    assert!(!read.is_empty() && !unread.is_empty());
    let flows = served(app, "/v1/flows?state=active,requested&page_size=50", key).await;
    let expected: BTreeSet<String> = read
        .iter()
        .flat_map(|r| [r.flow.clone(), r.pending.clone()])
        .collect();
    assert_eq!(flows, expected);
    let changeset = |row: &Value| row["changeset"].as_str().unwrap_or_default().to_owned();
    let nonconformities = served(app, "/v1/flow-nonconformities?page_size=50", changeset).await;
    for r in &read {
        assert!(nonconformities.contains(&r.rejected), "{nonconformities:?}");
    }
    for r in &unread {
        let named = |key: &String| nonconformities.contains(key);
        assert!(
            !named(&r.rejected) && !named(&r.pending),
            "{nonconformities:?}"
        );
    }
    for (rows, status) in [(&read, StatusCode::OK), (&unread, StatusCode::NOT_FOUND)] {
        for r in rows {
            for uri in [
                format!("/v1/flows/{}", r.flow),
                format!("/v1/flows/{}/evaluations", r.flow),
            ] {
                let (got, answer) = call(app, Method::GET, &uri, None).await;
                assert_eq!(got, status, "{uri}: {answer}");
            }
        }
    }
    let counted = stats(app).await;
    assert_eq!(counted["objects"]["flow.flow"], read.len(), "{counted}");
}

/// Task 006-39 (REQ-011-17, task 006-38): `flow.request` held on another entity than the
/// row's reads nothing of it. The requester who keeps it on `beta` only reads its rows of
/// `beta`, not those of `acme`.
#[kartenn_testing::pg_test]
async fn a_right_on_another_entity_reads_none_of_the_rows(pool: PgPool) {
    let env = Env::new(&pool).await;
    let beta = beta(&env).await;
    let author = env.granted(&others()).await;
    for entity in [env.acme, beta] {
        env.grant(author, &["flow.request"], Some(entity)).await;
    }
    let mine = env.app(author);
    let rows = rows_on(&env, &mine, author, &[env.acme, beta]).await;
    withdraw(&pool, author, Some(env.acme)).await;
    reads_only_on(&mine, &rows, beta).await;
}

/// Task 006-39 (K4, K13, task 006-38): the confinement of the issuer bounds `flow.request`
/// held everywhere. Authenticated by an issuer attached to `acme`, the requester reads its
/// rows of `acme` only.
#[kartenn_testing::pg_test]
async fn the_confinement_of_the_issuer_bounds_the_rows_of_the_requester(pool: PgPool) {
    let env = Env::new(&pool).await;
    let beta = beta(&env).await;
    let author = env.granted(REQUEST).await;
    let rows = rows_on(&env, &env.app(author), author, &[env.acme, beta]).await;
    sqlx::query(
        "INSERT INTO issuer_attachment (issuer, entity_id, valid_period, changeset_id)
         VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(ISSUER)
    .bind(env.acme)
    .execute(&pool)
    .await
    .unwrap();
    reads_only_on(&env.app_issued(author, Some(ISSUER)), &rows, env.acme).await;
}

/// Task 006-39 (REQ-011-17, task 006-38): a request pending a review that names its entity
/// by a local reference (an entity the proposal creates) names none by identifier: its
/// requester reads it only while it holds `flow.request` everywhere, not on an entity.
#[kartenn_testing::pg_test]
async fn a_request_naming_no_entity_is_read_with_the_right_everywhere(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let author = env.granted(REQUEST).await;
    let app = env.app(author);
    let (status, answer) = call(
        &app,
        Method::POST,
        "/v1/flows",
        Some(body(&env, dev, prod).await),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{answer}");
    let changeset = answer["changeset"]["id"].as_str().unwrap().to_owned();
    // The proposal as one that creates its entity would keep it.
    let named: u64 = sqlx::query(
        "UPDATE changeset
            SET request = jsonb_set(request, '{operations,0,entity}', '{\"local\": 1}')
          WHERE id = $1::uuid AND request #>> '{operations,0,op}' = 'flow.request'",
    )
    .bind(&changeset)
    .execute(&pool)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(named, 1);
    let uri = "/v1/flows?state=requested&page_size=50";
    let row = served(&app, uri, key).await;
    assert_eq!(row, BTreeSet::from([changeset]));
    // On `acme`, where it was asked, it is not read: the request names no entity.
    withdraw(&pool, author, None).await;
    env.grant(author, &["flow.request"], Some(env.acme)).await;
    assert!(served(&app, uri, key).await.is_empty());
    env.grant(author, &["flow.request"], None).await;
    assert_eq!(served(&app, uri, key).await, row);
}

/// Waits until the database reaches `until`.
async fn until_past(pool: &PgPool, until: OffsetDateTime) {
    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let past: bool = sqlx::query_scalar("SELECT now() >= $1")
            .bind(until)
            .fetch_one(pool)
            .await
            .unwrap();
        if past {
            return;
        }
    }
}

/// Task 006-40 (a): a recomputation in the background reads the rights of its key, not
/// those of the moment. The right withdrawn (the key « without » is born), given back,
/// the outbox moved and the keys swept, then withdrawn again: the next read serves what a
/// subject without it is served, not a count made with the right under the key without.
#[kartenn_testing::pg_test]
async fn a_right_withdrawn_given_back_and_withdrawn_again_counts_without_it(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (author, stranger) = requester_and_stranger(&env).await;
    let snapshots = Snapshots::new(Freshness {
        refresh: Duration::ZERO,
        ..Freshness::default()
    });
    let (mine, strange) = (env.app_with(author, &snapshots), env.app(stranger));
    applied_flows(&env, &mine, env.acme, 2).await;
    assert_eq!(stats(&mine).await["objects"]["flow.flow"], 2);
    withdraw(&pool, author, Some(env.acme)).await;
    assert_eq!(
        stats(&mine).await["objects"],
        stats(&strange).await["objects"]
    );
    env.grant(author, &["flow.request"], Some(env.acme)).await;
    assert_eq!(stats(&mine).await["objects"]["flow.flow"], 2);
    // The outbox moves, and the active keys are recomputed: the key without too.
    env.address("10.3.0.0/24", "10.3.0.1", "dev").await;
    snapshots.sweep(&env.api()).await.unwrap();
    withdraw(&pool, author, Some(env.acme)).await;
    let (after, expected) = (stats(&mine).await, stats(&strange).await);
    assert_eq!(after["objects"], expected["objects"], "{after}");
    assert_eq!(after["modules"], expected["modules"], "{after}");
}

/// Task 006-40: a key whose rights are no longer those of its subject is not computed by
/// the sweep that finds it due, and is not due at the next one: it is dropped.
#[kartenn_testing::pg_test]
async fn a_key_whose_rights_moved_is_dropped_by_the_sweep(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (author, _) = requester_and_stranger(&env).await;
    let snapshots = Snapshots::new(Freshness {
        refresh: Duration::ZERO,
        ..Freshness::default()
    });
    let mine = env.app_with(author, &snapshots);
    applied_flows(&env, &mine, env.acme, 1).await;
    stats(&mine).await;
    // The key with the right is left behind; the key without is born.
    withdraw(&pool, author, Some(env.acme)).await;
    stats(&mine).await;
    let computed = snapshots.computations();
    for (sweeps, network, address) in [
        (1, "10.3.0.0/24", "10.3.0.1"),
        (2, "10.4.0.0/24", "10.4.0.1"),
    ] {
        env.address(network, address, "dev").await;
        snapshots.sweep(&env.api()).await.unwrap();
        // What the rights share, and the key without: not the key with.
        assert_eq!(snapshots.computations(), computed + 2 * sweeps);
    }
}

/// Task 006-40 (b): a flow ended by its date alone, nothing written, leaves the counter
/// once its snapshot is older than [`Freshness::max_age`].
#[kartenn_testing::pg_test]
async fn a_flow_ended_by_its_date_leaves_the_counter_within_the_age(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (author, _) = requester_and_stranger(&env).await;
    let snapshots = Snapshots::new(Freshness {
        refresh: Duration::ZERO,
        max_age: Duration::from_secs(1),
        ..Freshness::default()
    });
    let mine = env.app_with(author, &snapshots);
    let flows = applied_flows(&env, &mine, env.acme, 2).await;
    let until: OffsetDateTime = sqlx::query_scalar("SELECT now() + interval '2 seconds'")
        .fetch_one(&pool)
        .await
        .unwrap();
    // Its period ends in two seconds, as a flow asked until a date.
    let ended = sqlx::query(
        "UPDATE flow.flow SET valid_period = tstzrange(lower(valid_period), $2)
          WHERE id = $1::uuid AND upper_inf(valid_period)",
    )
    .bind(&flows[0])
    .bind(until)
    .execute(&pool)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(ended, 1);
    assert_eq!(stats(&mine).await["objects"]["flow.flow"], 2);
    until_past(&pool, until).await;
    snapshots.sweep(&env.api()).await.unwrap();
    let after = stats(&mine).await;
    assert_eq!(after["objects"]["flow.flow"], 1, "{after}");
}
