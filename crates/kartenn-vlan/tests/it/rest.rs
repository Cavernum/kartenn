//! The routes of the module through the router (spec 008, REQ-008-19 to 22; task 008-14):
//! the VLANs filtered in conjunction, the occupancy and the next free VID of a group, the
//! networks of a VLAN under the rights of the caller, the writes under the rights of the
//! writer and `If-Match`.

use crate::support::env;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode, header};
use env::{Env, op};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::audit::Source;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::operations::RawInput;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ObjectKind, Operation, Ref};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

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
            kartenn_rest::v1_with(api, vec![kartenn_vlan::rest::routes()]),
            Arc::new(PgReadiness::new(self.pool.clone(), AuditStatus::default())),
            Settings::default(),
            Arc::new(Quotas::new(Rates::default())),
            Arc::new(As(Principal {
                identity,
                kind: IdentityKind::Person,
                issuer: None,
                groups: Vec::new(),
                method: Method::Bearer,
            })),
        )
    }

    /// A person granted `entries` on `scope` (`None`: everywhere).
    async fn person(&self, entries: &[&str], scope: Option<(&str, Uuid)>) -> Uuid {
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
             VALUES (gen_random_uuid(), 'identity', $1, $2, $3, $4,
                     tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(identity)
        .bind(role)
        .bind(scope.map_or("global", |(kind, _)| kind))
        .bind(scope.map(|(_, id)| id))
        .execute(pool)
        .await
        .unwrap();
        identity
    }

    /// Everything on the module and the prefixes, everywhere.
    async fn admin(&self) -> Uuid {
        self.person(
            &[
                "vlan.group.read",
                "vlan.group.write",
                "vlan.read",
                "vlan.write",
                "core.prefix.read",
            ],
            None,
        )
        .await
    }

    /// A new group `slug` anchored on `anchor`, range `range`, with the VLANs
    /// `(vid, status)`.
    async fn seed_group(
        &self,
        slug: &str,
        anchor: Value,
        range: (i64, i64),
        vlans: &[(i64, &str)],
    ) -> Uuid {
        let mut operations = vec![op(
            "create_group",
            json!({"local": 1, "slug": slug, "anchor": anchor,
                   "vid_min": range.0, "vid_max": range.1}),
        )];
        for (vid, status) in vlans {
            operations.push(op(
                "create_vlan",
                json!({"group": {"local": 1}, "vid": vid, "name": format!("v{vid}"),
                       "status": status}),
            ));
        }
        self.applied(operations).await;
        sqlx::query_scalar(
            "SELECT id FROM vlan.vlan_group WHERE slug = $1 AND valid_period @> now()",
        )
        .bind(slug)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn vlan_of(&self, group: Uuid, vid: i16) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM vlan.vlan WHERE group_id = $1 AND vid = $2 AND valid_period @> now()",
        )
        .bind(group)
        .bind(vid)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

/// The answer of `method uri` with `body` (`Content-Type` `media`) and `headers`.
async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<(&str, Value)>,
    headers: &[(&str, String)],
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut request = Request::builder().method(method).uri(uri).header(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer test"),
    );
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    let request = match body {
        Some((media, body)) => request
            .header(header::CONTENT_TYPE, media)
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, value)
}

