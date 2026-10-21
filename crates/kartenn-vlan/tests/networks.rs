//! The VLAN of a network (spec 008, REQ-008-17, 18; task 008-12): `vlan.vlan`, a reference
//! carried by the networks only, IPv4 and IPv6 alike; a warning, then an anomaly, when the
//! group of the VLAN is anchored on a site outside that of the network or on another
//! domain.

#[path = "support/env.rs"]
mod env;

use env::{Env, op, rules};
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

impl Env {
    /// The prefix `cidr` of the domain `lab`, of nature `nature`.
    async fn prefix(&self, cidr: &str, nature: &str) -> Uuid {
        let lab = self.id("addressing_domain", "lab").await;
        self.applied(vec![core(json!({
            "op": "create_prefix", "domain": {"id": lab}, "prefix": cidr, "nature": nature,
        }))])
        .await;
        sqlx::query_scalar(
            "SELECT id FROM prefix WHERE prefix = $1::cidr AND valid_period @> now()",
        )
        .bind(cidr)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The VLAN `vid` of a new group `slug` anchored on `anchor` (`{"site": …}`…), owned
    /// by `acme`.
    async fn vlan_in(&self, slug: &str, anchor: Value, vid: i64) -> Uuid {
        let acme = self.id("entity", "acme").await;
        self.applied(vec![
            op(
                "create_group",
                json!({"local": 1, "slug": slug, "anchor": anchor, "owner": {"id": acme}}),
            ),
            op(
                "create_vlan",
                json!({"group": {"local": 1}, "vid": vid, "name": format!("v{vid}")}),
            ),
        ])
        .await;
        sqlx::query_scalar(
            "SELECT v.id FROM vlan.vlan v JOIN vlan.vlan_group g ON g.id = v.group_id
              WHERE g.slug = $1 AND v.vid = $2 AND v.valid_period @> now()",
        )
        .bind(slug)
        .bind(i16::try_from(vid).unwrap())
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The assignment of `key` to the prefix `prefix`, of the object `value`.
    async fn assign(&self, prefix: Uuid, key: &str, value: Uuid) -> Operation {
        Operation::AssignProperty {
            object: Ref::Id(prefix),
            kind: ObjectKind::Prefix,
            property: Ref::Id(self.property(key).await),
            values: vec![RawInput::Text(value.to_string())],
        }
    }

    /// Whether an anomaly of `rule` is open on `object`.
    async fn anomaly(&self, object: Uuid, rule: &str) -> bool {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM anomaly
                             WHERE object_id = $1 AND rule_id = $2 AND closed_at IS NULL)",
        )
        .bind(object)
        .bind(rule)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn warnings(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Applied { warnings, .. } => warnings.iter().map(|v| v.rule.id).collect(),
        Outcome::Simulated { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        other => panic!("neither applied nor simulated: {other:?}"),
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn vlan_on_container_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let vlan = env
        .vlan_in("edge", json!({"domain": {"id": lab}}), 10)
        .await;
    let container = env.prefix("10.0.0.0/16", "container").await;
    let refused = env
        .submit(vec![env.assign(container, "vlan.vlan", vlan).await])
        .await;
    assert!(
        rules(&refused).contains(&"vlan.prefix.not_network"),
        "{refused:?}"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn network_to_container_with_vlan_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let vlan = env
        .vlan_in("edge", json!({"domain": {"id": lab}}), 10)
        .await;
    let network = env.prefix("10.1.0.0/24", "network").await;
    env.applied(vec![env.assign(network, "vlan.vlan", vlan).await])
        .await;
    let refused = env
        .submit(vec![core(json!({
            "op": "update_prefix", "id": {"id": network}, "nature": "container",
        }))])
        .await;
    assert!(
        rules(&refused).contains(&"vlan.prefix.not_network"),
        "{refused:?}"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn site_mismatch_is_an_anomaly(pool: PgPool) {
    let env = Env::new(&pool).await;
    let paris = env.id("site", "paris").await;
    env.applied(vec![core(json!({"op": "create_site", "slug": "lyon"}))])
        .await;
    let lyon = env.id("site", "lyon").await;
    let vlan = env
        .vlan_in("dc-paris", json!({"site": {"id": paris}}), 10)
        .await;
    // A network of Lyon on a VLAN of Paris: a warning at the simulation…
    let network = env.prefix("10.2.0.0/24", "network").await;
    let operations = vec![
        env.assign(network, "core.site", lyon).await,
        env.assign(network, "vlan.vlan", vlan).await,
    ];
    let mut request = env.request(operations.clone(), None);
    request.simulate = true;
    let simulated = submit(&env.store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(
        warnings(&simulated).contains(&"vlan.site.mismatch"),
        "{simulated:?}"
    );
    // … an anomaly once applied.
    let applied = env.applied(operations).await;
    assert!(
        warnings(&applied).contains(&"vlan.site.mismatch"),
        "{applied:?}"
    );
    assert!(env.anomaly(network, "vlan.site.mismatch").await);
    // A network of Paris on the same VLAN: nothing.
    let same = env.prefix("10.4.0.0/24", "network").await;
    let applied = env
        .applied(vec![
            env.assign(same, "core.site", paris).await,
            env.assign(same, "vlan.vlan", vlan).await,
        ])
        .await;
    assert!(warnings(&applied).is_empty(), "{applied:?}");
    // A group anchored on another domain: the domain does not match either.
    let owner = env.property("core.owner").await;
    let acme = env.id("entity", "acme").await;
    env.applied(vec![
        core(json!({"op": "create_domain", "local": 1, "slug": "other-lab"})),
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Domain,
            property: Ref::Id(owner),
            values: vec![RawInput::Text(acme.to_string())],
        },
    ])
    .await;
    let other_lab = env.id("addressing_domain", "other-lab").await;
    let foreign = env
        .vlan_in("other", json!({"domain": {"id": other_lab}}), 20)
        .await;
    let network = env.prefix("10.5.0.0/24", "network").await;
    let applied = env
        .applied(vec![env.assign(network, "vlan.vlan", foreign).await])
        .await;
    assert!(
        warnings(&applied).contains(&"vlan.domain.mismatch"),
        "{applied:?}"
    );
    assert!(env.anomaly(network, "vlan.domain.mismatch").await);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn site_mismatch_anomaly_closes_once_corrected(pool: PgPool) {
    let env = Env::new(&pool).await;
    let paris = env.id("site", "paris").await;
    env.applied(vec![core(json!({"op": "create_site", "slug": "lyon"}))])
        .await;
    let lyon = env.id("site", "lyon").await;
    let vlan = env
        .vlan_in("dc-paris", json!({"site": {"id": paris}}), 10)
        .await;
    let network = env.prefix("10.2.0.0/24", "network").await;
    env.applied(vec![
        env.assign(network, "core.site", lyon).await,
        env.assign(network, "vlan.vlan", vlan).await,
    ])
    .await;
    assert!(env.anomaly(network, "vlan.site.mismatch").await);
    // The network moves to Paris, a change of the core alone: the anomaly closes.
    let applied = env
        .applied(vec![env.assign(network, "core.site", paris).await])
        .await;
    assert!(warnings(&applied).is_empty(), "{applied:?}");
    assert!(!env.anomaly(network, "vlan.site.mismatch").await);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn v4_and_v6_networks_on_one_vlan(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let vlan = env
        .vlan_in("edge", json!({"domain": {"id": lab}}), 10)
        .await;
    let v4 = env.prefix("10.3.0.0/24", "network").await;
    let v6 = env.prefix("2001:db8:3::/64", "network").await;
    let applied = env
        .applied(vec![
            env.assign(v4, "vlan.vlan", vlan).await,
            env.assign(v6, "vlan.vlan", vlan).await,
        ])
        .await;
    assert!(warnings(&applied).is_empty(), "{applied:?}");
    let carried: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM property_assignment a
           JOIN property_declaration d ON d.id = a.property_id AND d.key = 'vlan.vlan'
          WHERE a.v_uuid = $1 AND a.valid_period @> now()",
    )
    .bind(vlan)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(carried, 2);
}
