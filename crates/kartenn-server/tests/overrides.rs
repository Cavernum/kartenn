//! Filters changed at run time: reserved to their administrators, audited, applied by
//! every instance, reverted at expiry (spec 012, REQ-012-27; task 012-17).

use std::time::Duration;

use kartenn_service::RequestError;
use kartenn_service::audit::{Instance, RequestContext};
use kartenn_service::authz::Grants;
use kartenn_service::observability::{OBSERVABILITY_ADMIN, Overrides, set_log_filter};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use kartenn_telemetry::capture::{CaptureLayer, FilterOverrides, Output};
use sqlx::PgPool;
use sqlx::types::Uuid;
use tracing_subscriber::layer::SubscriberExt;

fn valid(filter: &str) -> bool {
    tracing_subscriber::EnvFilter::try_new(filter).is_ok()
}

/// An instance: its output and the applier of the overrides.
struct Node {
    output: Output,
    receiver: tokio::sync::mpsc::Receiver<std::sync::Arc<kartenn_telemetry::capture::Record>>,
    filters: FilterOverrides,
    store: PgStore,
}

impl Node {
    fn new(pool: &PgPool) -> Self {
        let (output, receiver) = Output::new("stdout", "warn", 64).unwrap();
        Self {
            output,
            receiver,
            filters: FilterOverrides::new([("stdout".to_owned(), "warn".to_owned())]),
            store: PgStore::new(pool.clone()),
        }
    }

    async fn refresh(&mut self) -> Vec<String> {
        let pairs: Vec<(String, String)> = self
            .store
            .overrides()
            .await
            .unwrap()
            .into_iter()
            .map(|o| (o.output, o.filter))
            .collect();
        self.filters.apply(&[self.output.clone()], &pairs)
    }

    /// Whether an INFO event reaches the output.
    fn takes_info(&mut self) -> bool {
        let subscriber =
            tracing_subscriber::registry().with(CaptureLayer::new(vec![self.output.clone()]));
        tracing::subscriber::with_default(subscriber, || tracing::info!("probe"));
        self.receiver.try_recv().is_ok()
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_override_reaches_every_instance_and_expires(pool: PgPool) {
    let admin = Uuid::now_v7();
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("test"));
    let store = PgStore::new(pool.clone());
    let (mut one, mut two) = (Node::new(&pool), Node::new(&pool));
    assert!(!one.takes_info() && !two.takes_info());

    // Without the permission: refused and audited.
    let refused = set_log_filter(
        &store,
        &Grants::new(),
        &journal,
        &RequestContext::person(admin),
        "stdout",
        "debug",
        None,
        valid,
    )
    .await;
    assert!(matches!(refused, Err(RequestError::Forbidden(_))));
    let grants = Grants::new().everywhere(admin, OBSERVABILITY_ADMIN);
    // A filter that does not parse, or too long a duration: refused.
    for (filter, ttl) in [("x=y=z", None), ("debug", Some(Duration::from_hours(25)))] {
        assert!(matches!(
            set_log_filter(
                &store,
                &grants,
                &journal,
                &RequestContext::person(admin),
                "stdout",
                filter,
                ttl,
                valid
            )
            .await,
            Err(RequestError::InvalidOverride)
        ));
    }
    set_log_filter(
        &store,
        &grants,
        &journal,
        &RequestContext::person(admin),
        "stdout",
        "debug",
        Some(Duration::from_secs(1)),
        valid,
    )
    .await
    .unwrap();
    assert_eq!(one.refresh().await, ["stdout"]);
    assert_eq!(two.refresh().await, ["stdout"]);
    assert!(one.takes_info() && two.takes_info(), "on both instances");
    assert!(one.refresh().await.is_empty(), "applied once");

    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(one.refresh().await, ["stdout"], "reverted at expiry");
    two.refresh().await;
    assert!(!one.takes_info() && !two.takes_info());

    let names: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT name, details ->> 'filter' FROM audit.event ORDER BY seq")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        names,
        [
            ("authz.access.denied".to_owned(), None),
            (
                "sys.log_filter.changed".to_owned(),
                Some("debug".to_owned())
            ),
        ]
    );
}
