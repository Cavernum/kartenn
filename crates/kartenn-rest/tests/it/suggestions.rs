//! Suggestions (spec 018, REQ-018-23; task 018-15): the catalogue of the rules of the
//! build, the prefix `q` on the collections of the core. The rights of the suggestions
//! are proven with the modules, in `kartenn-machines` (`a_suggestion_never_shows_an_
//! unreadable_object`).

use axum::http::StatusCode;
use serde_json::Value;
use sqlx::PgPool;

use crate::labels::{app_with, get, person};

fn field(body: &Value, name: &str) -> Vec<String> {
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i[name].as_str().unwrap().to_owned())
        .collect()
}

/// REQ-018-23: `GET /v1/rules` serves each rule of the build, its severity and its
/// module, to any authenticated caller; `q` narrows it by prefix.
#[kartenn_testing::pg_test]
async fn the_rule_catalogue_is_served(pool: PgPool) {
    let registry = kartenn_server::modules::registry().unwrap();
    let app = app_with(&pool, person(&pool, &[]).await, registry);
    let (status, _, body) = get(&app, "/v1/rules", &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["items"].as_array().unwrap();
    let of = |id: &str| {
        items
            .iter()
            .find(|i| i["id"] == id)
            .unwrap_or_else(|| panic!("{id} missing: {body}"))
            .clone()
    };
    let import = of("import.unknown-reference");
    assert_eq!(
        (&import["severity"], &import["module"]),
        (&"error".into(), &"core".into())
    );
    assert_eq!(of("flow.update.ended")["module"], "flow");
    assert_eq!(
        of("prop.resolution.expected_mismatch")["severity"],
        "warning"
    );
    assert!(body["links"]["next"].is_null());
    let (_, _, imports) = get(&app, "/v1/rules?q=IMPORT.", &[]).await;
    assert_eq!(
        field(&imports, "id"),
        [
            "import.missing-external-id",
            "import.unknown-reference",
            "import.order",
            "import.invalid"
        ]
    );
    let (status, _, _) = get(&app, "/v1/rules?severity=error", &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
