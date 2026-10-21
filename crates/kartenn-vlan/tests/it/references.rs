//! The references of the module (spec 008, REQ-008-10; task 008-13): a VLAN carried by a
//! network is not closed while the network keeps it, and the cascade removes the
//! assignment; a group is closed with its VLANs under the cascade only; the anchor of a
//! group is not closed while the group lives on it, cascade or not.

use crate::support::env;

use env::{Env, op, rules};
use kartenn_service::operations::RawInput;
use kartenn_service::{ObjectKind, Operation, Outcome, Ref};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

impl Env {
    /// The network `cidr` of the domain `lab`.
    async fn network(&self, cidr: &str) -> Uuid {
        let lab = self.id("addressing_domain", "lab").await;
        let create: Operation = serde_json::from_value(json!({
            "op": "create_prefix", "domain": {"id": lab}, "prefix": cidr, "nature": "network",
        }))
        .unwrap();
        self.applied(vec![create]).await;
        sqlx::query_scalar(
            "SELECT id FROM prefix WHERE prefix = $1::cidr AND valid_period @> now()",
        )
        .bind(cidr)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// A new group `slug` anchored on `anchor`, with the VLANs `vids`: the group, then the
    /// VLANs in the order of `vids`.
    async fn group_with(&self, slug: &str, anchor: Value, vids: &[i64]) -> (Uuid, Vec<Uuid>) {
        let mut operations = vec![op(
            "create_group",
            json!({"local": 1, "slug": slug, "anchor": anchor}),
        )];
        for vid in vids {
            operations.push(op(
                "create_vlan",
                json!({"group": {"local": 1}, "vid": vid, "name": format!("v{vid}")}),
            ));
        }
        self.applied(operations).await;
        let group: Uuid = sqlx::query_scalar(
            "SELECT id FROM vlan.vlan_group WHERE slug = $1 AND valid_period @> now()",
        )
        .bind(slug)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let mut vlans = Vec::new();
        for vid in vids {
            vlans.push(
                sqlx::query_scalar(
                    "SELECT id FROM vlan.vlan
                      WHERE group_id = $1 AND vid = $2 AND valid_period @> now()",
                )
                .bind(group)
                .bind(i16::try_from(*vid).unwrap())
                .fetch_one(&self.pool)
                .await
                .unwrap(),
            );
        }
        (group, vlans)
    }

    /// The network `network` carries the VLAN `vlan`.
    async fn carry(&self, network: Uuid, vlan: Uuid) {
        self.applied(vec![Operation::AssignProperty {
            object: Ref::Id(network),
            kind: ObjectKind::Prefix,
            property: Ref::Id(self.property("vlan.vlan").await),
            values: vec![RawInput::Text(vlan.to_string())],
        }])
        .await;
    }

    /// Whether `vlan.vlan` is assigned to `network` now.
    async fn carries(&self, network: Uuid) -> bool {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM property_assignment a
                              JOIN property_declaration d ON d.id = a.property_id
                             WHERE a.object_id = $1 AND d.key = 'vlan.vlan'
                               AND a.valid_period @> now())",
        )
        .bind(network)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// Whether the object `id` of the table `table` lives now.
    async fn lives(&self, table: &str, id: Uuid) -> bool {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT EXISTS (SELECT 1 FROM {table} WHERE id = $1 AND valid_period @> now())"
        )))
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn close(kind: &str, id: Uuid, cascade: bool) -> Operation {
    Operation::Close {
        kind: ObjectKind::from_name(kind).unwrap(),
        id: Ref::Id(id),
        expected: None,
        cascade,
    }
}

/// The subjects of the violations of `rule`.
fn subjects(outcome: &Outcome, rule: &str) -> Vec<Uuid> {
    match outcome {
        Outcome::Refused { violations, .. } => violations
            .iter()
            .filter(|v| v.rule.id == rule)
            .flat_map(|v| v.subjects.iter().copied())
            .collect(),
        other => panic!("not refused: {other:?}"),
    }
}

