//! The routes of the module through the router (spec 010, REQ-010-24, 25, 27; task
//! 010-18): the collections read under `policy.catalogue.read` and written by family, the
//! matrix view and its cells, the query of compliance in its two forms, the audit report.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::audit::Source;
use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method as AuthMethod, Principal};
use kartenn_service::operations::RawInput;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ObjectKind, Operation, Ref};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use crate::support::env::{Env, op};

/// Any `Bearer` token is this principal.
struct As(Principal);

impl Authenticator for As {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let principal = match proof {
            Proof::Bearer(_) => Ok(self.0.clone().into()),
            _ => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(principal))
    }
}

impl Env {
    /// The router of an instance with the module, as the person `identity`.
    fn app(&self, identity: Uuid) -> Router {
        let api = Api {
            pool: self.pool.clone(),
            ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
            bound: Duration::from_secs(60),
            journal: PgAuditJournal::new(self.pool.clone(), Instance::new("test")),
            audit: ReadAudit::default(),
            sources: kartenn_service::expected::Sources::none(),
            outputs: Arc::from([]),
            modules: self.registry.clone(),
        };
        kartenn_rest::router(
            kartenn_rest::v1_with(api, vec![kartenn_policy::rest::routes()]),
            Arc::new(PgReadiness::new(self.pool.clone(), AuditStatus::default())),
            Settings::default(),
            Arc::new(Quotas::new(Rates::default())),
            Arc::new(As(Principal {
                identity,
                kind: IdentityKind::Person,
                issuer: None,
                groups: Vec::new(),
                method: AuthMethod::Bearer,
            })),
        )
    }

    /// A person granted `entries` everywhere.
    async fn person(&self, entries: &[&str]) -> Uuid {
        let pool = &self.pool;
        let identity: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let role = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO role (id, key, label, valid_period, changeset_id)
             VALUES ($1, $2, 'r', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
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
            "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                                  valid_period, changeset_id)
             VALUES (gen_random_uuid(), 'identity', $1, $2, 'global', NULL,
                     tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(identity)
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
        identity
    }

    /// The network `prefix` of the domain classified `environment`, and its address `at`.
    async fn host(&self, prefix: &str, environment: &str, at: &str) -> Uuid {
        let property = self.property("policy.environment").await;
        self.applied(vec![
            Operation::CreatePrefix {
                local: Some(1),
                domain: Ref::Id(self.domain),
                prefix: prefix.into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
            Operation::AssignProperty {
                object: Ref::Local(1),
                kind: ObjectKind::Prefix,
                property: Ref::Id(property),
                values: vec![RawInput::Text(environment.into())],
            },
            Operation::CreateAddress {
                local: None,
                domain: Ref::Id(self.domain),
                address: at.into(),
                usage: AddressUsage::Host,
            },
        ])
        .await;
        sqlx::query_scalar("SELECT id FROM address WHERE address = $1::inet")
            .bind(at)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

/// The environments `prod` and `dev`, the flow type `admin` with its service `ssh`, the rule
/// « prod to dev for admin ».
async fn policy(env: &Env) {
    env.applied(vec![
        op("environment.create", json!({ "key": "prod" })),
        op("environment.create", json!({ "key": "dev" })),
        op(
            "flow_type.create",
            json!({ "local": 1, "key": "admin", "label": "Administration" }),
        ),
        op(
            "service.create",
            json!({ "key": "ssh", "label": "SSH", "flow_type": { "local": 1 },
                    "entries": [{ "transport": "tcp", "ports": 22 }] }),
        ),
        op(
            "rule.create",
            json!({ "key": "prod-dev", "label": "Prod vers dev", "effect": "allow",
                    "flow_types": [{ "local": 1 }],
                    "source": { "policy.environment": ["prod"] },
                    "destination": { "policy.environment": ["dev"] } }),
        ),
    ])
    .await;
}

async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> (StatusCode, HeaderMapLite, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, "Bearer t");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        None => request.body(Body::empty()).unwrap(),
    };
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let etag = response.headers().get(header::ETAG).cloned();
    let location = response.headers().get(header::LOCATION).cloned();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, HeaderMapLite { etag, location }, value)
}

/// The headers the tests read.
struct HeaderMapLite {
    etag: Option<HeaderValue>,
    location: Option<HeaderValue>,
}

fn keys(page: &Value, member: &str) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i[member].as_str().unwrap().to_owned())
        .collect()
}

