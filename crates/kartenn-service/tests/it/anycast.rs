//! Public space and anycast across domains (spec 002, REQ-002-32, 46, 49; task 002-21).

use std::time::Duration;

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

/// An arbitrary public block: neither documentation nor any special-purpose entry.
const N: &str = "45.0.0.0/24";

struct Fixture {
    store: PgStore,
    author: Uuid,
    a: Uuid,
    b: Uuid,
}

impl Fixture {
    /// Domains `a` and `b`, empty.
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
            a: Uuid::nil(),
            b: Uuid::nil(),
        };
        f.applied(vec![
            Operation::CreateDomain {
                local: None,
                slug: "a".into(),
            },
            Operation::CreateDomain {
                local: None,
                slug: "b".into(),
            },
        ])
        .await;
        let id = |slug: &'static str| {
            sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = $1")
                .bind(slug)
                .fetch_one(pool)
        };
        f.a = id("a").await.unwrap();
        f.b = id("b").await.unwrap();
        f
    }

    fn prefix(domain: Uuid, value: &str, nature: Nature, anycast: bool) -> Operation {
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Id(domain),
            prefix: value.into(),
            nature,
            point_to_point: false,
            anycast,
        }
    }

    fn network(domain: Uuid, value: &str, anycast: bool) -> Operation {
        Self::prefix(domain, value, Nature::Network, anycast)
    }

    fn address(domain: Uuid, value: &str, usage: AddressUsage) -> Operation {
        Operation::CreateAddress {
            local: None,
            domain: Ref::Id(domain),
            address: value.into(),
            usage,
        }
    }

    fn request(&self, operations: Vec<Operation>) -> ChangesetRequest {
        ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        }
    }

    async fn submit(&self, operations: Vec<Operation>) -> Outcome {
        submit(
            &self.store,
            &Unrestricted,
            &self.request(operations),
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
    }

    /// Applies and returns the identifiers of the warnings.
    async fn applied(&self, operations: Vec<Operation>) -> Vec<&'static str> {
        match self.submit(operations).await {
            Outcome::Applied { warnings, .. } => warnings.iter().map(|v| v.rule.id).collect(),
            other => panic!("expected applied, got {other:?}"),
        }
    }

    /// Expects a refusal and returns the identifiers of the violations.
    async fn refused(&self, operations: Vec<Operation>) -> Vec<&'static str> {
        match self.submit(operations).await {
            Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
            other => panic!("expected refused, got {other:?}"),
        }
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_anycast_network_and_its_addresses_may_be_in_several_domains(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // REQ-002-46: the same network, anycast everywhere → accepted with a warning.
    assert!(
        f.applied(vec![Fixture::network(f.a, N, true)])
            .await
            .is_empty()
    );
    assert_eq!(
        f.applied(vec![Fixture::network(f.b, N, true)]).await,
        vec!["core.public.anycast_shared"]
    );
    // Its anycast addresses too (A11, now reachable).
    f.applied(vec![Fixture::address(
        f.a,
        "45.0.0.53",
        AddressUsage::Anycast,
    )])
    .await;
    assert_eq!(
        f.applied(vec![Fixture::address(
            f.b,
            "45.0.0.53",
            AddressUsage::Anycast
        )])
        .await,
        vec!["core.public.anycast_shared"]
    );
    // A public container of `b` around N stays out of uniqueness (A3).
    assert!(
        f.applied(vec![Fixture::prefix(
            f.b,
            "45.0.0.0/22",
            Nature::Container,
            false
        )])
        .await
        .is_empty()
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn every_occurrence_must_be_the_same_anycast_network(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![Fixture::network(f.a, N, true)]).await;
    // Not anycast in `b` → refused.
    assert!(
        f.refused(vec![Fixture::network(f.b, N, false)])
            .await
            .contains(&"core.public.overlap")
    );
    // Anycast but another value → refused.
    assert!(
        f.refused(vec![Fixture::network(f.b, "45.0.0.0/25", true)])
            .await
            .contains(&"core.public.overlap")
    );
    // The other order: a plain public network first, then anycast elsewhere.
    f.applied(vec![Fixture::network(f.a, "45.0.1.0/24", false)])
        .await;
    assert!(
        f.refused(vec![Fixture::network(f.b, "45.0.1.0/24", true)])
            .await
            .contains(&"core.public.overlap")
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_anycast_network_holds_anycast_addresses_only(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // REQ-002-49: only networks carry the mark.
    assert_eq!(
        f.refused(vec![Fixture::prefix(
            f.a,
            "45.0.0.0/22",
            Nature::Container,
            true
        )])
        .await,
        vec!["core.prefix.anycast_network"]
    );
    f.applied(vec![
        Fixture::network(f.a, N, true),
        Fixture::network(f.a, "45.0.1.0/24", false),
    ])
    .await;
    assert_eq!(
        f.refused(vec![Fixture::address(f.a, "45.0.0.1", AddressUsage::Host)])
            .await,
        vec!["core.anycast.address_usage"]
    );
    // An anycast address in a plain network stays accepted (A11).
    f.applied(vec![Fixture::address(
        f.a,
        "45.0.1.1",
        AddressUsage::Anycast,
    )])
    .await;
    // Whichever comes first: network and host address in the same changeset.
    assert!(
        f.refused(vec![
            Fixture::network(f.b, "10.9.0.0/24", true),
            Fixture::address(f.b, "10.9.0.1", AddressUsage::Host),
        ])
        .await
        .contains(&"core.anycast.address_usage")
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn concurrent_occurrences_are_serialised_by_the_public_space_lock(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // Hold the public space lock (same key as the store, ADR 0015) so that both
    // submissions queue behind it, then release them together.
    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(0x6b61_7274_656e_6e50_i64)
        .execute(&mut *holder)
        .await
        .unwrap();
    let spawn = |operations| {
        let store = f.store.clone();
        let request = f.request(operations);
        tokio::spawn(async move {
            submit(&store, &Unrestricted, &request, MAX_OPERATIONS)
                .await
                .unwrap()
        })
    };
    let first = spawn(vec![Fixture::network(f.a, N, true)]);
    let second = spawn(vec![Fixture::network(f.b, N, false)]);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!first.is_finished() && !second.is_finished());
    holder.rollback().await.unwrap();
    let outcomes = [first.await.unwrap(), second.await.unwrap()];
    let applied = outcomes
        .iter()
        .filter(|o| matches!(o, Outcome::Applied { .. }))
        .count();
    assert_eq!(applied, 1, "{outcomes:?}");
}