#[kartenn_testing::pg_test]
async fn vlans_filtered_by_vid_range(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let group = env
        .seed_group(
            "edge",
            json!({"domain": {"id": lab}}),
            (1, 4094),
            &[
                (25, "active"),
                (5, "active"),
                (15, "active"),
                (10, "active"),
                (12, "reserved"),
                (20, "active"),
            ],
        )
        .await;
    env.seed_group(
        "core",
        json!({"domain": {"id": lab}}),
        (1, 4094),
        &[(11, "active")],
    )
    .await;
    let app = env.app(env.admin().await);
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/v1/vlans?group={group}&vid=10..20&status=active"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let vids: Vec<i64> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["vid"].as_i64().unwrap())
        .collect();
    assert_eq!(vids, [10, 15, 20]);
    // The same filters in a QUERY body.
    let (status, _, body) = call(
        &app,
        "QUERY",
        "/v1/vlans",
        Some((
            "application/json",
            json!({"filters": [
                {"field": "group", "value": group.to_string()},
                {"field": "name_prefix", "value": "v1"},
            ]}),
        )),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let names: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["v10", "v12", "v15"]);
    // Pages of one, linked by the cursor.
    let (_, _, first) = call(
        &app,
        "GET",
        &format!("/v1/vlans?group={group}&page_size=4"),
        None,
        &[],
    )
    .await;
    let next = first["links"]["next"].as_str().unwrap().to_owned();
    let (_, _, second) = call(&app, "GET", &next, None, &[]).await;
    let vids: Vec<i64> = second["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["vid"].as_i64().unwrap())
        .collect();
    assert_eq!(vids, [20, 25]);
    let (status, _, _) = call(&app, "GET", "/v1/vlans?vid=0", None, &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[kartenn_testing::pg_test]
async fn occupancy_over_http(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let group = env
        .seed_group(
            "edge",
            json!({"domain": {"id": lab}}),
            (1, 10),
            &[(1, "active"), (2, "active"), (5, "reserved")],
        )
        .await;
    let app = env.app(env.admin().await);
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/v1/vlan-groups/{group}/occupancy"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["slices"],
        json!([
            {"first": 1, "last": 2, "state": "used"},
            {"first": 3, "last": 4, "state": "free"},
            {"first": 5, "last": 5, "state": "reserved"},
            {"first": 6, "last": 10, "state": "free"},
        ])
    );
    assert_eq!(
        (
            body["used"].clone(),
            body["reserved"].clone(),
            body["free"].clone()
        ),
        (json!(2), json!(1), json!(7))
    );
    let (_, _, body) = call(
        &app,
        "GET",
        &format!("/v1/vlan-groups/{group}/next-free"),
        None,
        &[],
    )
    .await;
    assert_eq!(body["vid"], 3);
    let (_, _, body) = call(
        &app,
        "GET",
        &format!("/v1/vlan-groups/{group}/next-free?from=5"),
        None,
        &[],
    )
    .await;
    assert_eq!(body["vid"], 6);
}

#[kartenn_testing::pg_test]
async fn next_free_404_when_exhausted(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let group = env
        .seed_group(
            "edge",
            json!({"domain": {"id": lab}}),
            (1, 2),
            &[(1, "active"), (2, "reserved")],
        )
        .await;
    let app = env.app(env.admin().await);
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/v1/vlan-groups/{group}/next-free"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["type"], "urn:kartenn:problem:exhausted", "{body}");
}

#[kartenn_testing::pg_test]
async fn prefixes_of_a_vlan_hide_unreadable(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (owner, acme, other) = (
        env.property("core.owner").await,
        env.id("entity", "acme").await,
        env.id("entity", "other").await,
    );
    let lab = env.id("addressing_domain", "lab").await;
    // A second domain, owned by `other`.
    env.applied(vec![
        Operation::CreateDomain {
            local: Some(1),
            slug: "far".into(),
        },
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Domain,
            property: Ref::Id(owner),
            values: vec![RawInput::Text(other.to_string())],
        },
    ])
    .await;
    let far = env.id("addressing_domain", "far").await;
    let group = env
        .seed_group(
            "edge",
            json!({"domain": {"id": lab}}),
            (1, 4094),
            &[(10, "active")],
        )
        .await;
    let vlan = env.vlan_of(group, 10).await;
    let vlan_property = env.property("vlan.vlan").await;
    let mut networks = Vec::new();
    for (domain, cidr) in [(lab, "10.0.0.0/24"), (far, "10.9.0.0/24")] {
        let create: Operation = serde_json::from_value(json!({
            "op": "create_prefix", "local": 1, "domain": {"id": domain}, "prefix": cidr,
            "nature": "network",
        }))
        .unwrap();
        env.applied(vec![
            create,
            Operation::AssignProperty {
                object: Ref::Local(1),
                kind: ObjectKind::Prefix,
                property: Ref::Id(vlan_property),
                values: vec![RawInput::Text(vlan.to_string())],
            },
        ])
        .await;
        networks.push(
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM prefix WHERE prefix = $1::cidr AND valid_period @> now()",
            )
            .bind(cidr)
            .fetch_one(&pool)
            .await
            .unwrap(),
        );
    }
    // Everything: both networks.
    let app = env.app(env.admin().await);
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/v1/vlans/{vlan}/prefixes"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["items"].as_array().unwrap().len(), 2, "{body}");
    // A reader of `acme`: the network of `far` is absent, and nothing counts it.
    let reader = env
        .person(&["vlan.read", "core.prefix.read"], Some(("entity", acme)))
        .await;
    let app = env.app(reader);
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/v1/vlans/{vlan}/prefixes"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [networks[0].to_string()], "{body}");
    assert_eq!(
        body.as_object().unwrap().len(),
        2,
        "items and links only: {body}"
    );
}

