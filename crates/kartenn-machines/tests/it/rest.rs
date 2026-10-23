//! The routes of the module through the router (spec 009, REQ-009-15 to 18; task 009-10):
//! the interface that held an address at a date, the card of a machine, the machines of a
//! network under the rights of the reader, the edge from an address to its interface, the
//! `If-Match` of a patch, the canonical EUI-48.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::audit::Source;
use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_machines::Machines;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{ModuleDeclarations, core_declarations_for, sync_modules};
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use kartenn_vlan::Vlan;
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

struct Env {
    pool: PgPool,
    registry: Arc<ModuleRegistry>,
    store: PgStore,
    author: Uuid,
}

impl Env {
    /// Both modules migrated and declared, the entities `acme` and `other`, the domain
    /// `lab` owned by `acme` with the network 10.0.0.0/24.
    async fn new(pool: &PgPool) -> Self {
        let modules: Vec<Arc<dyn Module>> = vec![Arc::new(Vlan), Arc::new(Machines)];
        let registry = Arc::new(ModuleRegistry::new(modules).unwrap());
        kartenn_store_pg::modules::migrate(pool, &registry)
            .await
            .unwrap();
        let store = PgStore::new(pool.clone()).with_modules(registry.clone());
        let declarations: Vec<ModuleDeclarations> =
            std::iter::once(core_declarations_for(&registry))
                .chain(registry.modules().filter_map(|m| m.declarations()))
                .collect();
        for outcome in sync_modules(&store, &Unrestricted, &declarations, MAX_OPERATIONS)
            .await
            .unwrap()
        {
            assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        }
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let env = Self {
            pool: pool.clone(),
            registry,
            store,
            author,
        };
        env.applied(vec![
            Operation::CreateEntity {
                local: None,
                slug: "acme".into(),
                parent: None,
            },
            Operation::CreateEntity {
                local: None,
                slug: "other".into(),
                parent: None,
            },
        ])
        .await;
        let owner: Uuid =
            sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'core.owner'")
                .fetch_one(pool)
                .await
                .unwrap();
        let acme = env.entity("acme").await;
        env.applied(vec![
            Operation::CreateDomain {
                local: Some(1),
                slug: "lab".into(),
            },
            Operation::AssignProperty {
                object: Ref::Local(1),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(acme.to_string())],
            },
            Operation::CreatePrefix {
                local: None,
                domain: Ref::Local(1),
                prefix: "10.0.0.0/24".into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
        ])
        .await;
        env
    }

