//! The groups of VLAN through the write pipeline (spec 008, REQ-008-12, 13; task 008-10):
//! an owner required, defaulted on a domain anchor; a slug unique at every instant; a
//! range that keeps the VLANs of its group; an anchor that moves the group to another
//! scope.

use crate::support::env;

use env::{Env, messages, op, rules};
use kartenn_service::Outcome;
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn group_requires_an_owner(pool: PgPool) {
    let env = Env::new(&pool).await;
    let paris = env.id("site", "paris").await;
    let acme = env.id("entity", "acme").await;
    // On a site, without owner: refused by the obligation of the core.
    let refused = env
        .submit(vec![op(
            "create_group",
            json!({"slug": "dc-paris", "anchor": {"site": {"id": paris}}}),
        )])
        .await;
    assert!(
        rules(&refused).contains(&"prop.assignment.required"),
        "{refused:?}"
    );
    // With its owner: the group carries both its owner and its site.
    env.applied(vec![op(
        "create_group",
        json!({"slug": "dc-paris", "anchor": {"site": {"id": paris}}, "owner": {"id": acme}}),
    )])
    .await;
    let group = env.id("vlan.vlan_group", "dc-paris").await;
    assert_eq!(env.owner(group).await, Some(acme));
    let site: Option<String> = sqlx::query_scalar(
        "SELECT a.v_uuid::text FROM property_assignment a JOIN property_declaration d
             ON d.id = a.property_id AND d.key = 'core.site'
          WHERE a.object_id = $1 AND a.valid_period @> now()",
    )
    .bind(group)
    .fetch_optional(&pool)
    .await
    .unwrap()
    .flatten();
    assert_eq!(site, Some(paris.to_string()));
    // On an entity: the entity owns it; another owner is refused.
    let other = env.id("entity", "other").await;
    let refused = env
        .submit(vec![op(
            "create_group",
            json!({"slug": "g-acme", "anchor": {"entity": {"id": acme}}, "owner": {"id": other}}),
        )])
        .await;
    assert!(
        rules(&refused).contains(&"vlan.group.anchor"),
        "{refused:?}"
    );
    env.applied(vec![op(
        "create_group",
        json!({"slug": "g-acme", "anchor": {"entity": {"id": acme}}}),
    )])
    .await;
    assert_eq!(
        env.owner(env.id("vlan.vlan_group", "g-acme").await).await,
        Some(acme)
    );
    // Bounds of the range out of 1 to 4094.
    for (min, max) in [(0, 10), (1, 4095)] {
        let refused = env
            .submit(vec![op(
                "create_group",
                json!({"slug": "bad", "anchor": {"entity": {"id": acme}},
                       "vid_min": min, "vid_max": max}),
            )])
            .await;
        assert!(rules(&refused).contains(&"vlan.vid.invalid"), "{refused:?}");
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn domain_anchor_defaults_owner(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let acme = env.id("entity", "acme").await;
    let other = env.id("entity", "other").await;
    env.applied(vec![
        op(
            "create_group",
            json!({"slug": "lab-a", "anchor": {"domain": {"id": lab}}}),
        ),
        op(
            "create_group",
            json!({"slug": "lab-b", "anchor": {"domain": {"id": lab}}, "owner": {"id": other}}),
        ),
    ])
    .await;
    let a = env.id("vlan.vlan_group", "lab-a").await;
    let b = env.id("vlan.vlan_group", "lab-b").await;
    assert_eq!(env.owner(a).await, Some(acme), "the owner of the domain");
    assert_eq!(env.owner(b).await, Some(other), "the owner given");
    // In the domain for the access control.
    let domain: Option<Uuid> = sqlx::query_scalar(
        "SELECT domain_id FROM module_object WHERE id = $1 AND valid_period @> now()",
    )
    .bind(a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(domain, Some(lab));
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn slug_unique_over_time(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.id("entity", "acme").await;
    let group = |slug: &str| {
        op(
            "create_group",
            json!({"slug": slug, "anchor": {"entity": {"id": acme}}}),
        )
    };
    env.applied(vec![group("core-net")]).await;
    let refused = env.submit(vec![group("core-net")]).await;
    assert!(
        rules(&refused).contains(&"core.slug.duplicate"),
        "{refused:?}"
    );
    // Renamed, the slug is free again; a bad slug is refused.
    let id = env.id("vlan.vlan_group", "core-net").await;
    env.applied(vec![op(
        "update_group",
        json!({"id": {"id": id}, "slug": "backbone"}),
    )])
    .await;
    env.applied(vec![group("core-net")]).await;
    let refused = env.submit(vec![group("Core Net")]).await;
    assert!(rules(&refused).contains(&"core.slug.syntax"), "{refused:?}");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn range_cannot_exclude_existing_vlans(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.id("entity", "acme").await;
    env.applied(vec![op(
        "create_group",
        json!({"slug": "edge", "anchor": {"entity": {"id": acme}}}),
    )])
    .await;
    let group = env.id("vlan.vlan_group", "edge").await;
    // The VLAN 100 of the group (its operations come with task 008-11).
    sqlx::query(
        "INSERT INTO vlan.vlan (id, group_id, vid, name, status, valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1, 100, 'users', 'active',
                 tstzrange(now() - interval '1 hour', NULL), gen_random_uuid())",
    )
    .bind(group)
    .execute(&pool)
    .await
    .unwrap();
    let refused = env
        .submit(vec![op(
            "update_group",
            json!({"id": {"id": group}, "vid_min": 1, "vid_max": 99}),
        )])
        .await;
    assert!(
        rules(&refused).contains(&"vlan.vid.out_of_range"),
        "{refused:?}"
    );
    assert!(messages(&refused).contains("100 (users)"), "{refused:?}");
    // One bound only, above the VLAN: refused as well.
    let refused = env
        .submit(vec![op(
            "update_group",
            json!({"id": {"id": group}, "vid_min": 4000}),
        )])
        .await;
    assert!(
        rules(&refused).contains(&"vlan.vid.out_of_range"),
        "{refused:?}"
    );
    // A range that keeps it is accepted.
    env.applied(vec![op(
        "update_group",
        json!({"id": {"id": group}, "vid_min": 100, "vid_max": 199}),
    )])
    .await;
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn moving_the_anchor_reclassifies(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.id("entity", "acme").await;
    let other = env.id("entity", "other").await;
    let paris = env.id("site", "paris").await;
    env.applied(vec![op(
        "create_group",
        json!({"slug": "moving", "anchor": {"entity": {"id": acme}}}),
    )])
    .await;
    let group = env.id("vlan.vlan_group", "moving").await;
    let (on_acme, on_other) = (env.reader_on(acme).await, env.reader_on(other).await);
    assert!(env.reads(on_acme, group).await);
    assert!(!env.reads(on_other, group).await);
    // To the other entity: its owner follows, and the readers change.
    let moved = env
        .applied(vec![op(
            "update_group",
            json!({"id": {"id": group}, "anchor": {"entity": {"id": other}}}),
        )])
        .await;
    let Outcome::Applied { diff, .. } = moved else {
        unreachable!()
    };
    assert!(
        diff.iter().any(|d| d.property == "core.owner"),
        "the differential names the owner: {diff:?}"
    );
    assert_eq!(env.owner(group).await, Some(other));
    assert!(!env.reads(on_acme, group).await);
    assert!(env.reads(on_other, group).await);
    // To a site, then away from it: `core.site` follows the anchor.
    env.applied(vec![op(
        "update_group",
        json!({"id": {"id": group}, "anchor": {"site": {"id": paris}}, "owner": {"id": other}}),
    )])
    .await;
    let site = || async {
        sqlx::query_scalar::<_, String>(
            "SELECT a.v_uuid::text FROM property_assignment a JOIN property_declaration d
                 ON d.id = a.property_id AND d.key = 'core.site'
              WHERE a.object_id = $1 AND a.valid_period @> now()",
        )
        .bind(group)
        .fetch_optional(&pool)
        .await
        .unwrap()
    };
    assert_eq!(site().await, Some(paris.to_string()));
    env.applied(vec![op(
        "update_group",
        json!({"id": {"id": group}, "anchor": {"entity": {"id": acme}}}),
    )])
    .await;
    assert_eq!(site().await, None);
    assert_eq!(env.owner(group).await, Some(acme));
}