#[kartenn_testing::pg_test]
async fn writer_on_other_entity_gets_403(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (acme, other) = (
        env.id("entity", "acme").await,
        env.id("entity", "other").await,
    );
    let mine = env
        .seed_group("mine", json!({"entity": {"id": acme}}), (1, 4094), &[])
        .await;
    let theirs = env
        .seed_group("theirs", json!({"entity": {"id": other}}), (1, 4094), &[])
        .await;
    let writer = env
        .person(
            &["vlan.group.read", "vlan.read", "vlan.write"],
            Some(("entity", acme)),
        )
        .await;
    let app = env.app(writer);
    let (status, headers, body) = call(
        &app,
        "POST",
        "/v1/vlans",
        Some((
            "application/json",
            json!({"motive": "new", "group": {"id": mine}, "vid": 10, "name": "users"}),
        )),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let location = headers[header::LOCATION].to_str().unwrap();
    assert!(location.starts_with("/v1/vlans/"), "{location}");
    let (status, _, body) = call(
        &app,
        "POST",
        "/v1/vlans",
        Some((
            "application/json",
            json!({"motive": "new", "group": {"id": theirs}, "vid": 10, "name": "users"}),
        )),
        &[],
    )
    .await;
    // Refused as a write out of the scope of the writer: the changeset names the rule.
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(
        body["violations"][0]["rule"], "core.object.forbidden",
        "{body}"
    );
    // An unknown member is a malformed request.
    let (status, _, _) = call(
        &app,
        "POST",
        "/v1/vlans",
        Some((
            "application/json",
            json!({"motive": "new", "group": {"id": mine}, "vid": 11, "name": "x", "colour": 1}),
        )),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[kartenn_testing::pg_test]
async fn patch_needs_if_match(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env.id("addressing_domain", "lab").await;
    let group = env
        .seed_group(
            "edge",
            json!({"domain": {"id": lab}}),
            (1, 4094),
            &[(10, "active")],
        )
        .await;
    let vlan = env.vlan_of(group, 10).await;
    let app = env.app(env.admin().await);
    let uri = format!("/v1/vlans/{vlan}");
    let patch = || {
        Some((
            "application/merge-patch+json",
            json!({"motive": "rename", "name": "users"}),
        ))
    };
    let (status, _, _) = call(&app, "PATCH", &uri, patch(), &[]).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    let (status, _, _) = call(
        &app,
        "PATCH",
        &uri,
        patch(),
        &[("if-match", format!("\"{}\"", Uuid::now_v7()))],
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    let (status, headers, body) = call(&app, "GET", &uri, None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let etag = headers[header::ETAG].to_str().unwrap().to_owned();
    let (status, _, body) = call(&app, "PATCH", &uri, patch(), &[("if-match", etag.clone())]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, _, body) = call(&app, "GET", &uri, None, &[]).await;
    assert_eq!(body["name"], "users");
    // The version read before is stale now.
    let (status, _, _) = call(&app, "PATCH", &uri, patch(), &[("if-match", etag)]).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    // The closure goes the same way.
    let (_, headers, _) = call(&app, "GET", &uri, None, &[]).await;
    let etag = headers[header::ETAG].to_str().unwrap().to_owned();
    let (status, _, body) = call(
        &app,
        "DELETE",
        &uri,
        Some(("application/json", json!({"motive": "gone"}))),
        &[("if-match", etag)],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, _) = call(&app, "GET", &uri, None, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
