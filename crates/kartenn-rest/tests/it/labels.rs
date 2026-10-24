//! The dictionary of labels `GET /v1/labels` (spec 018, REQ-018-14 to 17, 19; task
//! 018-10): negotiated by language, revised, dated, filtered by rights.

use crate::common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::values::Cardinality;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::authz::Unrestricted;
use kartenn_service::module::{LabelTable, Module, ModuleRegistry};
use kartenn_service::operations::{DeclarationInput, PeriodSpec, RawLabels, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// A module whose things own labels, read under `kit.thing.read`.
#[derive(Debug)]
struct Kit;

impl Module for Kit {
    fn name(&self) -> &'static str {
        "kit"
    }
    fn version(&self) -> &'static str {
        "1.0.0"
    }
    fn label_tables(&self) -> &'static [LabelTable] {
        &[LabelTable {
            space: "kit.thing",
            table: "kit_thing",
            permission: "kit.thing.read",
        }]
    }
}

/// A person holding `entries` on the global scope.
async fn person(pool: &PgPool, entries: &[&str]) -> Uuid {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, $2, '{\"fr\": \"Rôle de test\", \"en\": \"Test role\"}',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .bind(format!("r_{}", role.simple()))
    .execute(pool)
    .await
    .unwrap();
    for entry in entries {
        sqlx::query(
            "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
             VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(role)
        .bind(entry)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, valid_period,
                              changeset_id)
         VALUES (gen_random_uuid(), 'identity', $1, $2, 'global',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(identity)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    identity
}

fn app(pool: &PgPool, identity: Uuid) -> Router {
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from([]),
        modules: Arc::new(ModuleRegistry::new(vec![Arc::new(Kit) as Arc<dyn Module>]).unwrap()),
    };
    kartenn_rest::router(
        kartenn_rest::v1(api),
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        Settings::default(),
        Arc::new(Quotas::new(Rates::default())),
        Arc::new(common::As(Principal {
            identity,
            kind: IdentityKind::Person,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })),
    )
}

/// A read of the dictionary: status, headers, body.
async fn get(
    app: &Router,
    uri: &str,
    headers: &[(header::HeaderName, &str)],
) -> (StatusCode, header::HeaderMap, Value) {
    let mut request = Request::get(uri);
    for (name, value) in headers {
        request = request.header(name, *value);
    }
    let response = app
        .clone()
        .oneshot(common::bearer(request.body(Body::empty()).unwrap()))
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, body)
}

fn labels(pairs: &[(&str, &str)]) -> RawLabels {
    pairs
        .iter()
        .map(|(l, t)| ((*l).to_owned(), (*t).to_owned()))
        .collect()
}

/// Submits `operations` as the system, from `start` (microseconds) on when given.
async fn applied(pool: &PgPool, operations: Vec<Operation>, start: Option<i64>) {
    applied_over(
        pool,
        operations,
        start.map(|start| PeriodSpec { start, end: None }),
    )
    .await;
}

