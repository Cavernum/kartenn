//! Each event of the catalogue, stored then exported, validates the published JSON Schema
//! (spec 012, REQ-012-10); an event that breaks it is refused by the schema.

use kartenn_core::audit::{EVENTS, Outcome, event_schema};
use kartenn_service::audit::{AuditJournal, Instance, RequestContext};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use sqlx::PgPool;
use sqlx::types::Uuid;

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn every_event_of_the_catalogue_validates_the_schema(pool: PgPool) {
    let schema = jsonschema::draft202012::new(&event_schema()).expect("a valid schema");
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("test"));
    let person = RequestContext::person(Uuid::now_v7());
    for kind in EVENTS {
        let mut event = person
            .event(kind, Outcome::Success)
            .unwrap()
            .target("object", [Uuid::now_v7()]);
        for detail in kind.details {
            event = event.detail(detail, "x").unwrap();
        }
        journal.record(&event).await.unwrap();
    }
    let rows: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT to_jsonb(e) FROM audit.event e ORDER BY seq")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), EVENTS.len());
    for row in &rows {
        let errors: Vec<String> = schema.iter_errors(row).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{}: {errors:?}", row["name"]);
    }

    // The schema refuses what the catalogue does not allow.
    let mut wrong = rows[0].clone();
    wrong["severity"] = "ERROR".into();
    assert!(
        !schema.is_valid(&wrong),
        "a severity other than the catalogue's"
    );
    let mut wrong = rows[0].clone();
    wrong["details"]["undeclared"] = "x".into();
    assert!(!schema.is_valid(&wrong), "an undeclared detail");
    let mut wrong = rows[0].clone();
    wrong["actor_id"] = serde_json::Value::Null;
    assert!(
        !schema.is_valid(&wrong),
        "a person without identifier (REQ-012-9)"
    );
}