#[kartenn_testing::pg_test]
async fn closing_a_vlan_on_a_network_lists_it(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let (_, vlans) = env
        .group_with("edge", json!({"domain": {"id": lab}}), &[10])
        .await;
    let network = env.network("10.0.0.0/24").await;
    env.carry(network, vlans[0]).await;
    let refused = env.submit(vec![close("vlan.vlan", vlans[0], false)]).await;
    assert!(
        subjects(&refused, "prop.assignment.dangling_reference").contains(&network),
        "{refused:?}"
    );
    assert!(env.lives("vlan.vlan", vlans[0]).await);
    assert!(env.carries(network).await);
}

#[kartenn_testing::pg_test]
async fn cascade_unassigns_vlan_property(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let (_, vlans) = env
        .group_with("edge", json!({"domain": {"id": lab}}), &[10])
        .await;
    let network = env.network("10.0.0.0/24").await;
    env.carry(network, vlans[0]).await;
    let outcome = env.applied(vec![close("vlan.vlan", vlans[0], true)]).await;
    assert!(!env.lives("vlan.vlan", vlans[0]).await);
    assert!(!env.carries(network).await);
    // The differential shows the network losing its VLAN.
    let Outcome::Applied { diff, .. } = outcome else {
        unreachable!()
    };
    assert!(
        diff.iter()
            .any(|d| d.property == "vlan.vlan" && d.after.is_none() && d.count == 1),
        "{diff:?}"
    );
}

#[kartenn_testing::pg_test]
async fn closing_a_group_cascades_its_vlans(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let (group, vlans) = env
        .group_with("edge", json!({"domain": {"id": lab}}), &[10, 20])
        .await;
    let network = env.network("10.0.0.0/24").await;
    env.carry(network, vlans[1]).await;
    // Without the cascade: refused, the VLANs listed.
    let refused = env.submit(vec![close("vlan.group", group, false)]).await;
    let listed = subjects(&refused, "core.object.dependents");
    assert!(
        vlans.iter().all(|v| listed.contains(v)),
        "{listed:?} {refused:?}"
    );
    assert!(env.lives("vlan.vlan_group", group).await);
    // With it: the group, its VLANs and the VLAN of the network go.
    let applied = env.applied(vec![close("vlan.group", group, true)]).await;
    assert!(!env.lives("vlan.vlan_group", group).await);
    for vlan in &vlans {
        assert!(!env.lives("vlan.vlan", *vlan).await);
    }
    assert!(!env.carries(network).await);
    let Outcome::Applied { cascaded, .. } = applied else {
        unreachable!()
    };
    assert_eq!(cascaded.len(), 2, "{cascaded:?}");
}

#[kartenn_testing::pg_test]
async fn closing_an_anchor_entity_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.applied(vec![Operation::CreateEntity {
        local: Some(1),
        slug: "solo".into(),
        parent: None,
    }])
    .await;
    let solo = env.id("entity", "solo").await;
    let (group, _) = env
        .group_with("desk", json!({"entity": {"id": solo}}), &[])
        .await;
    // Cascade or not: change the anchor first. The group is listed, by the reference of
    // the module or by the anchor of the core it owns (`core.anchor.in_use`).
    for cascade in [false, true] {
        let refused = env.submit(vec![close("entity", solo, cascade)]).await;
        let listed: Vec<Uuid> = ["core.object.dependents", "core.anchor.in_use"]
            .iter()
            .flat_map(|rule| subjects(&refused, rule))
            .collect();
        assert!(listed.contains(&group), "{:?} {refused:?}", rules(&refused));
    }
    assert!(env.lives("entity", solo).await);
    assert!(env.lives("vlan.vlan_group", group).await);
    // A domain carries nothing of the group but its anchor: the reference of the module
    // alone refuses its closure.
    let lab = env.id("addressing_domain", "lab").await;
    let (anchored, _) = env
        .group_with("core", json!({"domain": {"id": lab}}), &[])
        .await;
    for cascade in [false, true] {
        let refused = env.submit(vec![close("domain", lab, cascade)]).await;
        assert!(
            subjects(&refused, "core.object.dependents").contains(&anchored),
            "{refused:?}"
        );
    }
    assert!(env.lives("addressing_domain", lab).await);
}