/// Submits `operations` as the system over `period` when given.
async fn applied_over(pool: &PgPool, operations: Vec<Operation>, period: Option<PeriodSpec>) {
    let request = ChangesetRequest {
        author: kartenn_service::declarations::SYSTEM_AUTHOR,
        motive: "labels".into(),
        period,
        operations,
        simulate: false,
        idempotency_key: None,
    };
    let outcome = submit(
        &PgStore::new(pool.clone()),
        &Unrestricted,
        &request,
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}

/// `custom.stage` with the values `prod` (French and English) and `rec` (French only),
/// from `start` on.
async fn stages(pool: &PgPool, sensitive: bool, start: Option<i64>) -> Uuid {
    applied(
        pool,
        vec![
            Operation::DeclareProperty {
                local: Some(1),
                declaration: DeclarationInput {
                    key: "custom.stage".into(),
                    value_type: TypeInput::Enum { values_of: None },
                    cardinality: Cardinality::Single,
                    carriers: vec!["prefix".into()],
                    inherit: None,
                    module: None,
                    obligations: Vec::new(),
                    expected: None,
                    labels: Some(labels(&[("fr", "Étape"), ("en", "Stage")])),
                },
            },
            Operation::AddEnumValue {
                local: None,
                property: Ref::Local(1),
                key: "prod".into(),
                labels: labels(&[("fr", "Production"), ("en", "Production line")]),
            },
            Operation::AddEnumValue {
                local: None,
                property: Ref::Local(1),
                key: "rec".into(),
                labels: labels(&[("fr", "Recette")]),
            },
        ],
        start,
    )
    .await;
    let property: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'custom.stage'")
            .fetch_one(pool)
            .await
            .unwrap();
    if sensitive {
        applied(
            pool,
            vec![Operation::SetSensitivity {
                property: Ref::Id(property),
                sensitive: true,
            }],
            None,
        )
        .await;
    }
    property
}

/// REQ-018-14, 15: `lang` prevails, else `Accept-Language` by Lookup; the response says
/// the language served and varies with `Accept-Language`; `spaces` narrows it.
#[kartenn_testing::pg_test]
async fn labels_are_negotiated(pool: PgPool) {
    stages(&pool, false, None).await;
    let app = app(&pool, person(&pool, &["core.entity.read"]).await);
    let (status, headers, body) = get(
        &app,
        "/v1/labels?spaces=value",
        &[(header::ACCEPT_LANGUAGE, "fr-CA, en;q=0.5")],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers[header::CONTENT_LANGUAGE], "fr");
    assert_eq!(headers[header::VARY], "Accept-Language");
    assert_eq!(body["lang"], "fr");
    assert_eq!(body["default_lang"], "fr");
    assert_eq!(
        body["spaces"]["value"]["custom.stage"]["prod"],
        "Production"
    );
    assert_eq!(
        body["spaces"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["value"]
    );
    let (_, headers, body) = get(
        &app,
        "/v1/labels?lang=en-GB",
        &[(header::ACCEPT_LANGUAGE, "fr")],
    )
    .await;
    assert_eq!(headers[header::CONTENT_LANGUAGE], "en");
    assert_eq!(
        body["spaces"]["value"]["custom.stage"]["prod"],
        "Production line"
    );
    assert_eq!(body["spaces"]["property"]["custom.stage"], "Stage");
    assert_eq!(
        body["spaces"]["permission"]["core.entity.read"],
        "Read the entities."
    );
    assert_eq!(
        body["spaces"]["problem"]["changeset-refused"],
        "The changeset is refused"
    );
    assert_eq!(body["spaces"]["kind"]["core.prefix"], "Prefix");
    let (status, _, problem) = get(&app, "/v1/labels?lang=fr_FR", &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{problem}");
    let (_, headers, _) = get(&app, "/v1/labels", &[(header::ACCEPT_LANGUAGE, "de")]).await;
    assert_eq!(
        headers[header::CONTENT_LANGUAGE],
        "fr",
        "the default language"
    );
}

/// REQ-018-15: an entry without a label in the language served takes that of the default
/// language, and the response says so.
#[kartenn_testing::pg_test]
async fn a_missing_label_falls_back_to_the_default_language(pool: PgPool) {
    stages(&pool, false, None).await;
    let app = app(&pool, person(&pool, &[]).await);
    let (_, _, body) = get(&app, "/v1/labels?lang=en&spaces=value", &[]).await;
    assert_eq!(body["spaces"]["value"]["custom.stage"]["rec"], "Recette");
    assert_eq!(
        body["fallback"],
        json!({"lang": "fr", "keys": ["value/custom.stage/rec"]})
    );
    let (_, _, body) = get(&app, "/v1/labels?lang=fr&spaces=value", &[]).await;
    assert!(body.get("fallback").is_none(), "{body}");
}

/// REQ-018-16: the same dictionary keeps its `ETag` and answers 304; a relabelled value
/// or another language changes it.
#[kartenn_testing::pg_test]
async fn the_labels_etag_changes_with_the_revision(pool: PgPool) {
    let property = stages(&pool, false, None).await;
    let app = app(&pool, person(&pool, &[]).await);
    let (_, headers, body) = get(&app, "/v1/labels?lang=fr", &[]).await;
    let etag = headers[header::ETAG].to_str().unwrap().to_owned();
    let revision = body["revision"].as_str().unwrap().to_owned();
    assert!(
        etag.starts_with(&format!("\"{revision}.")),
        "{etag} from {revision}"
    );
    assert_eq!(headers[header::CACHE_CONTROL], "private, no-cache");
    let (_, again, _) = get(&app, "/v1/labels?lang=fr", &[]).await;
    assert_eq!(again[header::ETAG], etag.as_str());
    let (status, unchanged, _) = get(
        &app,
        "/v1/labels?lang=fr",
        &[(header::IF_NONE_MATCH, etag.as_str())],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert_eq!(unchanged[header::CONTENT_LANGUAGE], "fr");
    let (_, english, _) = get(&app, "/v1/labels?lang=en", &[]).await;
    assert_ne!(english[header::ETAG], etag.as_str());
    applied(
        &pool,
        vec![Operation::UpdateEnumValue {
            property: Ref::Id(property),
            key: "prod".into(),
            labels: labels(&[("fr", "Prod.")]),
        }],
        None,
    )
    .await;
    let (status, changed, body) = get(
        &app,
        "/v1/labels?lang=fr",
        &[(header::IF_NONE_MATCH, etag.as_str())],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(changed[header::ETAG], etag.as_str());
    assert_ne!(body["revision"], revision.as_str());
    assert_eq!(body["spaces"]["value"]["custom.stage"]["prod"], "Prod.");
}

/// REQ-018-17: the labels of a valid time are those in force then; a read at a past
/// `as_of` is kept a day.
#[kartenn_testing::pg_test]
async fn labels_read_at_a_date(pool: PgPool) {
    let day = 86_400_000_000_i64;
    let now: i64 = sqlx::query_scalar("SELECT (extract(epoch FROM now()) * 1000000)::bigint")
        .fetch_one(&pool)
        .await
        .unwrap();
    let property = stages(&pool, false, Some(now - 10 * day)).await;
    applied(
        &pool,
        vec![Operation::UpdateEnumValue {
            property: Ref::Id(property),
            key: "prod".into(),
            labels: labels(&[("fr", "Production v2")]),
        }],
        Some(now - 5 * day),
    )
    .await;
    let app = app(&pool, person(&pool, &[]).await);
    let date = |micros: i64| {
        time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(micros) * 1000)
            .unwrap()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
            .replace('+', "%2B")
    };
    let (_, _, before) = get(
        &app,
        &format!(
            "/v1/labels?lang=fr&spaces=value&valid_at={}",
            date(now - 7 * day)
        ),
        &[],
    )
    .await;
    assert_eq!(
        before["spaces"]["value"]["custom.stage"]["prod"],
        "Production"
    );
    assert!(before["valid_at"].is_string(), "{before}");
    let (_, _, after) = get(&app, "/v1/labels?lang=fr&spaces=value", &[]).await;
    assert_eq!(
        after["spaces"]["value"]["custom.stage"]["prod"],
        "Production v2"
    );
    let (status, headers, past) = get(
        &app,
        &format!("/v1/labels?lang=fr&spaces=value&as_of={}", date(now - day)),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{past}");
    assert_eq!(headers[header::CACHE_CONTROL], "private, max-age=86400");
    assert!(
        past["spaces"].get("value").is_none(),
        "nothing asserted then: {past}"
    );
}

/// REQ-018-19: a space of data is served to the readers of its collection only; the
/// values of a sensitive property need its own read; the roles, `core.rbac.read` or a
/// binding; the codes, anyone. The `ETag` differs with the rights.
#[kartenn_testing::pg_test]
async fn a_space_is_served_only_to_its_readers(pool: PgPool) {
    sqlx::raw_sql(
        "CREATE TABLE kit_thing (key text, labels jsonb, valid_period tstzrange,
                                 sys_start timestamptz DEFAULT now());
         CREATE TABLE kit_thing_history (LIKE kit_thing, sys_end timestamptz);
         INSERT INTO kit_thing VALUES ('bolt', '{\"fr\": \"Boulon\", \"en\": \"Bolt\"}',
                                       tstzrange(now() - interval '1 day', NULL));",
    )
    .execute(&pool)
    .await
    .unwrap();
    stages(&pool, true, None).await;
    let nobody = app(&pool, person(&pool, &[]).await);
    let reader = app(
        &pool,
        person(
            &pool,
            &["kit.thing.read", "core.rbac.read", "prop.custom.stage.read"],
        )
        .await,
    );
    let (_, plain_headers, plain) = get(&nobody, "/v1/labels?lang=fr", &[]).await;
    assert!(plain["spaces"].get("kit.thing").is_none(), "{plain}");
    assert!(plain["spaces"]["value"].get("custom.stage").is_none());
    let roles = plain["spaces"]["role"].as_object().unwrap();
    assert_eq!(roles.len(), 1, "its own role only: {roles:?}");
    assert_eq!(plain["spaces"]["property"]["custom.stage"], "Étape");
    assert!(plain["spaces"]["rule"].is_object());
    let (_, reader_headers, read) = get(&reader, "/v1/labels?lang=fr", &[]).await;
    assert_eq!(read["spaces"]["kit.thing"]["bolt"], "Boulon");
    assert_eq!(
        read["spaces"]["value"]["custom.stage"]["prod"],
        "Production"
    );
    assert!(read["spaces"]["role"].as_object().unwrap().len() >= 2);
    assert_ne!(plain_headers[header::ETAG], reader_headers[header::ETAG]);
}

/// REQ-018-22: `/values` serves the key and the period of each value valid at
/// `valid_at`; `q` keeps those whose key or a label starts with it, ignoring case; a value
/// closed before the date is absent; the cursor keeps `q`.
#[kartenn_testing::pg_test]
async fn values_are_suggested_by_prefix_at_a_date(pool: PgPool) {
    let day = 86_400_000_000_i64;
    let now: i64 = sqlx::query_scalar("SELECT (extract(epoch FROM now()) * 1000000)::bigint")
        .fetch_one(&pool)
        .await
        .unwrap();
    let property = stages(&pool, false, Some(now - 10 * day)).await;
    applied(
        &pool,
        vec![Operation::AddEnumValue {
            local: None,
            property: Ref::Id(property),
            key: "preprod".into(),
            labels: labels(&[("fr", "Préproduction"), ("en", "Production staging")]),
        }],
        Some(now - 10 * day),
    )
    .await;
    applied_over(
        &pool,
        vec![Operation::AddEnumValue {
            local: None,
            property: Ref::Id(property),
            key: "proto".into(),
            labels: labels(&[]),
        }],
        Some(PeriodSpec {
            start: now - 10 * day,
            end: Some(now - 5 * day),
        }),
    )
    .await;
    // A label written over a portion splits the versions, not the period of the value.
    applied(
        &pool,
        vec![Operation::UpdateEnumValue {
            property: Ref::Id(property),
            key: "prod".into(),
            labels: labels(&[("fr", "Production v2")]),
        }],
        Some(now - 3 * day),
    )
    .await;
    let app = app(&pool, person(&pool, &[]).await);
    let date = |micros: i64| {
        time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(micros) * 1000)
            .unwrap()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    };
    let path = "/v1/property-declarations/custom.stage/values";
    let keys = |body: &Value| -> Vec<String> {
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["key"].as_str().unwrap().to_owned())
            .collect()
    };
    let (status, _, today) = get(&app, &format!("{path}?q=PRO"), &[]).await;
    assert_eq!(status, StatusCode::OK, "{today}");
    assert_eq!(keys(&today), ["preprod", "prod"], "{today}");
    let prod = &today["items"][1];
    assert_eq!(prod["valid_from"], date(now - 10 * day), "{prod}");
    assert!(prod["valid_to"].is_null(), "{prod}");
    let at = date(now - 7 * day).replace('+', "%2B");
    let (_, _, before) = get(&app, &format!("{path}?q=pro&valid_at={at}"), &[]).await;
    assert_eq!(keys(&before), ["preprod", "prod", "proto"], "{before}");
    assert_eq!(before["items"][2]["valid_to"], date(now - 5 * day));
    let (_, _, all) = get(&app, path, &[]).await;
    assert_eq!(keys(&all), ["preprod", "prod", "rec"], "{all}");
    let (_, _, by_label) = get(&app, &format!("{path}?q=PR%C3%89"), &[]).await;
    assert_eq!(
        keys(&by_label),
        ["preprod"],
        "« Préproduction »: {by_label}"
    );
    let (_, _, wild) = get(&app, &format!("{path}?q=%25"), &[]).await;
    assert_eq!(
        keys(&wild),
        Vec::<String>::new(),
        "% is a character: {wild}"
    );
    let (_, _, first) = get(&app, &format!("{path}?q=pro&page_size=1"), &[]).await;
    assert_eq!(keys(&first), ["preprod"]);
    let next = first["links"]["next"].as_str().unwrap();
    let (_, _, second) = get(&app, next, &[]).await;
    assert_eq!(keys(&second), ["prod"], "the cursor keeps q: {second}");
    assert!(second["links"]["next"].is_null(), "{second}");
    for wrong in ["q=a&q=b", "q=", "name=prod"] {
        let (status, _, body) = get(&app, &format!("{path}?{wrong}"), &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{wrong}: {body}");
    }
}
