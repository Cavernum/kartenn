//! Navigation, occupancy and next free values in the database (REQ-002-14, 42 to 44).

use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::family::ALL;
use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::occupancy::Occupancy;
use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::tree::{
    Node, ancestors, children, next_free_address, next_free_prefix, occupancy,
};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

fn p(t: &str) -> Prefix {
    Prefix::parse_any(&ALL, t).unwrap()
}

async fn id_of(pool: &PgPool, value: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = $1::cidr")
        .bind(value)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn the_tree_is_deduced_and_measured(pool: PgPool) {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let store = PgStore::new(pool.clone());
    let pre = |v: &str, n| Operation::CreatePrefix {
        local: None,
        domain: Ref::Local(1),
        prefix: v.into(),
        nature: n,
        point_to_point: false,
        anycast: false,
    };
    let addr = |v: &str| Operation::CreateAddress {
        local: None,
        domain: Ref::Local(1),
        address: v.into(),
        usage: AddressUsage::Host,
    };
    let ops = vec![
        Operation::CreateDomain {
            local: Some(1),
            slug: "prod".into(),
        },
        pre("10.0.0.0/8", Nature::Container),
        pre("10.1.0.0/16", Nature::Container),
        pre("10.1.0.0/24", Nature::Network),
        pre("10.1.2.0/24", Nature::Network),
        addr("10.1.2.1"),
        addr("10.1.2.2"),
        addr("10.1.2.4"),
    ];
    let req = ChangesetRequest {
        author,
        motive: "m".into(),
        period: None,
        operations: ops,
        simulate: false,
        idempotency_key: None,
    };
    let outcome = submit(&store, &Unrestricted, &req, MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let now = OffsetDateTime::now_utc();
    let (root, mid, net) = (
        id_of(&pool, "10.0.0.0/8").await,
        id_of(&pool, "10.1.0.0/16").await,
        id_of(&pool, "10.1.2.0/24").await,
    );

    let kids = children(&pool, root, now).await.unwrap();
    assert_eq!(kids.len(), 1, "the /8 has one direct child, the /16");
    let kids = children(&pool, mid, now).await.unwrap();
    assert_eq!(
        kids.iter()
            .filter(|n| matches!(n, Node::Prefix { .. }))
            .count(),
        2
    );
    assert_eq!(
        children(&pool, net, now).await.unwrap().len(),
        3,
        "three addresses"
    );

    let a: Uuid = sqlx::query_scalar("SELECT id FROM address WHERE address = '10.1.2.4'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let chain: Vec<Prefix> = ancestors(&pool, a, now)
        .await
        .unwrap()
        .into_iter()
        .map(|n| {
            if let Node::Prefix { value, .. } = n {
                value
            } else {
                panic!()
            }
        })
        .collect();
    assert_eq!(
        chain,
        vec![p("10.1.2.0/24"), p("10.1.0.0/16"), p("10.0.0.0/8")]
    );

    assert_eq!(
        occupancy(&pool, net, now).await.unwrap(),
        Some(Occupancy {
            used: 3,
            total: 254
        })
    );
    assert_eq!(
        occupancy(&pool, mid, now).await.unwrap(),
        Some(Occupancy {
            used: 512,
            total: 65_536
        })
    );
    assert_eq!(
        next_free_address(&pool, net, now).await.unwrap(),
        Some(Addr::parse_any(&ALL, "10.1.2.3").unwrap())
    );
    assert_eq!(
        next_free_prefix(&pool, mid, 24, now).await.unwrap(),
        Some(p("10.1.1.0/24"))
    );
    assert_eq!(
        next_free_prefix(&pool, net, 25, now).await.unwrap(),
        None,
        "a network holds no prefix"
    );
    let before = OffsetDateTime::from_unix_timestamp(0).unwrap();
    assert_eq!(
        occupancy(&pool, net, before).await.unwrap(),
        None,
        "did not exist then"
    );
}