#[kartenn_testing::pg_test]
async fn the_collections_are_read_under_the_catalogue(pool: PgPool) {
    let env = Env::new(&pool).await;
    policy(&env).await;
    let reader = env.app(env.person(&["policy.catalogue.read"]).await);
    let (status, _, page) = call(&reader, Method::GET, "/v1/environments", None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(keys(&page, "value_key"), ["dev", "prod"]);
    let (_, _, page) = call(
        &reader,
        Method::GET,
        "/v1/policy-rules?effect=allow",
        None,
        &[],
    )
    .await;
    assert_eq!(keys(&page, "key"), ["prod-dev"]);
    let (_, _, page) = call(
        &reader,
        Method::GET,
        "/v1/policy-rules?effect=deny",
        None,
        &[],
    )
    .await;
    assert!(page["items"].as_array().unwrap().is_empty());
    let (_, _, page) = call(&reader, Method::GET, "/v1/services", None, &[]).await;
    let id = page["items"][0]["id"].as_str().unwrap().to_owned();
    let flow_type = page["items"][0]["flow_type_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (_, _, page) = call(
        &reader,
        Method::GET,
        &format!("/v1/services?flow_type={flow_type}"),
        None,
        &[],
    )
    .await;
    assert_eq!(keys(&page, "key"), ["ssh"]);
    let (status, headers, object) = call(
        &reader,
        Method::GET,
        &format!("/v1/services/{id}"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(object["key"], "ssh");
    assert!(object.get("changeset_id").is_none());
    assert!(object["valid_from"].is_string());
    assert!(headers.etag.is_some());

    // Without the catalogue: nothing listed, an object answered as absent (REQ-010-24).
    let stranger = env.app(env.person(&[]).await);
    let (_, _, page) = call(&stranger, Method::GET, "/v1/services", None, &[]).await;
    assert!(page["items"].as_array().unwrap().is_empty());
    let (status, _, _) = call(
        &stranger,
        Method::GET,
        &format!("/v1/services/{id}"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = call(
        &reader,
        Method::GET,
        "/v1/policy-rules?colour=red",
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[kartenn_testing::pg_test]
async fn a_family_is_written_under_its_administration(pool: PgPool) {
    let env = Env::new(&pool).await;
    let admin = env.app(
        env.person(&["policy.catalogue.read", "policy.flow_type.admin"])
            .await,
    );
    let (status, headers, body) = call(
        &admin,
        Method::POST,
        "/v1/flow-types",
        Some(json!({ "key": "backup", "label": "Sauvegarde", "motive": "m" })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let location = headers.location.unwrap().to_str().unwrap().to_owned();
    let (_, headers, _) = call(&admin, Method::GET, &location, None, &[]).await;
    let etag = headers.etag.unwrap().to_str().unwrap().to_owned();
    let (status, _, body) = call(
        &admin,
        Method::PATCH,
        &location,
        Some(json!({ "label": "Sauvegardes", "motive": "m" })),
        &[
            ("if-match", &etag),
            ("content-type", "application/merge-patch+json"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, headers, object) = call(&admin, Method::GET, &location, None, &[]).await;
    assert_eq!(object["label"], "Sauvegardes");
    let etag = headers.etag.unwrap().to_str().unwrap().to_owned();
    let (status, _, body) = call(
        &admin,
        Method::DELETE,
        &location,
        Some(json!({ "motive": "m" })),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, _) = call(&admin, Method::GET, &location, None, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Another family: refused (S15).
    let (status, _, body) = call(
        &admin,
        Method::POST,
        "/v1/policy-patterns",
        Some(json!({ "key": "p", "label": "P", "motive": "m" })),
        &[],
    )
    .await;
    assert!(status.is_client_error(), "{status} {body}");
}

#[kartenn_testing::pg_test]
async fn the_matrix_is_read_and_its_cells_are_set(pool: PgPool) {
    let env = Env::new(&pool).await;
    policy(&env).await;
    let flow_type: Uuid =
        sqlx::query_scalar("SELECT id FROM policy.policy_flow_type WHERE key = 'admin'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    let admin = env.app(
        env.person(&["policy.catalogue.read", "policy.rule.admin"])
            .await,
    );
    let (status, headers, matrix) = call(
        &admin,
        Method::GET,
        "/v1/policy-matrix?flow_type=admin",
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{matrix}");
    assert_eq!(matrix["cells"][0]["source"], "prod");
    assert_eq!(matrix["cells"][0]["state"], "allow");
    let etag = headers.etag.unwrap().to_str().unwrap().to_owned();
    assert!(!etag.starts_with("W/"), "a strong tag: {etag}");
    let deny = json!({ "state": "deny", "motive": "m",
                       "texts": { "fr": { "body": "Pas de dev vers prod." } } });
    let cell = format!("/v1/policy-matrix/dev/{flow_type}/prod");
    // S35: without If-Match, 428.
    let (status, _, body) = call(&admin, Method::PUT, &cell, Some(deny.clone()), &[]).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED, "{body}");
    let (status, _, body) = call(
        &admin,
        Method::PUT,
        &cell,
        Some(deny),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The same tag once the matrix changed: 412, the cell not written.
    let other = format!("/v1/policy-matrix/dev/{flow_type}/dev");
    let (status, _, body) = call(
        &admin,
        Method::PUT,
        &other,
        Some(json!({ "state": "allow", "motive": "m" })),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "{body}");
    let (_, headers, matrix) = call(
        &admin,
        Method::GET,
        "/v1/policy-matrix?flow_type=admin",
        None,
        &[],
    )
    .await;
    assert!(
        !matrix["cells"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["source"] == "dev" && c["destination"] == "dev"),
        "{matrix}"
    );
    let reread = headers.etag.unwrap().to_str().unwrap().to_owned();
    assert_ne!(reread, etag);
    let (status, _, body) = call(
        &admin,
        Method::PUT,
        &other,
        Some(json!({ "state": "allow", "motive": "m" })),
        &[("if-match", &reread)],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, _, matrix) = call(
        &admin,
        Method::GET,
        "/v1/policy-matrix?flow_type=admin",
        None,
        &[],
    )
    .await;
    let states: Vec<(String, String, String)> = matrix["cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["source"].as_str().unwrap().to_owned(),
                c["destination"].as_str().unwrap().to_owned(),
                c["state"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(states.contains(&("dev".into(), "prod".into(), "deny".into())));
    let (status, _, _) = call(
        &admin,
        Method::GET,
        "/v1/policy-matrix?flow_type=nothing",
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[kartenn_testing::pg_test]
async fn a_verdict_is_asked_in_both_forms(pool: PgPool) {
    let env = Env::new(&pool).await;
    policy(&env).await;
    let source = env.host("10.1.0.0/24", "prod", "10.1.0.5").await;
    let destination = env.host("10.2.0.0/24", "dev", "10.2.0.7").await;
    let auditor = env.app(
        env.person(&["policy.verdict.read", "core.address.read"])
            .await,
    );
    let body = json!({
        "source": { "kind": "address", "id": source },
        "destination": { "kind": "address", "id": destination },
        "service": "ssh",
    });
    let query = Method::from_bytes(b"QUERY").unwrap();
    let (status, _, verdict) = call(
        &auditor,
        query.clone(),
        "/v1/verdicts",
        Some(body.clone()),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{verdict}");
    assert_eq!(verdict["access"], "full");
    assert_eq!(verdict["synthesis"]["verdict"], "allowed");
    assert_eq!(verdict["evaluations"][0]["decisive"]["rule"], "prod-dev");
    let (status, _, simple) = call(
        &auditor,
        Method::GET,
        &format!(
            "/v1/verdicts?source=address:{source}&destination=address:{destination}&service=ssh"
        ),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{simple}");
    assert_eq!(simple["synthesis"], verdict["synthesis"]);

    // The other way: no rule, denied.
    let (_, _, back) = call(
        &auditor,
        Method::GET,
        &format!(
            "/v1/verdicts?source=address:{destination}&destination=address:{source}&service=ssh"
        ),
        None,
        &[],
    )
    .await;
    assert_eq!(back["synthesis"]["verdict"], "denied");

    // Without right: the 404 of an absent object (REQ-010-25).
    let stranger = env.app(env.person(&["core.address.read"]).await);
    let (status, _, _) = call(&stranger, query.clone(), "/v1/verdicts", Some(body), &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A range carries no classification; a malformed query is refused.
    let (status, _, _) = call(
        &auditor,
        query.clone(),
        "/v1/verdicts",
        Some(json!({ "source": { "kind": "range", "id": source },
                     "destination": { "kind": "address", "id": destination },
                     "service": "ssh" })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _, _) = call(
        &auditor,
        query,
        "/v1/verdicts",
        Some(json!({ "source": { "kind": "address", "id": source },
                     "destination": { "kind": "address", "id": destination } })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[kartenn_testing::pg_test]
async fn a_report_is_asked_for_and_read(pool: PgPool) {
    let env = Env::new(&pool).await;
    let admin = env.app(env.person(&["policy.rule.admin"]).await);
    let (status, headers, body) = call(
        &admin,
        Method::POST,
        "/v1/policy-reports",
        Some(json!({})),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let location = headers.location.unwrap().to_str().unwrap().to_owned();
    let (status, _, report) = call(&admin, Method::GET, &location, None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["state"], "pending");
    let (status, _, _) = call(&admin, Method::POST, "/v1/policy-reports", None, &[]).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _, items) = call(
        &admin,
        Method::GET,
        &format!("{location}/items?rubric=unclassified"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{items}");
    assert!(items["items"].as_array().unwrap().is_empty());

    let reader = env.app(env.person(&["policy.catalogue.read"]).await);
    let (status, _, _) = call(&reader, Method::GET, &location, None, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = call(&reader, Method::POST, "/v1/policy-reports", None, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// The reports are listed, the most recently asked for first, by state and by date,
/// page by page under the rights of a report (task 021-30).
#[kartenn_testing::pg_test]
async fn the_reports_are_listed_by_pages(pool: PgPool) {
    let env = Env::new(&pool).await;
    let launcher = env.person(&["policy.rule.admin"]).await;
    let mut asked = Vec::new();
    for days in 1..=4 {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO policy.policy_report (id, valid_at, requested_by, requested_at,
                                                state, done_at)
             VALUES (gen_random_uuid(), now(), $1, now() - make_interval(days => $2), 'done',
                     now())
             RETURNING id",
        )
        .bind(launcher)
        .bind(days)
        .fetch_one(&pool)
        .await
        .unwrap();
        asked.push(id.to_string());
    }
    let admin = env.app(env.person(&["policy.rule.admin"]).await);
    let (status, _, body) = call(
        &admin,
        Method::POST,
        "/v1/policy-reports",
        Some(json!({})),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let pending = body["id"].as_str().unwrap().to_owned();
    let ids = |page: &Value| -> Vec<String> {
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_owned())
            .collect()
    };

    // Page after page, newest first; the next link holds the filters.
    let (mut seen, mut uri) = (Vec::new(), "/v1/policy-reports?page_size=2".to_owned());
    let mut cursor = None;
    loop {
        let (status, _, page) = call(&admin, Method::GET, &uri, None, &[]).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        seen.extend(ids(&page));
        match page["links"]["next"].as_str() {
            Some(next) => {
                cursor.get_or_insert_with(|| next.to_owned());
                next.clone_into(&mut uri);
            }
            None => break,
        }
    }
    let mut wanted = vec![pending.clone()];
    wanted.extend(asked.iter().cloned());
    assert_eq!(seen, wanted);
    let (_, _, page) = call(
        &admin,
        Method::GET,
        "/v1/policy-reports?state=pending&state=running",
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&page), std::slice::from_ref(&pending));
    assert_eq!(page["items"][0]["state"], "pending");
    let (_, _, page) = call(
        &admin,
        Method::GET,
        &format!("/v1/policy-reports?state=done&requested_by={launcher}"),
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&page), asked);
    let to: String = sqlx::query_scalar(
        "SELECT to_char((now() - interval '36 hours') AT TIME ZONE 'UTC',
                        'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, _, page) = call(
        &admin,
        Method::GET,
        &format!("/v1/policy-reports?to={to}"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(ids(&page), asked[1..]);

    // An altered cursor, an unknown state or parameter, a date: 400.
    let cursor = cursor.unwrap();
    let (head, last) = cursor.split_at(cursor.len() - 1);
    let altered = format!("{head}{}", if last == "A" { "B" } else { "A" });
    for uri in [
        altered.as_str(),
        "/v1/policy-reports?state=lost",
        "/v1/policy-reports?colour=red",
        "/v1/policy-reports?valid_at=2026-01-01T00:00:00Z",
    ] {
        let (status, _, _) = call(&admin, Method::GET, uri, None, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
    }
    // Without the right of a report: 404, as a report.
    let reader = env.app(env.person(&["policy.catalogue.read"]).await);
    let (status, _, _) = call(&reader, Method::GET, "/v1/policy-reports", None, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// The rows `policy.inference.node` of the section of `answer` (a simulation) or of
/// `page` (a page of the section).
fn preview_nodes(rows: &Value) -> Vec<Value> {
    rows.as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "policy.inference.node")
        .cloned()
        .collect()
}

/// S36, REQ-005-21: on a sensitive axis, the values of the preview (expected, candidates,
/// rule, declared) are masked for a reader without `prop.<axis>.read` on the node, in the
/// simulation as in the section of the changeset applied; shown with the right.
#[kartenn_testing::pg_test]
async fn the_preview_masks_the_values_of_a_sensitive_axis(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (_, addresses) = crate::preview::tree(&env).await;
    sqlx::query(
        "UPDATE property_declaration SET sensitive = true WHERE key = 'policy.environment'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let rights = ["policy.inference.admin", "core.changeset.read"];
    let blind = env.app(env.person(&rights).await);
    let mut seeing_rights = rights.to_vec();
    seeing_rights.push("prop.policy.environment.read");
    let seeing = env.app(env.person(&seeing_rights).await);
    let body = json!({
        "motive": "aperçu",
        "operations": [{
            "op": "policy.inference_rule.create", "key": "r-prod",
            "axis": "policy.environment", "priority": 0,
            "criteria": [{ "within": [crate::preview::NETWORK] }], "values": ["prod"],
        }],
    });
    let section = |answer: &Value| -> Value {
        answer["sections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == "policy.combinations")
            .map_or_else(|| panic!("no section: {answer}"), |s| s["rows"].clone())
    };
    let declared_dev = |rows: &[Value]| {
        rows.iter()
            .find(|r| r["object"] == addresses[0].to_string())
            .cloned()
            .unwrap()
    };

    let uri = "/v1/changesets?simulate=true";
    let (status, _, answer) = call(&blind, Method::POST, uri, Some(body.clone()), &[]).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let rows = preview_nodes(&section(&answer));
    assert_eq!(rows.len(), 12, "{answer}");
    let row = declared_dev(&rows);
    for side in ["before", "after"] {
        let shown = &row[side];
        assert_eq!(shown["masked"], true, "{row}");
        for member in ["expected", "candidates", "rule", "declared"] {
            assert_eq!(shown[member], Value::Null, "{member}: {row}");
        }
    }
    assert_eq!(
        row["after"]["deviates"], true,
        "the deviation is not a value"
    );
    let (status, _, answer) = call(&seeing, Method::POST, uri, Some(body.clone()), &[]).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let row = declared_dev(&preview_nodes(&section(&answer)));
    assert_eq!(row["after"]["expected"], json!(["prod"]), "{row}");
    assert_eq!(row["after"]["declared"], json!(["dev"]));
    assert!(row["after"].get("masked").is_none(), "{row}");

    // Applied: the section of the changeset, masked the same way at its read.
    let (status, _, applied) = call(&seeing, Method::POST, "/v1/changesets", Some(body), &[]).await;
    assert_eq!(status, StatusCode::CREATED, "{applied}");
    let id = applied["id"]
        .as_str()
        .or_else(|| applied["changeset"]["id"].as_str())
        .unwrap_or_else(|| panic!("{applied}"))
        .to_owned();
    let page = format!("/v1/changesets/{id}/diff?section=policy.combinations&page_size=1000");
    let (status, _, read) = call(&blind, Method::GET, &page, None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{read}");
    let row = declared_dev(&preview_nodes(&read["rows"]));
    assert_eq!(row["after"]["masked"], true, "{row}");
    assert_eq!(row["after"]["expected"], Value::Null);
    let (_, _, read) = call(&seeing, Method::GET, &page, None, &[]).await;
    let row = declared_dev(&preview_nodes(&read["rows"]));
    assert_eq!(row["after"]["expected"], json!(["prod"]), "{row}");
}
