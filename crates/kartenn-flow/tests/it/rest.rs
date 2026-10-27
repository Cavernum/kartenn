//! The routes of the registry through the router (task 011-9; REQ-011-14 to 18, 25): a
//! request answered with the flow and its evaluations, the list and the read under
//! `flow.read` or as the requester, the evaluations reduced for the requester, the
//! flows of an address, the non-conformities.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::audit::Source;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method as AuthMethod, Principal};
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use tower::ServiceExt;

use crate::requests::Env;

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
    /// The router of an instance with the modules, as the person `identity`.
    pub(crate) fn app(&self, identity: Uuid) -> Router {
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
            kartenn_rest::v1_with(api, vec![kartenn_flow::rest::routes()]),
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
    async fn granted(&self, entries: &[&str]) -> Uuid {
        self.granted_on(entries, None).await
    }

    /// A person granted `entries` on the entity `entity`, everywhere when `None`.
    async fn granted_on(&self, entries: &[&str], entity: Option<Uuid>) -> Uuid {
        let pool = &self.pool;
        let identity: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let role = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO role (id, key, labels, valid_period, changeset_id)
             VALUES ($1, $2, jsonb_build_object('fr', 'r'), tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
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
        sqlx::query(kartenn_testing::BIND_IDENTITY)
            .bind(identity)
            .bind(role)
            .bind(if entity.is_some() { "entity" } else { "global" })
            .bind(entity)
            .execute(pool)
            .await
            .unwrap();
        identity
    }
}

pub(crate) async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let uri = kartenn_testing::applied_by_author(method.as_str(), uri);
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, "Bearer t");
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        None => request.body(Body::empty()).unwrap(),
    };
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// The body of a request `prod → dev` for `ssh`.
async fn body(env: &Env, prod: Uuid, dev: Uuid) -> Value {
    let mut fields = env.fields(prod, dev).await;
    fields.as_object_mut().unwrap().remove("local");
    fields["motive"] = json!("m");
    fields
}

pub(crate) fn ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect()
}

