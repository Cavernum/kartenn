//! Ranges through the changeset pipeline (spec 002, REQ-002-21 to 24, 28; task 002-20).

use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
use kartenn_core::rules::Severity;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

struct Fixture {
    store: PgStore,
    author: Uuid,
    domain: Uuid,
}

impl Fixture {
    /// Domain `prod` with the networks `10.1.2.0/24`, `10.1.3.0/24` and `2001:db8:1::/64`.
    async fn new(pool: &PgPool) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'tester') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let mut f = Self {
            store: PgStore::new(pool.clone()),
            author,
            domain: Uuid::nil(),
        };
        f.applied(vec![Operation::CreateDomain {
            local: None,
            slug: "prod".into(),
        }])
        .await;
        f.domain = sqlx::query_scalar("SELECT id FROM addressing_domain")
            .fetch_one(pool)
            .await
            .unwrap();
        f.applied(vec![
            f.network("10.1.2.0/24"),
            f.network("10.1.3.0/24"),
            f.network("2001:db8:1::/64"),
        ])
        .await;
        f
    }

    fn network(&self, value: &str) -> Operation {
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Id(self.domain),
            prefix: value.into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        }
    }

    fn range(&self, first: &str, last: &str, usage: RangeUsage) -> Operation {
        Operation::CreateRange {
            local: None,
            domain: Ref::Id(self.domain),
            first: first.into(),
            last: last.into(),
            usage,
        }
    }

    fn address(&self, value: &str, usage: AddressUsage) -> Operation {
        Operation::CreateAddress {
            local: None,
            domain: Ref::Id(self.domain),
            address: value.into(),
            usage,
        }
    }

    async fn submit(&self, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit(&self.store, &request, MAX_OPERATIONS).await.unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>) {
        let outcome = self.submit(operations).await;
        assert!(
            matches!(outcome, Outcome::Applied { .. }),
            "refused: {outcome:?}"
        );
    }

    /// Error rules of a refused changeset (warnings left out).
    async fn refused(&self, operations: Vec<Operation>) -> Vec<&'static str> {
        match self.submit(operations).await {
            Outcome::Refused { violations, .. } => violations
                .iter()
                .filter(|v| v.rule.severity == Severity::Error)
                .map(|v| v.rule.id)
                .collect(),
            other => panic!("expected a refusal: {other:?}"),
        }
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_range_lies_in_a_single_network(pool: PgPool) {
    // REQ-002-21 acceptance.
    let f = Fixture::new(&pool).await;
    f.applied(vec![f.range("10.1.2.10", "10.1.2.50", RangeUsage::Dhcp)])
        .await;
    assert_eq!(
        f.refused(vec![f.range("10.1.2.200", "10.1.3.20", RangeUsage::Static)])
            .await,
        vec!["core.range.outside_network"],
        "across two networks"
    );
    assert_eq!(
        f.refused(vec![f.range("10.9.0.10", "10.9.0.20", RangeUsage::Static)])
            .await,
        vec!["core.range.outside_network"],
        "outside any network"
    );
    assert_eq!(
        f.refused(vec![f.range("10.1.2.50", "10.1.2.10", RangeUsage::Static)])
            .await,
        vec!["core.range.bounds"]
    );
    assert_eq!(
        f.refused(vec![f.range(
            "10.1.2.10",
            "2001:db8:1::10",
            RangeUsage::Static
        )])
        .await,
        vec!["core.range.bounds"]
    );
    assert_eq!(
        f.refused(vec![f.range("10.1.2.010", "10.1.2.20", RangeUsage::Static)])
            .await,
        vec!["core.addr.syntax"]
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn ranges_are_disjoint_and_avoid_reserved_positions(pool: PgPool) {
    // REQ-002-22, 23 acceptance.
    let f = Fixture::new(&pool).await;
    f.applied(vec![f.range("10.1.2.10", "10.1.2.50", RangeUsage::Dhcp)])
        .await;
    assert_eq!(
        f.refused(vec![f.range("10.1.2.40", "10.1.2.60", RangeUsage::Static)])
            .await,
        vec!["core.range.overlap"]
    );
    f.applied(vec![f.range("10.1.2.51", "10.1.2.60", RangeUsage::Static)])
        .await;
    for (first, last) in [("10.1.2.0", "10.1.2.5"), ("10.1.2.200", "10.1.2.255")] {
        assert_eq!(
            f.refused(vec![f.range(first, last, RangeUsage::Reserved)])
                .await,
            vec!["core.range.reserved_bounds"],
            "{first}–{last}"
        );
    }
    assert_eq!(
        f.refused(vec![f.range(
            "2001:db8:1::",
            "2001:db8:1::ff",
            RangeUsage::Dhcp
        )])
        .await,
        vec!["core.range.reserved_bounds"],
        "zero interface identifier"
    );
    f.applied(vec![f.range(
        "2001:db8:1::100",
        "2001:db8:1::1ff",
        RangeUsage::Dhcp,
    )])
    .await;
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn addresses_respect_the_usage_of_their_range(pool: PgPool) {
    // REQ-002-24, 28 acceptance, in both orders of creation.
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        f.range("10.1.2.100", "10.1.2.199", RangeUsage::Dhcp),
        f.range("10.1.2.1", "10.1.2.9", RangeUsage::Reserved),
        f.range("10.1.2.10", "10.1.2.19", RangeUsage::Static),
    ])
    .await;
    assert_eq!(
        f.refused(vec![f.address("10.1.2.150", AddressUsage::Host)])
            .await,
        vec!["core.address.range_usage"]
    );
    f.applied(vec![f.address("10.1.2.150", AddressUsage::Reserved)])
        .await;
    assert_eq!(
        f.refused(vec![f.address("10.1.2.5", AddressUsage::Reserved)])
            .await,
        vec!["core.address.range_usage"]
    );
    f.applied(vec![f.address("10.1.2.15", AddressUsage::Gateway)])
        .await;
    // A range created over existing addresses checks them too.
    f.applied(vec![f.address("10.1.3.60", AddressUsage::Host)])
        .await;
    assert_eq!(
        f.refused(vec![f.range("10.1.3.50", "10.1.3.70", RangeUsage::Dhcp)])
            .await,
        vec!["core.address.range_usage"]
    );
    // Same changeset: the final state is checked.
    assert_eq!(
        f.refused(vec![
            f.range("10.1.3.80", "10.1.3.90", RangeUsage::Reserved),
            f.address("10.1.3.85", AddressUsage::Host),
        ])
        .await,
        vec!["core.address.range_usage", "core.address.range_usage"]
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn closing_a_network_needs_its_ranges_closed(pool: PgPool) {
    // REQ-002-21 over time; closing a range (REQ-004-15).
    let f = Fixture::new(&pool).await;
    f.applied(vec![f.range("10.1.3.10", "10.1.3.20", RangeUsage::Dhcp)])
        .await;
    let (net, range): (Uuid, Uuid) = sqlx::query_as(
        "SELECT p.id, r.id FROM prefix p, ip_range r WHERE p.prefix = '10.1.3.0/24'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let close = |kind, id| Operation::Close {
        kind,
        id: Ref::Id(id),
        expected: None,
    };
    assert_eq!(
        f.refused(vec![close(ObjectKind::Prefix, net)]).await,
        vec!["core.range.outside_network"]
    );
    f.applied(vec![
        close(ObjectKind::Range, range),
        close(ObjectKind::Prefix, net),
    ])
    .await;
}
