//! Events of modules (spec 016, REQ-016-15): emitted in the transaction of a changeset or of
//! a background task, delivered as those of the core, and only if the transaction ends
//! well.

use std::sync::Arc;

use kartenn_service::authz::Unrestricted;
use kartenn_service::module::{Module, ModuleRegistry, RegistryError};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, submit};
use kartenn_store_pg::outbox::{Event, relay};
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::support::dummy_module as dummy;

async fn person(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'tester') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

fn request(author: Uuid, operations: Vec<Operation>, simulate: bool) -> ChangesetRequest {
    ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations,
        simulate,
        idempotency_key: None,
        approval_self: true,
    }
}

async fn drain(pool: &PgPool) -> Vec<Event> {
    let mut got = Vec::new();
    relay(pool, "events", 100, |e| {
        got.push(e);
        async { Ok::<(), std::convert::Infallible>(()) }
    })
    .await
    .unwrap();
    got
}

fn types(events: &[Event]) -> Vec<&str> {
    events
        .iter()
        .map(|e| e.payload["type"].as_str().unwrap())
        .collect()
}

#[kartenn_testing::pg_test]
async fn a_module_event_is_delivered_only_if_applied(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let emits = || dummy::create_thing(json!({"label": "emits", "keys": ["dummy:things"]}));
    // A simulation emits nothing.
    let simulated = submit(
        &store,
        &Unrestricted,
        &request(author, vec![emits()], true),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(
        matches!(simulated, Outcome::Simulated { .. }),
        "{simulated:?}"
    );
    // A refused changeset emits nothing: its other thing is forbidden.
    let refused = submit(
        &store,
        &Unrestricted,
        &request(
            author,
            vec![emits(), dummy::create_thing(json!({"label": "forbidden"}))],
            false,
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(refused, Outcome::Refused { .. }), "{refused:?}");
    assert!(drain(&pool).await.is_empty(), "nothing emitted yet");
    // Applied: the changeset's event, then the module's, under the same keys.
    let Outcome::Applied { changeset, .. } = submit(
        &store,
        &Unrestricted,
        &request(author, vec![emits()], false),
        MAX_OPERATIONS,
    )
    .await
    .unwrap() else {
        panic!()
    };
    let events = drain(&pool).await;
    assert_eq!(
        types(&events),
        ["org.kartenn.changeset.applied", "org.kartenn.dummy.created"]
    );
    let event = &events[1];
    assert_eq!(event.changeset, Some(changeset));
    assert_eq!(event.order_keys, ["dummy:things"]);
    assert_eq!(event.payload["data"], json!({"label": "emits"}));
    assert_eq!(event.payload["specversion"], "1.0");
    assert!(event.payload["subject"].is_string());
    assert_ne!(
        event.payload["id"], events[0].payload["id"],
        "each event has its own id"
    );
}

/// A module whose events are misnamed.
#[derive(Debug)]
struct Misnamed;

impl Module for Misnamed {
    fn name(&self) -> &'static str {
        "misnamed"
    }
    fn version(&self) -> &'static str {
        "0.0.0"
    }
    fn events(&self) -> &'static [&'static str] {
        &["Revised"]
    }
}

#[kartenn_testing::pg_test]
async fn an_undeclared_event_fails_the_registry(pool: PgPool) {
    // At startup: a misnamed event stops the build.
    assert_eq!(
        ModuleRegistry::new(vec![Arc::new(Misnamed)]).map(|_| ()),
        Err(RegistryError::InvalidEvent {
            module: "misnamed".into(),
            name: "Revised".into()
        })
    );
    // At run time: an event no module declares fails the changeset, which writes nothing.
    let store = dummy::store(&pool).await;
    let author = person(&pool).await;
    let outcome = submit(
        &store,
        &Unrestricted,
        &request(
            author,
            vec![dummy::create_thing(json!({"label": "emits-undeclared"}))],
            false,
        ),
        MAX_OPERATIONS,
    )
    .await;
    assert!(outcome.is_err(), "{outcome:?}");
    let things: i64 = sqlx::query_scalar("SELECT count(*) FROM dummy.thing")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(things, 0);
    assert!(drain(&pool).await.is_empty());
}

#[kartenn_testing::pg_test]
async fn a_background_task_emits_its_events(pool: PgPool) {
    let store = dummy::store(&pool).await;
    assert!(
        kartenn_service::module::run_job(&store, "dummy.tick")
            .await
            .unwrap()
    );
    assert!(
        !kartenn_service::module::run_job(&store, "dummy.unknown")
            .await
            .unwrap(),
        "not a task of the module"
    );
    let events = drain(&pool).await;
    assert_eq!(types(&events), ["org.kartenn.dummy.created"]);
    let event = &events[0];
    assert_eq!(event.changeset, None, "no changeset");
    assert_eq!(event.order_keys, ["dummy:tick"]);
    assert_eq!(event.payload["data"], json!({"job": "tick"}));
    assert!(event.payload.get("subject").is_none());
}