/// REQ-011-18, 21: `POST /v1/flows` triages the request and, applied, answers with the
/// flow (its state) and its evaluations; the requester reads it, a stranger does not.
#[kartenn_testing::pg_test]
async fn a_request_answers_with_the_flow_and_its_evaluations(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let requester = env.granted(crate::requests::REQUEST).await;
    let app = env.app(requester);
    let (status, answer) = call(
        &app,
        Method::POST,
        "/v1/flows",
        Some(body(&env, prod, dev).await),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
    assert_eq!(answer["flow"]["state"], "active", "{answer}");
    assert_eq!(answer["flow"]["summary"], "allowed");
    let evaluations = answer["evaluations"].as_array().unwrap();
    assert_eq!(evaluations.len(), 1);
    assert_eq!(evaluations[0]["verdict"], "allowed");
    let id = answer["flow"]["id"].as_str().unwrap().to_owned();

    // The requester reads its own request; its evaluations are reduced (REQ-010-25).
    let (status, flow) = call(&app, Method::GET, &format!("/v1/flows/{id}"), None).await;
    assert_eq!(status, StatusCode::OK, "{flow}");
    let (status, page) = call(
        &app,
        Method::GET,
        &format!("/v1/flows/{id}/evaluations"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page["items"][0]["source"].is_null(), "{page}");
    assert_eq!(page["items"][0]["verdict"], "allowed");

    // A stranger: the 404 of an absent flow, an empty list.
    let stranger = env.app(env.granted(&[]).await);
    let (status, _) = call(&stranger, Method::GET, &format!("/v1/flows/{id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, page) = call(&stranger, Method::GET, "/v1/flows", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ids(&page).is_empty(), "{page}");
}

/// REQ-011-14, 15, 17: a reader under `flow.read` lists, filters and reads the flow
/// with its full evaluations, and finds it among the flows of its source address.
#[kartenn_testing::pg_test]
async fn a_reader_lists_filters_and_reads_flows(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let requester = env.granted(crate::requests::REQUEST).await;
    let (status, answer) = call(
        &env.app(requester),
        Method::POST,
        "/v1/flows",
        Some(body(&env, prod, dev).await),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
    let id = answer["flow"]["id"].as_str().unwrap().to_owned();

    let reader = env.app(env.granted(&["flow.read"]).await);
    let (status, page) = call(&reader, Method::GET, "/v1/flows", None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(ids(&page), std::slice::from_ref(&id));
    let (_, page) = call(&reader, Method::GET, "/v1/flows?summary=denied", None).await;
    assert!(ids(&page).is_empty(), "{page}");
    let (_, page) = call(
        &reader,
        Method::GET,
        &format!("/v1/flows?endpoint=address:{prod}&state=active"),
        None,
    )
    .await;
    assert_eq!(ids(&page), std::slice::from_ref(&id));
    let (status, _) = call(&reader, Method::GET, "/v1/flows?colour=blue", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, flow) = call(&reader, Method::GET, &format!("/v1/flows/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(flow["state"], "active");
    let (_, page) = call(
        &reader,
        Method::GET,
        &format!("/v1/flows/{id}/evaluations"),
        None,
    )
    .await;
    assert_eq!(page["items"][0]["source"], prod.to_string(), "{page}");

    let (status, page) = call(
        &reader,
        Method::GET,
        &format!("/v1/addresses/{prod}/flows"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(ids(&page), [id]);

    let (status, page) = call(&reader, Method::GET, "/v1/flow-nonconformities", None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page["items"].as_array().unwrap().is_empty());
}

/// The instant of the database, as a query parameter.
async fn instant(pool: &PgPool) -> String {
    let at: sqlx::types::time::OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(pool)
        .await
        .unwrap();
    at.format(&time::format_description::well_known::Rfc3339)
        .unwrap()
}

/// The items of a page.
fn items(page: &Value) -> &Vec<Value> {
    page["items"].as_array().unwrap()
}

/// REQ-011-5, 14, 17 (F19): a request held for a review is read with `state=requested`,
/// without identifier, with its proposal and the triage its operation keeps; reduced for
/// its requester, in full for a reader, seen by a reviewer, unseen elsewhere; absent
/// before its submission and once rejected.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // one block per reader and per filter of the acceptance
async fn a_pending_request_is_read_with_its_triage(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let before = instant(&pool).await;
    let requester = env.granted(crate::requests::REQUEST).await;
    let app = env.app(requester);
    let (status, answer) = call(
        &app,
        Method::POST,
        "/v1/flows",
        Some(body(&env, dev, prod).await),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{answer}");
    let changeset = answer["changeset"]["id"].as_str().unwrap().to_owned();
    let stored: Value = sqlx::query_scalar(
        "SELECT o -> 'completed' -> 'triage'
           FROM changeset c, jsonb_array_elements(c.request -> 'operations') o
          WHERE c.id = $1::uuid",
    )
    .bind(&changeset)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored["summary"], "denied", "{stored}");

    // The requester: the row, its triage reduced (REQ-010-25).
    let (status, page) = call(&app, Method::GET, "/v1/flows?state=requested", None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let [row] = items(&page).as_slice() else {
        panic!("one row: {page}");
    };
    assert!(row["id"].is_null(), "{row}");
    assert_eq!(row["state"], "requested");
    assert_eq!(row["fields"]["requester_id"], requester.to_string());
    let review = &row["review"];
    assert_eq!(review["changeset"], changeset.as_str());
    assert_eq!(review["kind"], "request");
    let triage = &review["triage"];
    assert_eq!(triage["outcome"], stored["outcome"]);
    assert_eq!(triage["summary"], stored["summary"]);
    assert_eq!(row["outcome"], stored["outcome"]);
    assert_eq!(triage["rule"], stored["rule"]);
    assert_eq!(triage["policy_revision"], stored["revision"]);
    assert!(triage["decisive"].is_null(), "{triage}");
    let pairs = triage["evaluations"].as_array().unwrap();
    assert_eq!(pairs.len(), 1, "{triage}");
    assert!(pairs[0]["source"].is_null(), "{triage}");
    // Without `requested`, the applied flows only.
    let (_, page) = call(&app, Method::GET, "/v1/flows", None).await;
    assert!(items(&page).is_empty(), "{page}");

    // A reader of the entity: the triage in full, as the operation keeps it.
    let reader = env.app(env.granted(&["flow.read"]).await);
    let (_, page) = call(&reader, Method::GET, "/v1/flows?state=requested", None).await;
    let [row] = items(&page).as_slice() else {
        panic!("one row: {page}");
    };
    let triage = &row["review"]["triage"];
    assert_eq!(triage["decisive"], stored["decisive"]);
    assert_eq!(triage["evaluations"], stored["evaluations"], "{triage}");
    assert_eq!(triage["evaluations"][0]["source"], dev.to_string());
    for query in [
        "review=pending",
        "state=active,requested",
        &format!("changeset={changeset}"),
        &format!("state=requested&requester={requester}"),
        &format!("state=requested&entity={}", env.acme),
        &format!("state=requested&endpoint=address:{dev}"),
        "state=requested&summary=denied",
    ] {
        let (status, page) = call(&reader, Method::GET, &format!("/v1/flows?{query}"), None).await;
        assert_eq!(status, StatusCode::OK, "{query}: {page}");
        assert_eq!(items(&page).len(), 1, "{query}: {page}");
    }
    let network: Uuid = sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = '10.2.0.0/24'")
        .fetch_one(&pool)
        .await
        .unwrap();
    for query in [
        format!("state=requested&prefix={network}"),
        format!("changeset={}", Uuid::now_v7()),
        format!("state=requested&endpoint=address:{prod}&summary=allowed"),
        format!("state=requested&as_of={before}"),
        "state=active".to_owned(),
    ] {
        let (status, page) = call(&reader, Method::GET, &format!("/v1/flows?{query}"), None).await;
        assert_eq!(status, StatusCode::OK, "{query}: {page}");
        assert!(items(&page).is_empty(), "{query}: {page}");
    }

    // A reviewer sees it; a reader of another entity does not.
    let reviewer = env.granted(&["flow.review"]).await;
    let (_, page) = call(
        &env.app(reviewer),
        Method::GET,
        "/v1/flows?review=pending",
        None,
    )
    .await;
    assert_eq!(items(&page).len(), 1, "{page}");
    env.applied(vec![kartenn_service::Operation::CreateEntity {
        local: Some(1),
        slug: "other".into(),
        parent: None,
    }])
    .await;
    let other: Uuid = sqlx::query_scalar("SELECT id FROM entity WHERE slug = 'other'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let elsewhere = env.app(env.granted_on(&["flow.read"], Some(other)).await);
    let (_, page) = call(&elsewhere, Method::GET, "/v1/flows?state=requested", None).await;
    assert!(items(&page).is_empty(), "{page}");

    // Rejected: no longer pending, but pending at a date before the rejection.
    let pending = instant(&pool).await;
    kartenn_service::review::reject(
        &env.store,
        &kartenn_service::authz::Unrestricted,
        changeset.parse().unwrap(),
        &reviewer.into(),
        "no administration from dev",
    )
    .await
    .unwrap();
    let (_, page) = call(&reader, Method::GET, "/v1/flows?state=requested", None).await;
    assert!(items(&page).is_empty(), "{page}");
    let (_, page) = call(
        &reader,
        Method::GET,
        &format!("/v1/flows?state=requested&as_of={pending}"),
        None,
    )
    .await;
    assert_eq!(items(&page).len(), 1, "{page}");
}

/// F19: the pending rows follow the applied flows, a page at a time, the cursor on the
/// proposal.
#[kartenn_testing::pg_test]
async fn pending_requests_are_paged_after_the_applied_flows(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let requester = env.granted(crate::requests::REQUEST).await;
    let app = env.app(requester);
    let (status, _) = call(
        &app,
        Method::POST,
        "/v1/flows",
        Some(body(&env, prod, dev).await),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    for _ in 0..2 {
        let (status, answer) = call(
            &app,
            Method::POST,
            "/v1/flows",
            Some(body(&env, dev, prod).await),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED, "{answer}");
    }
    let mut uri = "/v1/flows?state=active,requested&page_size=1".to_owned();
    let mut seen = Vec::new();
    loop {
        let (status, page) = call(&app, Method::GET, &uri, None).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        seen.extend(
            items(&page)
                .iter()
                .map(|i| i["state"].as_str().unwrap().to_owned()),
        );
        match page["links"]["next"].as_str() {
            Some(next) => uri = next.to_owned(),
            None => break,
        }
    }
    assert_eq!(seen, ["active", "requested", "requested"]);
}

/// `PATCH /v1/flows/{id}` under the version read, as a JSON Merge Patch.
pub(crate) async fn patch(app: &Router, id: Uuid, body: &Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/v1/flows/{id}"))
        .header(header::AUTHORIZATION, "Bearer t")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let etag = response.headers()[header::ETAG].clone();
    let request = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/v1/flows/{id}?approval=self"))
        .header(header::AUTHORIZATION, "Bearer t")
        .header(header::CONTENT_TYPE, kartenn_rest::kit::MERGE_PATCH)
        .header(header::IF_MATCH, etag)
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// REQ-011-4, 18, F20: `PATCH` applies an earlier end (200) and holds for a review an
/// extension whose added portion is allowed under reserve (202, the changeset pending).
#[kartenn_testing::pg_test]
async fn a_patch_shortens_or_holds_an_extension(pool: PgPool) {
    use crate::extensions::{WRITE, instructed, parties, under_reserve};
    let env = Env::new(&pool).await;
    under_reserve(&env).await;
    let a = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let b = env.address("10.3.0.0/24", "10.3.0.1", "prod").await;
    let (requester, reviewer, _, authorizer) = parties(&env).await;
    let id = instructed(&env, &authorizer, (requester, reviewer), (a, b), 200, None).await;
    let writer = env.app(env.granted(&[WRITE, &["flow.read"]].concat()).await);

    let (status, answer) = patch(
        &writer,
        id,
        &json!({ "end": crate::requests::later(150), "motive": "m" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");

    let (status, answer) = patch(
        &writer,
        id,
        &json!({ "end": crate::requests::later(300), "motive": "m" }),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{answer}");
    assert_eq!(
        (answer["kind"].as_str(), answer["state"].as_str()),
        (Some("proposal"), Some("pending")),
        "{answer}"
    );

    // F19: the extension pending is read as a review of the flow, its added portion
    // triaged; the flow itself stays listed as applied.
    let changeset = answer["changeset"]["id"].as_str().unwrap().to_owned();
    let (status, page) = call(&writer, Method::GET, "/v1/flows?review=pending", None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let [row] = items(&page).as_slice() else {
        panic!("one row: {page}");
    };
    assert_eq!(row["id"], id.to_string(), "{row}");
    assert_eq!(row["review"]["kind"], "extension");
    assert_eq!(row["review"]["changeset"], changeset.as_str());
    let triage = &row["review"]["triage"];
    assert!(triage["outcome"].is_string(), "{row}");
    assert!(
        !triage["evaluations"].as_array().unwrap().is_empty(),
        "{row}"
    );
    let (_, page) = call(&writer, Method::GET, "/v1/flows", None).await;
    assert!(items(&page).iter().all(|i| i["review"].is_null()), "{page}");
}

/// The non-conformities page by signed cursor, newest first, to the last page (task
/// 021-29); an altered cursor is refused.
#[kartenn_testing::pg_test]
async fn nonconformities_page_by_cursor(pool: PgPool) {
    let env = Env::new(&pool).await;
    let reviewer = env.granted(&[]).await;
    let mut rejected = Vec::new();
    for n in 0..5 {
        let changeset = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO flow.flow_rejection (changeset_id, request, evaluation, reason,
                                              reviewed_by, requester_id, entity_id, rejected_at)
             VALUES ($1, '{}', '{}', 'non', $2, $2, $3,
                     now() - make_interval(hours => $4))",
        )
        .bind(changeset)
        .bind(reviewer)
        .bind(env.acme)
        .bind(n % 2)
        .execute(&pool)
        .await
        .unwrap();
        rejected.push((n % 2, changeset));
    }
    // Newest first, then by changeset descending.
    rejected.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let wanted: Vec<String> = rejected.iter().map(|(_, c)| c.to_string()).collect();
    let reader = env.app(env.granted(&["flow.read"]).await);
    let (mut seen, mut uri) = (
        Vec::new(),
        "/v1/flow-nonconformities?page_size=2".to_owned(),
    );
    let mut altered = None;
    loop {
        let (status, page) = call(&reader, Method::GET, &uri, None).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert!(items(&page).len() <= 2, "{page}");
        seen.extend(
            items(&page)
                .iter()
                .map(|i| i["changeset"].as_str().unwrap().to_owned()),
        );
        match page["links"]["next"].as_str() {
            Some(next) => {
                let (head, last) = next.split_at(next.len() - 1);
                altered.get_or_insert_with(|| {
                    format!("{head}{}", if last == "A" { "B" } else { "A" })
                });
                next.clone_into(&mut uri);
            }
            None => break,
        }
    }
    assert_eq!(seen, wanted);
    let (status, problem) = call(&reader, Method::GET, &altered.unwrap(), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{problem}");
    assert_eq!(problem["type"], "urn:kartenn:problem:invalid-cursor");
}

/// A host address of the domain, in a network already created.
async fn host(env: &Env, address: &str) -> Uuid {
    env.applied(vec![kartenn_service::Operation::CreateAddress {
        local: None,
        domain: kartenn_service::Ref::Id(env.domain),
        address: address.into(),
        usage: kartenn_core::objects::AddressUsage::Host,
    }])
    .await;
    sqlx::query_scalar("SELECT id FROM address WHERE address = $1::inet")
        .bind(address)
        .fetch_one(&env.pool)
        .await
        .unwrap()
}

/// `GET /v1/flow-checks` of `source` to `destination` (`<kind>:<id>`) for `ssh`, with
/// `extra` parameters.
pub(crate) async fn checked(app: &Router, source: &str, destination: &str, extra: &str) -> Value {
    let (status, answer) = call(
        app,
        Method::GET,
        &format!("/v1/flow-checks?source={source}&destination={destination}&service=ssh{extra}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    answer
}

/// The flows cited by the first pair of a test.
pub(crate) fn opening(answer: &Value) -> Vec<(String, String)> {
    answer["pairs"][0]["flows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["id"].as_str().unwrap().to_owned(),
                f["state"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// REQ-011-26 (F21): a connection test answers « open » (the flows `active` or `waived`
/// that cover the pair and the service) apart from « conformant » (the verdict), sums them
/// up in one of four cases, and writes nothing.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // the cases of acceptance, in turn
async fn acceptance_of_req_011_26_open_and_conformant(pool: PgPool) {
    use crate::requests::{flow, later, policy};

    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let other = host(&env, "10.1.0.2").await;
    let network: Uuid = sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = '10.2.0.0/24'")
        .fetch_one(&pool)
        .await
        .unwrap();
    // A flow `active` whose destination is the network of the destination.
    let mut fields = env.fields(prod, dev).await;
    fields["destination"] = json!({ "kind": "prefix", "id": { "id": network } });
    env.applied(vec![flow("request", fields)]).await;
    let wide: Uuid = sqlx::query_scalar("SELECT id FROM flow.flow")
        .fetch_one(&pool)
        .await
        .unwrap();
    let reader = env.app(
        env.granted(&["policy.verdict.read", "core.address.read", "flow.read"])
            .await,
    );
    let (src, dst) = (format!("address:{prod}"), format!("address:{dev}"));
    let flows = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM flow.flow")
            .fetch_one(&pool)
            .await
            .unwrap()
    };
    let before = flows().await;

    // Open and conformant, the flow cited; the same in the QUERY form.
    let answer = checked(&reader, &src, &dst, "").await;
    assert_eq!(answer["case"], "open_conformant", "{answer}");
    assert_eq!(answer["pairs"][0]["open"], true);
    assert_eq!(answer["pairs"][0]["conformant"], true);
    assert_eq!(opening(&answer), [(wide.to_string(), "active".to_owned())]);
    assert_eq!(answer["verdict"]["synthesis"]["verdict"], "allowed");
    let (status, queried) = call(
        &reader,
        Method::from_bytes(b"QUERY").unwrap(),
        "/v1/flow-checks",
        Some(json!({ "source": { "kind": "address", "id": prod },
                     "destination": { "kind": "address", "id": dev },
                     "service": "ssh" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{queried}");
    assert_eq!(queried["case"], answer["case"]);

    // No covering flow, verdict allowed: not open but conformant, the outcome given.
    let answer = checked(&reader, &format!("address:{other}"), &dst, "").await;
    assert_eq!(answer["case"], "not_open_conformant", "{answer}");
    assert!(answer["verdict"]["evaluations"][0]["outcome"].is_string());
    assert!(opening(&answer).is_empty());

    // A dated flow opens the pair until its end; expired, it no longer does.
    env.applied(vec![{
        let mut fields = env.fields(other, dev).await;
        fields["end"] = json!(later(1));
        flow("request", fields)
    }])
    .await;
    let answer = checked(&reader, &format!("address:{other}"), &dst, "").await;
    assert_eq!(answer["case"], "open_conformant", "{answer}");
    let at = format!("&valid_at={}", later(3));
    let answer = checked(&reader, &format!("address:{other}"), &dst, &at).await;
    assert_eq!(answer["case"], "not_open_conformant", "{answer}");

    // A request pending a review does not open the pair: not open, not conformant.
    let requester = env.person().await;
    let outcome = env
        .submit_as(
            &crate::requests::grants(requester, crate::requests::REQUEST),
            requester,
            vec![flow("request", env.fields(dev, prod).await)],
        )
        .await;
    crate::requests::proposal(&outcome);
    let answer = checked(&reader, &dst, &src, "").await;
    assert_eq!(answer["case"], "not_open_nonconformant", "{answer}");

    // A reader of the verdicts who may not read the flow: nothing open for them.
    let blind = env.app(
        env.granted(&["policy.verdict.read", "core.address.read"])
            .await,
    );
    let answer = checked(&blind, &src, &dst, "").await;
    assert_eq!(answer["case"], "not_open_conformant", "{answer}");

    // A rule `deny` makes the flow noncompliant; waived, it is open but not conformant,
    // the waiver and the rule cited.
    let admin: Uuid =
        sqlx::query_scalar("SELECT id FROM policy.policy_flow_type WHERE key = 'admin'")
            .fetch_one(&pool)
            .await
            .unwrap();
    env.applied(vec![policy(
        "rule.create",
        json!({ "key": "no-prod-dev", "labels": { "fr": "Pas de prod vers dev" },
                "effect": "deny", "flow_types": [{ "id": admin }],
                "source": { "policy.environment": ["prod"] },
                "destination": { "policy.environment": ["dev"] },
                "texts": { "fr": { "body": "Interdit." } } }),
    )])
    .await;
    // Granted by another than its requester.
    let outcome = env
        .submit_as(
            &kartenn_service::authz::Unrestricted,
            env.person().await,
            vec![flow(
                "waive",
                json!({ "flow": { "id": wide }, "reason": "migration",
                        "accepted_risk": "admin access", "end": later(30) }),
            )],
        )
        .await;
    assert!(
        matches!(outcome, kartenn_service::Outcome::Applied { .. }),
        "{outcome:?}"
    );
    let answer = checked(&reader, &src, &dst, "").await;
    assert_eq!(answer["case"], "open_nonconformant", "{answer}");
    assert_eq!(opening(&answer), [(wide.to_string(), "waived".to_owned())]);
    assert_eq!(
        answer["verdict"]["evaluations"][0]["decisive"]["rule"],
        "no-prod-dev"
    );

    // A machine is refused; nothing was written.
    let (web, _) = crate::endpoints::machine(&env, "web", prod).await;
    let (status, problem) = call(
        &reader,
        Method::GET,
        &format!("/v1/flow-checks?source=machine:{web}&destination={dst}&service=ssh"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    assert_eq!(problem["violations"][0]["rule"], "policy.endpoint.machine");
    assert_eq!(
        flows().await,
        before + 1,
        "only the dated flow was requested"
    );
}
