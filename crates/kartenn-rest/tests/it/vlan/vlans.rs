//! The VLANs through the write pipeline (spec 026; spec 008, REQ-008-14 to 16): a VID and
//! a name unique in their group at every instant, under the key of the group, the
//! exclusion constraints as the last defence; a VID free again once its VLAN is closed; a
//! VLAN that moves to another group under the same rules.

use std::time::Duration;

use super::env::{Env, op, rules};
use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ObjectKind, Operation, Outcome, Ref, submit};
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;

/// 2027-01-01T00:00:00Z, in microseconds.
const NEW_YEAR: i64 = 1_798_761_600_000_000;

/// The class of the named locks of the store (`write.rs`, REQ-008-27); the second key is
/// the bucket `hashtext(key) & 1023` (ADR 0015, amendment of 2026-10-22).
const NAMED_LOCK_CLASS: i32 = 0x6b61_726b;

impl Env {
    /// A group `slug` anchored on `acme`, its range from `min` to `max`.
    async fn group(&self, slug: &str, (min, max): (i64, i64)) -> Uuid {
        let acme = self.id("entity", "acme").await;
        self.applied(vec![op(
            "create_vlan_group",
            json!({"slug": slug, "anchor": {"entity": {"id": acme}},
                   "vid_min": min, "vid_max": max}),
        )])
        .await;
        self.id("vlan_group", slug).await
    }

    /// The VLAN `vid` living now in `group`.
    async fn vlan(&self, group: Uuid, vid: i16) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM vlan
              WHERE group_id = $1 AND vid = $2 AND valid_period @> now()",
        )
        .bind(group)
        .bind(vid)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The group of the VLAN `vlan` now, as the tree of the VLANs reads it.
    async fn parent(&self, vlan: Uuid) -> Option<Uuid> {
        sqlx::query_scalar(
            "SELECT parent_id FROM vlan_tree WHERE id = $1 AND valid_period @> now()",
        )
        .bind(vlan)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn create(group: Uuid, vid: i64, name: &str) -> Operation {
    op(
        "create_vlan",
        json!({"group": {"id": group}, "vid": vid, "name": name}),
    )
}

#[kartenn_testing::pg_test]
async fn concurrent_same_vid_one_wins(pool: PgPool) {
    let env = Env::new(&pool).await;
    let group = env.group("edge", (1, 4094)).await;
    // Hold the key of the group, as the store takes it, so that both submissions queue
    // behind it; then release them together.
    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext($2) & 1023)")
        .bind(NAMED_LOCK_CLASS)
        .bind(kartenn_service::vlan::group_key(group))
        .execute(&mut *holder)
        .await
        .unwrap();
    let spawn = |name: &str| {
        let store = env.store.clone();
        let request = env.request(vec![create(group, 10, name)], None);
        tokio::spawn(async move {
            submit(&store, &Unrestricted, &request, MAX_OPERATIONS)
                .await
                .unwrap()
        })
    };
    let (first, second) = (spawn("users"), spawn("guests"));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!first.is_finished() && !second.is_finished());
    holder.rollback().await.unwrap();
    let outcomes = [first.await.unwrap(), second.await.unwrap()];
    let applied = outcomes
        .iter()
        .filter(|o| matches!(o, Outcome::Applied { .. }))
        .count();
    assert_eq!(applied, 1, "{outcomes:?}");
    let refused = outcomes
        .iter()
        .find(|o| !matches!(o, Outcome::Applied { .. }))
        .unwrap();
    assert!(
        rules(refused).contains(&"core.vlan.vid_duplicate"),
        "{refused:?}"
    );
}

#[kartenn_testing::pg_test]
async fn same_vid_two_groups(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (a, b) = (
        env.group("a", (1, 4094)).await,
        env.group("b", (1, 4094)).await,
    );
    env.applied(vec![create(a, 10, "users"), create(b, 10, "users")])
        .await;
    // Twice in one group, in the same changeset: refused.
    let refused = env
        .submit(vec![create(a, 20, "x"), create(a, 20, "y")])
        .await;
    assert!(
        rules(&refused).contains(&"core.vlan.vid_duplicate"),
        "{refused:?}"
    );
    // Outside the range of the group, or outside 1 to 4094.
    let narrow = env.group("narrow", (100, 199)).await;
    let refused = env.submit(vec![create(narrow, 10, "low")]).await;
    assert!(
        rules(&refused).contains(&"core.vlan.vid_out_of_range"),
        "{refused:?}"
    );
    let refused = env.submit(vec![create(narrow, 4095, "bad")]).await;
    assert!(
        rules(&refused).contains(&"core.vlan.vid_invalid"),
        "{refused:?}"
    );
    // In a group created in the same changeset.
    let acme = env.id("entity", "acme").await;
    env.applied(vec![
        op(
            "create_vlan_group",
            json!({"local": 1, "slug": "fresh", "anchor": {"entity": {"id": acme}}}),
        ),
        op(
            "create_vlan",
            json!({"group": {"local": 1}, "vid": 10, "name": "users", "status": "reserved"}),
        ),
    ])
    .await;
    // In the tree of the VLANs, under its group.
    let fresh = env.id("vlan_group", "fresh").await;
    let vlan = env.vlan(fresh, 10).await;
    assert_eq!(env.parent(vlan).await, Some(fresh));
}

