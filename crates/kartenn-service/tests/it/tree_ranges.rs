//! Ranges in navigation, occupancy and next free address (REQ-002-14, 42 to 44).

use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::family::ALL;
use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
use kartenn_core::occupancy::Occupancy;
use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::tree::{Node, ancestors, children, next_free_address, occupancy};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

fn a(t: &str) -> Addr {
    Addr::parse_any(&ALL, t).unwrap()
}

#[kartenn_testing::pg_test]
async fn ranges_are_children_of_their_network_and_count_in_occupancy(pool: PgPool) {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let store = PgStore::new(pool.clone());
    let pre = |v: &str, nature| Operation::CreatePrefix {
        local: None,
        domain: Ref::Local(1),
        prefix: v.into(),
        nature,
        point_to_point: false,
        anycast: false,
    };
    let range = |first: &str, last: &str, usage| Operation::CreateRange {
        local: None,
        domain: Ref::Local(1),
        first: first.into(),
        last: last.into(),
        usage,
    };
    let addr = |v: &str, usage| Operation::CreateAddress {
        local: None,
        domain: Ref::Local(1),
        address: v.into(),
        usage,
    };
    let operations = vec![
        Operation::CreateDomain {
            local: Some(1),
            slug: "prod".into(),
        },
        pre("10.1.0.0/16", Nature::Container),
        pre("10.1.2.0/24", Nature::Network),
        range("10.1.2.1", "10.1.2.9", RangeUsage::Reserved),
        range("10.1.2.10", "10.1.2.19", RangeUsage::Static),
        range("10.1.2.100", "10.1.2.199", RangeUsage::Dhcp),
        addr("10.1.2.10", AddressUsage::Host),
        addr("10.1.2.20", AddressUsage::Host),
        addr("10.1.2.150", AddressUsage::Reserved),
    ];
    let request = ChangesetRequest {
        author,
        motive: "m".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
        apply_own: true,
    };
    let outcome = submit(&store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let now = OffsetDateTime::now_utc();
    let (container, net): (Uuid, Uuid) = sqlx::query_as(
        "SELECT c.id, n.id FROM prefix c, prefix n
         WHERE c.prefix = '10.1.0.0/16' AND n.prefix = '10.1.2.0/24'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // REQ-002-42: a network's direct content, in address order, range before the address
    // that starts it.
    let shape: Vec<String> = children(&pool, net, now)
        .await
        .unwrap()
        .into_iter()
        .map(|n| match n {
            Node::Range { first, last, .. } => format!("{first}-{last}"),
            Node::Address { value, .. } => value.to_string(),
            Node::Prefix { value, .. } => value.to_string(),
        })
        .collect();
    assert_eq!(
        shape,
        vec![
            "10.1.2.1-10.1.2.9",
            "10.1.2.10-10.1.2.19",
            "10.1.2.10",
            "10.1.2.20",
            "10.1.2.100-10.1.2.199",
            "10.1.2.150",
        ]
    );
    assert_eq!(
        children(&pool, container, now).await.unwrap().len(),
        1,
        "ranges are not children of the container"
    );

    // REQ-002-14: the parent of a range is its network.
    let range_id: Uuid = sqlx::query_scalar("SELECT id FROM ip_range WHERE usage = 'dhcp'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let chain: Vec<Prefix> = ancestors(&pool, range_id, now)
        .await
        .unwrap()
        .into_iter()
        .map(|n| match n {
            Node::Prefix { value, .. } => value,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        chain,
        vec![
            Prefix::parse_any(&ALL, "10.1.2.0/24").unwrap(),
            Prefix::parse_any(&ALL, "10.1.0.0/16").unwrap()
        ]
    );

    // REQ-002-43: DHCP and reserved ranges in full (100 + 9), plus the addresses outside
    // them (.10 in the static range, .20); .150 is inside the DHCP range, counted once.
    assert_eq!(
        occupancy(&pool, net, now).await.unwrap(),
        Some(Occupancy {
            used: 111,
            total: 254
        })
    );
    // REQ-002-44: .0 is reserved, .1–.9 a reserved range, .10 declared; a static range
    // does not block.
    assert_eq!(
        next_free_address(&pool, net, now).await.unwrap(),
        Some(a("10.1.2.11"))
    );
}