    async fn applied(&self, operations: Vec<Operation>) {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        let outcome = submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap();
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    async fn id(&self, sql: &str, value: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
            .bind(value)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn entity(&self, slug: &str) -> Uuid {
        self.id(
            "SELECT id FROM entity WHERE slug = $1 AND valid_period @> now()",
            slug,
        )
        .await
    }

    async fn domain(&self) -> Uuid {
        self.id(
            "SELECT id FROM addressing_domain WHERE slug = $1 AND valid_period @> now()",
            "lab",
        )
        .await
    }

    async fn network(&self) -> Uuid {
        self.id(
            "SELECT id FROM prefix WHERE prefix = $1::cidr AND valid_period @> now()",
            "10.0.0.0/24",
        )
        .await
    }

    /// The address `value` of `lab`, created now.
    async fn address(&self, value: &str) -> Uuid {
        self.applied(vec![Operation::CreateAddress {
            local: None,
            domain: Ref::Id(self.domain().await),
            address: value.into(),
            usage: AddressUsage::Host,
        }])
        .await;
        self.id(
            "SELECT id FROM address WHERE address = $1::inet AND valid_period @> now()",
            value,
        )
        .await
    }

    /// A physical machine `name` owned by `owner`, with its interface `eth0`: `(machine,
    /// interface)`.
    async fn machine(&self, name: &str, owner: Uuid) -> (Uuid, Uuid) {
        self.applied(vec![
            op(
                "create_machine",
                json!({"local": 1, "name": name, "kind": "physical", "owner": {"id": owner}}),
            ),
            op(
                "create_interface",
                json!({"machine": {"local": 1}, "name": "eth0"}),
            ),
        ])
        .await;
        let machine: Uuid = sqlx::query_scalar(
            "SELECT id FROM machines.machine
              WHERE name = $1 AND valid_period @> now() AND kartenn_owner(id, now()) = $2",
        )
        .bind(name)
        .bind(owner)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let interface = sqlx::query_scalar(
            "SELECT id FROM machines.interface WHERE machine_id = $1 AND valid_period @> now()",
        )
        .bind(machine)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        (machine, interface)
    }

    /// A person granted `entries` on `scope` (`None`: everywhere).
    async fn person(&self, entries: &[&str], scope: Option<Uuid>) -> Uuid {
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
        .bind(if scope.is_some() { "entity" } else { "global" })
        .bind(scope)
        .execute(pool)
        .await
        .unwrap();
        identity
    }

    /// Everything on the module, and the reading of the addressing and the VLANs.
    async fn admin(&self) -> Uuid {
        self.person(
            &[
                "machines.cluster.read",
                "machines.cluster.write",
                "machines.machine.read",
                "machines.machine.write",
                "machines.interface.read",
                "machines.interface.write",
                "core.address.read",
                "core.prefix.read",
                "vlan.vlan.read",
            ],
            None,
        )
        .await
    }

    /// The router of an instance with both modules, as the person `identity`.
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
            kartenn_rest::v1_with(
                api,
                vec![
                    kartenn_vlan::rest::routes(),
                    kartenn_machines::rest::routes(),
                ],
            ),
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
}

/// `machines.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

fn attach(interface: Uuid, address: Uuid) -> Operation {
    op(
        "attach_address",
        json!({"interface": {"id": interface}, "address": {"id": address}}),
    )
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

fn ids(body: &Value, field: &str) -> Vec<String> {
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i[field].as_str().unwrap().to_owned())
        .collect()
}

#[kartenn_testing::pg_test]
async fn interfaces_by_address_at_a_date(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let (first, eth_first) = env.machine("web1", acme).await;
    let (second, eth_second) = env.machine("web2", acme).await;
    let address = env.address("10.0.0.10").await;
    env.applied(vec![attach(eth_first, address)]).await;
    let between: time::OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    env.applied(vec![
        op(
            "detach_address",
            json!({"interface": {"id": eth_first}, "address": {"id": address}}),
        ),
        attach(eth_second, address),
    ])
    .await;
    let app = env.app(env.admin().await);
    let at = between
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/v1/interfaces?address={address}&valid_at={at}"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids(&body, "id"), [eth_first.to_string()]);
    assert_eq!(body["items"][0]["machine"]["id"], json!(first));
    let (_, _, now) = call(
        &app,
        "GET",
        &format!("/v1/interfaces?address={address}"),
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&now, "id"), [eth_second.to_string()]);
    assert_eq!(now["items"][0]["machine"]["id"], json!(second));
}

#[kartenn_testing::pg_test]
async fn machine_card_with_interfaces(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let (machine, eth0) = env.machine("db1", acme).await;
    env.applied(vec![op(
        "create_interface",
        json!({"machine": {"id": machine}, "name": "eth1", "mac": "02:00:00:00:00:01"}),
    )])
    .await;
    let address = env.address("10.0.0.20").await;
    env.applied(vec![attach(eth0, address)]).await;
    let app = env.app(env.admin().await);
    let (status, headers, body) =
        call(&app, "GET", &format!("/v1/machines/{machine}"), None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(headers.contains_key(header::ETAG));
    assert_eq!(body["name"], "db1");
    assert_eq!(body["owner"]["id"], json!(acme));
    let names: Vec<&str> = body["interfaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["eth0", "eth1"]);
    assert_eq!(
        body["interfaces"][0]["addresses"],
        json!([{"id": address, "value": "10.0.0.20", "primary": false}])
    );
    let (status, _, addresses) = call(
        &app,
        "GET",
        &format!("/v1/machines/{machine}/addresses"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{addresses}");
    assert_eq!(addresses["items"][0]["interface_name"], "eth0");
    assert_eq!(addresses["items"][0]["value"], "10.0.0.20");
}

#[kartenn_testing::pg_test]
async fn machines_filtered_by_owner(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let other = env.entity("other").await;
    env.machine("web2", acme).await;
    env.machine("db1", other).await;
    env.machine("web1", acme).await;
    let app = env.app(env.admin().await);
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/v1/machines?owner={acme}&kind=physical"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids(&body, "name"), ["web1", "web2"]);
    let (_, _, body) = call(
        &app,
        "GET",
        &format!("/v1/machines?owner={acme}&kind=virtual"),
        None,
        &[],
    )
    .await;
    assert!(ids(&body, "name").is_empty(), "{body}");
}

#[kartenn_testing::pg_test]
async fn machines_filtered_by_site(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    env.applied(vec![
        Operation::CreateSite {
            local: None,
            slug: "paris".into(),
            parent: None,
        },
        Operation::CreateSite {
            local: None,
            slug: "lyon".into(),
            parent: None,
        },
    ])
    .await;
    let site = |slug: &'static str| {
        env.id(
            "SELECT id FROM site WHERE slug = $1 AND valid_period @> now()",
            slug,
        )
    };
    let (paris, lyon) = (site("paris").await, site("lyon").await);
    for (name, site) in [
        ("web2", Some(paris)),
        ("db1", Some(lyon)),
        ("web1", Some(paris)),
        ("app1", None),
    ] {
        let mut fields = json!({"name": name, "kind": "physical", "owner": {"id": acme}});
        if let Some(site) = site {
            fields["site"] = json!({"id": site});
        }
        env.applied(vec![op("create_machine", fields)]).await;
    }
    let app = env.app(env.admin().await);
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/v1/machines?site={paris}"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids(&body, "name"), ["web1", "web2"]);
    let (_, _, body) = call(
        &app,
        "GET",
        &format!("/v1/machines?site={lyon}&owner={acme}"),
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&body, "name"), ["db1"], "{body}");
    let (_, _, body) = call(
        &app,
        "GET",
        &format!("/v1/machines?site={lyon}&kind=virtual"),
        None,
        &[],
    )
    .await;
    assert!(ids(&body, "name").is_empty(), "{body}");
}

#[kartenn_testing::pg_test]
async fn machines_of_a_network_hide_unreadable(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let other = env.entity("other").await;
    for n in 0..5 {
        let (_, interface) = env.machine(&format!("app{n}"), acme).await;
        let address = env.address(&format!("10.0.0.{}", 30 + n)).await;
        env.applied(vec![attach(interface, address)]).await;
    }
    let (_, foreign) = env.machine("hidden", other).await;
    let address = env.address("10.0.0.50").await;
    env.applied(vec![attach(foreign, address)]).await;
    // A machine of acme outside the network.
    env.machine("lonely", acme).await;
    let reader = env
        .person(&["machines.machine.read", "core.prefix.read"], Some(acme))
        .await;
    let app = env.app(reader);
    let network = env.network().await;
    let mut names = Vec::new();
    let mut uri = format!("/v1/machines?network={network}&page_size=2");
    loop {
        let (status, _, body) = call(&app, "GET", &uri, None, &[]).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        names.extend(ids(&body, "name"));
        match body["links"]["next"].as_str() {
            Some(next) => uri = next.to_owned(),
            None => break,
        }
    }
    assert_eq!(names, ["app0", "app1", "app2", "app3", "app4"]);
}

#[kartenn_testing::pg_test]
async fn graph_edge_address_to_interface(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let (_, interface) = env.machine("fw1", acme).await;
    let address = env.address("10.0.0.1").await;
    env.applied(vec![attach(interface, address)]).await;
    let app = env.app(env.admin().await);
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/v1/graph/edges?object={address}"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let edges = body["items"].as_array().unwrap();
    assert_eq!(edges.len(), 1, "{body}");
    assert_eq!(edges[0]["type"], "machines.has_address");
    assert_eq!(edges[0]["from"]["id"], json!(interface));
    assert_eq!(edges[0]["to"]["id"], json!(address));
}

#[kartenn_testing::pg_test]
async fn patch_interface_needs_if_match(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let (_, interface) = env.machine("sw1", acme).await;
    let app = env.app(env.admin().await);
    let patch = json!({"motive": "rename", "name": "uplink0"});
    let (status, _, body) = call(
        &app,
        "PATCH",
        &format!("/v1/interfaces/{interface}"),
        Some(("application/merge-patch+json", patch.clone())),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED, "{body}");
    let (_, headers, _) = call(
        &app,
        "GET",
        &format!("/v1/interfaces/{interface}"),
        None,
        &[],
    )
    .await;
    let etag = headers[header::ETAG].to_str().unwrap().to_owned();
    let (status, _, body) = call(
        &app,
        "PATCH",
        &format!("/v1/interfaces/{interface}"),
        Some(("application/merge-patch+json", patch.clone())),
        &[("If-Match", etag.clone())],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The version read is no longer the current one.
    let (status, _, body) = call(
        &app,
        "PATCH",
        &format!("/v1/interfaces/{interface}"),
        Some(("application/merge-patch+json", patch)),
        &[("If-Match", etag)],
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "{body}");
    let (_, _, read) = call(
        &app,
        "GET",
        &format!("/v1/interfaces/{interface}"),
        None,
        &[],
    )
    .await;
    assert_eq!(read["name"], "uplink0");
}

#[kartenn_testing::pg_test]
async fn mac_canonical_over_http(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let (machine, eth0) = env.machine("nas1", acme).await;
    let app = env.app(env.admin().await);
    let (status, headers, body) = call(
        &app,
        "POST",
        "/v1/interfaces",
        Some((
            "application/json",
            json!({"motive": "new port", "machine": {"id": machine}, "name": "eth1",
                   "mac": "02-AB-CD-EF-00-11"}),
        )),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let location = headers[header::LOCATION].to_str().unwrap().to_owned();
    let (status, _, interface) = call(&app, "GET", &location, None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{interface}");
    assert_eq!(interface["mac"], "02:ab:cd:ef:00:11");
    // The filter accepts any usual notation.
    let (_, _, found) = call(&app, "GET", "/v1/interfaces?mac=02ab.cdef.0011", None, &[]).await;
    assert_eq!(ids(&found, "name"), ["eth1"]);
    // An attachment through the route of the interface, under its version.
    let address = env.address("10.0.0.60").await;
    let (_, headers, _) = call(&app, "GET", &format!("/v1/interfaces/{eth0}"), None, &[]).await;
    let etag = headers[header::ETAG].to_str().unwrap().to_owned();
    let (status, _, body) = call(
        &app,
        "POST",
        &format!("/v1/interfaces/{eth0}/attachments"),
        Some((
            "application/json",
            json!({"motive": "service address", "address": {"id": address}, "primary": true}),
        )),
        &[("If-Match", etag)],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, _, held) = call(
        &app,
        "GET",
        &format!("/v1/interfaces?address={address}"),
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&held, "id"), [eth0.to_string()]);
}

/// The `ETag` of `GET uri`.
async fn etag_of(app: &Router, uri: &str) -> String {
    let (status, headers, body) = call(app, "GET", uri, None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    headers[header::ETAG].to_str().unwrap().to_owned()
}

/// The answer of `GET uri` under `If-None-Match: etag`.
async fn revalidated(app: &Router, uri: &str, etag: &str) -> (StatusCode, Value) {
    let (status, _, body) =
        call(app, "GET", uri, None, &[("If-None-Match", etag.to_owned())]).await;
    (status, body)
}

/// The card of a machine embeds its interfaces (REQ-009-15): renaming one changes the
/// body of the card, so its `ETag`, while a write of the machine under its old tag still
/// succeeds, its own version unchanged (REQ-006-14, 006/H19).
#[kartenn_testing::pg_test]
async fn a_renamed_interface_changes_the_etag_of_the_card(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let (machine, interface) = env.machine("db1", acme).await;
    let app = env.app(env.admin().await);
    let uri = format!("/v1/machines/{machine}");
    let etag = etag_of(&app, &uri).await;
    let iface = format!("/v1/interfaces/{interface}");
    let iface_etag = etag_of(&app, &iface).await;
    let (status, _, body) = call(
        &app,
        "PATCH",
        &iface,
        Some((
            "application/merge-patch+json",
            json!({"motive": "rename", "name": "uplink0"}),
        )),
        &[("If-Match", iface_etag)],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = revalidated(&app, &uri, &etag).await;
    assert_eq!(status, StatusCode::OK, "the card shows the new name");
    assert_eq!(body["interfaces"][0]["name"], "uplink0");
    let (status, _, body) = call(
        &app,
        "PATCH",
        &uri,
        Some((
            "application/merge-patch+json",
            json!({"motive": "rename", "name": "db2"}),
        )),
        &[("If-Match", etag.clone())],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, body) = call(
        &app,
        "PATCH",
        &uri,
        Some((
            "application/merge-patch+json",
            json!({"motive": "rename", "name": "db3"}),
        )),
        &[("If-Match", etag)],
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "{body}");
}

/// The card of a machine embeds its interfaces (REQ-009-15): a new interface changes
/// its body, so its `ETag` (REQ-006-14, 006/H19).
#[kartenn_testing::pg_test]
async fn a_new_interface_changes_the_etag_of_the_card(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let (machine, _) = env.machine("db1", acme).await;
    let app = env.app(env.admin().await);
    let uri = format!("/v1/machines/{machine}");
    let etag = etag_of(&app, &uri).await;
    let (status, _) = revalidated(&app, &uri, &etag).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    env.applied(vec![op(
        "create_interface",
        json!({"machine": {"id": machine}, "name": "eth1"}),
    )])
    .await;
    let (status, body) = revalidated(&app, &uri, &etag).await;
    assert_eq!(status, StatusCode::OK, "the card shows the new interface");
    assert_eq!(body["interfaces"].as_array().unwrap().len(), 2);
}