#[kartenn_testing::pg_test]
async fn vid_reused_after_closure(pool: PgPool) {
    let env = Env::new(&pool).await;
    let group = env.group("edge", (1, 4094)).await;
    env.applied(vec![create(group, 10, "users")]).await;
    let old = env.vlan(group, 10).await;
    // The VLAN 10 ends on 2027-01-01.
    let closed = env
        .submit_from(
            vec![Operation::Close {
                kind: ObjectKind::Vlan,
                id: Ref::Id(old),
                expected: None,
                cascade: false,
            }],
            Some(NEW_YEAR),
        )
        .await;
    assert!(matches!(closed, Outcome::Applied { .. }), "{closed:?}");
    // Before that date, the VID is taken; from it on, free again, name included.
    let refused = env.submit(vec![create(group, 10, "again")]).await;
    assert!(
        rules(&refused).contains(&"core.vlan.vid_duplicate"),
        "{refused:?}"
    );
    let reused = env
        .submit_from(vec![create(group, 10, "users")], Some(NEW_YEAR))
        .await;
    assert!(matches!(reused, Outcome::Applied { .. }), "{reused:?}");
}

#[kartenn_testing::pg_test]
async fn move_to_group_with_taken_vid_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (a, b) = (
        env.group("a", (1, 4094)).await,
        env.group("b", (1, 99)).await,
    );
    env.applied(vec![
        create(a, 10, "users"),
        create(b, 10, "staff"),
        create(a, 200, "servers"),
    ])
    .await;
    let users = env.vlan(a, 10).await;
    let refused = env
        .submit(vec![op(
            "update_vlan",
            json!({"id": {"id": users}, "group": {"id": b}}),
        )])
        .await;
    assert!(
        rules(&refused).contains(&"core.vlan.vid_duplicate"),
        "{refused:?}"
    );
    // Outside the range of the group it joins: refused as well.
    let servers = env.vlan(a, 200).await;
    let refused = env
        .submit(vec![op(
            "update_vlan",
            json!({"id": {"id": servers}, "group": {"id": b}}),
        )])
        .await;
    assert!(
        rules(&refused).contains(&"core.vlan.vid_out_of_range"),
        "{refused:?}"
    );
    // With a free VID in range, the move is accepted; the tree of the VLANs follows.
    env.applied(vec![op(
        "update_vlan",
        json!({"id": {"id": users}, "group": {"id": b}, "vid": 11}),
    )])
    .await;
    assert_eq!(env.vlan(b, 11).await, users);
    assert_eq!(env.parent(users).await, Some(b));
}

#[kartenn_testing::pg_test]
async fn name_duplicate_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let group = env.group("edge", (1, 4094)).await;
    env.applied(vec![
        create(group, 10, "users"),
        create(group, 11, "guests"),
    ])
    .await;
    let refused = env.submit(vec![create(group, 12, "users")]).await;
    assert!(
        rules(&refused).contains(&"core.vlan.name_duplicate"),
        "{refused:?}"
    );
    // Renaming onto a name taken: refused; onto a free one, NFC-normalised: accepted.
    let guests = env.vlan(group, 11).await;
    let refused = env
        .submit(vec![op(
            "update_vlan",
            json!({"id": {"id": guests}, "name": "users"}),
        )])
        .await;
    assert!(
        rules(&refused).contains(&"core.vlan.name_duplicate"),
        "{refused:?}"
    );
    env.applied(vec![op(
        "update_vlan",
        json!({"id": {"id": guests}, "name": "Invite\u{301}s", "status": "deprecated"}),
    )])
    .await;
    let (name, status): (String, String) =
        sqlx::query_as("SELECT name, status FROM vlan WHERE id = $1 AND valid_period @> now()")
            .bind(guests)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((name.as_str(), status.as_str()), ("Invités", "deprecated"));
    let refused = env.submit(vec![create(group, 13, "")]).await;
    assert!(
        rules(&refused).contains(&"core.vlan.name_invalid"),
        "{refused:?}"
    );
}
