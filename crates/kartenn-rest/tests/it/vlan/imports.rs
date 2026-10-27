//! The operations of the VLANs in the imports in batches (spec 026; spec 008, REQ-008-23):
//! an import of 10⁴ VLANs in 100 groups ends, and a replay after a stop duplicates nothing.

use super::env::{Env, op};
use kartenn_service::authz::Unrestricted;
use kartenn_service::import::{create_import, run_import};
use kartenn_service::port::ImportItem;
use serde_json::json;
use sqlx::PgPool;

const SOURCE: &str = "legacy-vlans";
const GROUPS: u32 = 100;
const VLANS: u32 = 100;

/// 100 groups anchored on `acme`, then 100 VLANs in each, given VLANs first: the import
/// orders them by the local references of their bodies.
fn items(acme: sqlx::types::Uuid) -> Vec<ImportItem> {
    let vlans = (0..GROUPS).flat_map(|g| {
        (1..=VLANS).map(move |vid| ImportItem {
            external_id: Some(format!("vlan-{g}-{vid}")),
            operation: op(
                "create_vlan",
                json!({"local": 1_000 + g * VLANS + vid, "group": {"local": g + 1},
                       "vid": vid, "name": format!("v{vid}")}),
            ),
        })
    });
    let groups = (0..GROUPS).map(|g| ImportItem {
        external_id: Some(format!("group-{g}")),
        operation: op(
            "create_vlan_group",
            json!({"local": g + 1, "slug": format!("g{g}"), "anchor": {"entity": {"id": acme}},
                   "vid_min": 1, "vid_max": 4094}),
        ),
    });
    vlans.chain(groups).collect()
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn import_ten_thousand_vlans_resumes_without_duplicates(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.id("entity", "acme").await;
    let import = create_import(&env.store, env.author, SOURCE, items(acme), 2_000)
        .await
        .unwrap();
    let done = run_import(&env.store, &Unrestricted, import, 2_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(done.state, "done", "{}", done.report);
    let groups = "SELECT count(*) FROM vlan_group";
    let vlans = "SELECT count(*) FROM vlan";
    let bound = "SELECT count(*) FROM external_id WHERE source = 'legacy-vlans'";
    let expected = i64::from(GROUPS * VLANS);
    assert_eq!(count(&pool, groups).await, i64::from(GROUPS));
    assert_eq!(count(&pool, vlans).await, expected);
    assert_eq!(count(&pool, bound).await, expected + i64::from(GROUPS));
    // Bound under the kinds of the core of their creations.
    let typed = "SELECT count(*) FROM external_id WHERE source = 'legacy-vlans'
                    AND object_type = CASE WHEN external_id LIKE 'vlan-%' THEN 'vlan'
                                           ELSE 'vlan_group' END";
    assert_eq!(count(&pool, typed).await, expected + i64::from(GROUPS));
    // Every VLAN is in the group its body named.
    let misplaced = "SELECT count(*) FROM vlan v
                      JOIN external_id e ON e.object_id = v.id
                      JOIN external_id g ON g.object_id = v.group_id AND g.source = e.source
                     WHERE split_part(e.external_id, '-', 2) <> split_part(g.external_id, '-', 2)";
    assert_eq!(count(&pool, misplaced).await, 0);

    // A stop after the batches were applied but before they were recorded: the replay
    // leaves out every creation already known to the source (REQ-004-29).
    sqlx::query("UPDATE import SET state = 'running' WHERE id = $1")
        .bind(import)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE import_batch SET state = 'queued', changeset_id = NULL WHERE import_id = $1",
    )
    .bind(import)
    .execute(&pool)
    .await
    .unwrap();
    let replayed = run_import(&env.store, &Unrestricted, import, 2_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replayed.state, "done", "{}", replayed.report);
    assert_eq!(count(&pool, groups).await, i64::from(GROUPS));
    assert_eq!(count(&pool, vlans).await, expected);
    assert_eq!(count(&pool, bound).await, expected + i64::from(GROUPS));
}
