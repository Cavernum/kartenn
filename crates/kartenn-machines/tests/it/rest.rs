//! The routes of the module through the router (spec 009, REQ-009-15 to 18; task 009-10):
//! the interface that held an address at a date, the card of a machine, the machines of a
//! network and the interfaces of a network with their addresses under the rights of the
//! reader, the edge from an address to its interface, the
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
use kartenn_service::operations::{ModuleKind, RawInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
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
        let modules: Vec<Arc<dyn Module>> = vec![Arc::new(Machines)];
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
            approval_self: true,
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
        self.grant(identity, entries, scope).await;
        identity
    }

    /// Grants `entries` to the person `identity` on `scope` (`None`: everywhere), by a
    /// role of their own.
    async fn grant(&self, identity: Uuid, entries: &[&str], scope: Option<Uuid>) {
        let kind = if scope.is_some() { "entity" } else { "global" };
        self.grant_on(identity, entries, kind, scope).await;
    }

    /// [`Self::grant`] on a scope of the kind `kind` (`entity`, `prefix`…).
    async fn grant_on(&self, identity: Uuid, entries: &[&str], kind: &str, scope: Option<Uuid>) {
        let pool = &self.pool;
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
            .bind(kind)
            .bind(scope)
            .execute(pool)
            .await
            .unwrap();
    }

    /// Everything on the module, and the reading of the addressing and the VLANs.
    async fn admin(&self) -> Uuid {
        self.person(
            &[
                "machines.cluster.read",
                "machines.cluster.edit",
                "machines.cluster.approval.self",
                "machines.read",
                "machines.edit",
                "machines.approval.self",
                "machines.interface.read",
                "machines.interface.edit",
                "machines.interface.approval.self",
                "core.address.read",
                "core.prefix.read",
                "core.vlan.read",
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
            kartenn_rest::v1_with(api, vec![kartenn_machines::rest::routes()]),
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
    let uri = kartenn_testing::applied_by_author(method, uri);
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

/// A machine and an interface read by identifier are read with their version in one
/// snapshot (task 021-11d): a write validated between the two reads is seen by neither,
/// so that the `version` names the value served.
#[kartenn_testing::pg_test]
async fn a_read_by_identifier_carries_the_version_of_the_value_served(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let (machine, eth0) = env.machine("db1", acme).await;
    let app = env.app(env.admin().await);
    for (id, uri) in [
        (machine.to_string(), format!("/v1/machines/{machine}")),
        (eth0.to_string(), format!("/v1/interfaces/{eth0}")),
    ] {
        let (_, _, before) = call(&app, "GET", &uri, None, &[]).await;
        let request = Request::get(&uri)
            .header(header::AUTHORIZATION, "Bearer test")
            .body(Body::empty())
            .unwrap();
        let (response, written) =
            kartenn_testing::across_a_write(&pool, &id, app.clone().oneshot(request)).await;
        let response = response.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let shown: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            shown["version"], before["version"],
            "{uri}: the version of the value"
        );
        let (_, _, after) = call(&app, "GET", &uri, None, &[]).await;
        assert_eq!(after["version"], written, "{uri}: the write is seen after");
    }
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

/// A28 (task 021-18f): `host:exists=false` keeps the machines without a host, the member
/// of a cluster among them (membership is not hosting), page by page under its cursor;
/// `true` the others; a `QUERY` says it as `exists`.
#[kartenn_testing::pg_test]
async fn machines_filtered_by_their_host_or_none(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let owner = json!({"id": acme});
    env.applied(vec![
        op(
            "create_cluster",
            json!({"local": 1, "slug": "c1", "owner": owner}),
        ),
        op(
            "create_machine",
            json!({"local": 2, "name": "host1", "kind": "physical", "owner": owner,
                   "member_of": {"local": 1}}),
        ),
        op(
            "create_machine",
            json!({"name": "vm1", "kind": "virtual", "owner": owner,
                   "host": {"machine": {"local": 2}}}),
        ),
        op(
            "create_machine",
            json!({"name": "vm2", "kind": "virtual", "owner": owner,
                   "host": {"cluster": {"local": 1}}}),
        ),
        op(
            "create_machine",
            json!({"name": "vm3", "kind": "virtual", "owner": owner}),
        ),
        op(
            "create_machine",
            json!({"name": "app1", "kind": "appliance", "owner": owner}),
        ),
    ])
    .await;
    let app = env.app(env.admin().await);
    let mut names = Vec::new();
    let mut uri = "/v1/machines?host:exists=false&page_size=2".to_owned();
    loop {
        let (status, _, body) = call(&app, "GET", &uri, None, &[]).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        names.extend(ids(&body, "name"));
        match body["links"]["next"].as_str() {
            Some(next) => uri = next.to_owned(),
            None => break,
        }
    }
    assert_eq!(names, ["app1", "host1", "vm3"]);
    let (_, _, body) = call(&app, "GET", "/v1/machines?host:exists=true", None, &[]).await;
    assert_eq!(ids(&body, "name"), ["vm1", "vm2"], "{body}");
    let (_, _, body) = call(
        &app,
        "GET",
        "/v1/machines?host:exists=false&kind=virtual",
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&body, "name"), ["vm3"], "{body}");
    let query = json!({"filters": [{"field": "host", "op": "exists", "value": false}]});
    let (status, _, body) = call(
        &app,
        "QUERY",
        "/v1/machines",
        Some(("application/json", query)),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids(&body, "name"), ["app1", "host1", "vm3"]);
    let (status, _, body) = call(&app, "GET", "/v1/machines?host:exists=none", None, &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
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
        .person(
            &["machines.read", "core.prefix.read", "core.address.read"],
            Some(acme),
        )
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

/// Every page of `uri`, its links followed: the items in the order served.
async fn every_page(app: &Router, uri: &str) -> Vec<Value> {
    let mut items = Vec::new();
    let mut uri = uri.to_owned();
    loop {
        let (status, _, body) = call(app, "GET", &uri, None, &[]).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        items.extend(body["items"].as_array().unwrap().iter().cloned());
        match body["links"]["next"].as_str() {
            Some(next) => next.clone_into(&mut uri),
            None => break,
        }
    }
    items
}

/// The values of the addresses of each interface of `items`, by interface.
fn held_values(items: &[Value]) -> std::collections::BTreeMap<String, Vec<String>> {
    items
        .iter()
        .map(|i| {
            let values = i["addresses"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a["value"].as_str().unwrap().to_owned())
                .collect();
            (i["id"].as_str().unwrap().to_owned(), values)
        })
        .collect()
}

/// 021/A28: `/v1/interfaces?network=` serves the interfaces holding an address of the
/// network that the caller may read, each with those addresses only, by pages; an
/// interface or an address the caller may not read is not served; the filter is given
/// once.
#[kartenn_testing::pg_test]
async fn interfaces_of_a_network_with_their_addresses(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let other = env.entity("other").await;
    env.applied(vec![Operation::CreatePrefix {
        local: None,
        domain: Ref::Id(env.domain().await),
        prefix: "10.0.1.0/24".into(),
        nature: Nature::Network,
        point_to_point: false,
        anycast: false,
    }])
    .await;
    // web1: two addresses of the network and one outside; an interface without address.
    let (web1, web1_eth0) = env.machine("web1", acme).await;
    let first = env.address("10.0.0.11").await;
    let second = env.address("10.0.0.10").await;
    let outside = env.address("10.0.1.5").await;
    env.applied(vec![
        attach(web1_eth0, first),
        attach(web1_eth0, second),
        attach(web1_eth0, outside),
        op(
            "create_interface",
            json!({"machine": {"id": web1}, "name": "eth1"}),
        ),
    ])
    .await;
    // An interface of `other`, which a reader of acme may not read.
    let (_, hidden_eth0) = env.machine("hidden", other).await;
    let hidden_address = env.address("10.0.0.13").await;
    env.applied(vec![attach(hidden_eth0, hidden_address)]).await;
    let mut expected = std::collections::BTreeMap::new();
    expected.insert(
        web1_eth0.to_string(),
        vec!["10.0.0.10".to_owned(), "10.0.0.11".to_owned()],
    );
    for n in 0..4 {
        let (_, interface) = env.machine(&format!("app{n}"), acme).await;
        let value = format!("10.0.0.{}", 20 + n);
        let address = env.address(&value).await;
        env.applied(vec![attach(interface, address)]).await;
        expected.insert(interface.to_string(), vec![value]);
    }
    let network = env.network().await;
    let uri = format!("/v1/interfaces?network={network}&page_size=2");

    let reader = env
        .person(
            &[
                "machines.interface.read",
                "core.address.read",
                "core.prefix.read",
            ],
            Some(acme),
        )
        .await;
    let app = env.app(reader);
    let items = every_page(&app, &uri).await;
    assert_eq!(held_values(&items), expected);
    assert_eq!(items.len(), 5, "each interface once");
    // The addresses served by the last page are journaled as read.
    let journaled: Vec<Uuid> = sqlx::query_scalar(
        "SELECT target_ids FROM audit.event
          WHERE name = 'data.objects.read' AND target_kind = 'address'
          ORDER BY seq DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let last: Vec<Uuid> = items[4]["addresses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert_eq!(journaled, last);
    let web1_item = items.iter().find(|i| i["id"] == json!(web1_eth0)).unwrap();
    assert_eq!(
        web1_item["addresses"],
        json!([
            {"id": second, "value": "10.0.0.10", "primary": false},
            {"id": first, "value": "10.0.0.11", "primary": false},
        ])
    );
    assert_eq!(web1_item["machine"]["id"], json!(web1));
    assert!(web1_item["version"].is_string(), "{web1_item}");
    // Without `network`, the interfaces carry no addresses.
    let (_, _, body) = call(&app, "GET", "/v1/interfaces?name=eth1", None, &[]).await;
    assert_eq!(ids(&body, "name"), ["eth1"]);
    assert!(body["items"][0].get("addresses").is_none(), "{body}");

    // Whoever reads everything sees the interface of `other` as well.
    let app = env.app(env.admin().await);
    let all = held_values(&every_page(&app, &uri).await);
    assert_eq!(all[&hidden_eth0.to_string()], ["10.0.0.13"]);
    assert_eq!(all.len(), 6);

    // Without the reading of the interfaces, or of the addresses: nothing. The interfaces
    // read on acme and the addresses on `other` only: those of `lab`, owned by acme, are
    // not read, nor are the interfaces that hold them.
    for entries in [
        &["core.address.read", "core.prefix.read"][..],
        &["machines.interface.read", "core.prefix.read"][..],
    ] {
        let app = env.app(env.person(entries, Some(acme)).await);
        assert!(every_page(&app, &uri).await.is_empty(), "{entries:?}");
    }
    let elsewhere = env
        .person(&["machines.interface.read", "core.prefix.read"], Some(acme))
        .await;
    env.grant(elsewhere, &["core.address.read"], Some(other))
        .await;
    let app = env.app(elsewhere);
    assert!(every_page(&app, &uri).await.is_empty());
    let (_, _, body) = call(&app, "GET", "/v1/interfaces?name=eth1", None, &[]).await;
    assert_eq!(ids(&body, "name"), ["eth1"], "the interfaces are read");

    // The filter once, an identifier.
    let app = env.app(reader);
    for wrong in [
        format!("/v1/interfaces?network={network}&network={network}"),
        "/v1/interfaces?network=lab".to_owned(),
    ] {
        let (status, _, body) = call(&app, "GET", &wrong, None, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{wrong}: {body}");
    }
}

/// `items` and `links` of `GET uri`, what tells two lists apart.
async fn listed(app: &Router, uri: &str) -> Value {
    let (status, _, body) = call(app, "GET", uri, None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    json!({"items": body["items"], "links": body["links"]})
}

/// ADR 0021 on the lists by network (task 021-18e): of an interface the caller reads, only
/// the addresses it reads are served, and a machine counts only by those; a network the
/// caller may not read answers as an unknown one, on the interfaces and on the machines.
#[kartenn_testing::pg_test]
async fn a_list_by_network_serves_only_what_the_caller_reads(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    // web1 holds an address the reader reads and one it does not; web2 only the latter.
    let (web1, web1_eth0) = env.machine("web1", acme).await;
    let read = env.address("10.0.0.10").await;
    let unread = env.address("10.0.0.20").await;
    let (_, web2_eth0) = env.machine("web2", acme).await;
    let unread_too = env.address("10.0.0.30").await;
    env.applied(vec![
        attach(web1_eth0, read),
        attach(web1_eth0, unread),
        attach(web2_eth0, unread_too),
    ])
    .await;
    // The reader reads the addresses of 10.0.0.8/29 only: a prefix scope narrower than
    // the network, written in the base (the tree keeps a network a leaf; the scope reads
    // the prefix by its identifier and value alone).
    let narrow = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO prefix (id, domain_id, prefix, nature, public, registry_version,
                             valid_period, changeset_id)
         VALUES ($1, $2, '10.0.0.8/29', 'network', false, 'test',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(narrow)
    .bind(env.domain().await)
    .execute(&pool)
    .await
    .unwrap();
    let reader = env
        .person(
            &[
                "machines.read",
                "machines.interface.read",
                "core.prefix.read",
            ],
            None,
        )
        .await;
    env.grant_on(reader, &["core.address.read"], "prefix", Some(narrow))
        .await;
    let app = env.app(reader);
    let network = env.network().await;
    let body = listed(&app, &format!("/v1/interfaces?network={network}")).await;
    assert_eq!(ids(&body, "id"), [web1_eth0.to_string()]);
    assert_eq!(
        body["items"][0]["addresses"],
        json!([{"id": read, "value": "10.0.0.10", "primary": false}])
    );
    let body = listed(&app, &format!("/v1/machines?network={network}")).await;
    assert_eq!(ids(&body, "id"), [web1.to_string()]);

    // A network it may not read: the answer of an unknown one, on both lists.
    let blind = env
        .person(
            &[
                "machines.read",
                "machines.interface.read",
                "core.address.read",
            ],
            None,
        )
        .await;
    let app = env.app(blind);
    let unknown = Uuid::now_v7();
    for collection in ["interfaces", "machines"] {
        let hidden = listed(&app, &format!("/v1/{collection}?network={network}")).await;
        let absent = listed(&app, &format!("/v1/{collection}?network={unknown}")).await;
        assert_eq!(hidden, absent, "{collection}");
        assert_eq!(hidden["items"], json!([]), "{collection}");
    }
}

/// REQ-018-23: `q` suggests by prefix, ignoring case, only the objects the caller may
/// read, in the core and in the modules; the cursor keeps it.
#[kartenn_testing::pg_test]
async fn a_suggestion_never_shows_an_unreadable_object(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let other = env.entity("other").await;
    for (name, owner) in [
        ("web2", acme),
        ("webx", other),
        ("db1", acme),
        ("web1", acme),
    ] {
        env.machine(name, owner).await;
    }
    env.address("10.0.0.31").await;
    let owner: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'core.owner'")
            .fetch_one(&pool)
            .await
            .unwrap();
    env.applied(vec![
        Operation::CreateDomain {
            local: Some(1),
            slug: "lan".into(),
        },
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Domain,
            property: Ref::Id(owner),
            values: vec![RawInput::Text(other.to_string())],
        },
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Local(1),
            prefix: "10.0.0.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
        Operation::CreateAddress {
            local: None,
            domain: Ref::Local(1),
            address: "10.0.0.32".into(),
            usage: AddressUsage::Host,
        },
    ])
    .await;
    let entries = [
        "machines.read",
        "machines.interface.read",
        "core.entity.read",
        "core.domain.read",
        "core.prefix.read",
        "core.address.read",
    ];
    let reader = env.app(env.person(&entries, Some(acme)).await);
    let everyone = env.app(env.person(&entries, None).await);
    let found = |app: Router, uri: String, field: &'static str| async move {
        let (status, _, body) = call(&app, "GET", &uri, None, &[]).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        ids(&body, field)
    };
    let cases: [(&str, &str, &[&str], &[&str]); 5] = [
        (
            "machines?q=WEB",
            "name",
            &["web1", "web2"],
            &["web1", "web2", "webx"],
        ),
        (
            "addresses?q=10.0.0.3",
            "value",
            &["10.0.0.31"],
            &["10.0.0.31", "10.0.0.32"],
        ),
        ("domains?q=L", "slug", &["lab"], &["lab", "lan"]),
        ("entities?q=o", "slug", &[], &["other"]),
        (
            "prefixes?q=10.0.0",
            "value",
            &["10.0.0.0/24"],
            &["10.0.0.0/24", "10.0.0.0/24"],
        ),
    ];
    for (uri, field, read, all) in cases {
        let uri = format!("/v1/{uri}");
        assert_eq!(
            found(reader.clone(), uri.clone(), field).await,
            read,
            "{uri}"
        );
        assert_eq!(
            found(everyone.clone(), uri.clone(), field).await,
            all,
            "{uri}"
        );
    }
    let (_, _, interfaces) = call(&reader, "GET", "/v1/interfaces?q=ETH", None, &[]).await;
    assert_eq!(
        ids(&interfaces, "name"),
        ["eth0", "eth0", "eth0"],
        "{interfaces}"
    );
    let (_, _, first) = call(
        &everyone,
        "GET",
        "/v1/machines?q=web&page_size=2",
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&first, "name"), ["web1", "web2"]);
    let next = first["links"]["next"].as_str().unwrap();
    let (_, _, second) = call(&everyone, "GET", next, None, &[]).await;
    assert_eq!(ids(&second, "name"), ["webx"], "the cursor keeps q");
    // Addresses suggested in the order of their text, page after page.
    let uri = "/v1/addresses?q=10.0.0.3&page_size=1";
    let (_, _, first) = call(&everyone, "GET", uri, None, &[]).await;
    assert_eq!(ids(&first, "value"), ["10.0.0.31"]);
    let next = first["links"]["next"].as_str().unwrap();
    let (_, _, second) = call(&everyone, "GET", next, None, &[]).await;
    assert_eq!(ids(&second, "value"), ["10.0.0.32"], "{second}");
    assert!(second["links"]["next"].is_null(), "{second}");
    let (status, _, _) = call(&reader, "GET", "/v1/ranges?q=10", None, &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a range is no reference");
    let uri = "/v1/prefixes?q=10&domain=lab&prop.core.site:exists=true";
    let (status, _, _) = call(&reader, "GET", uri, None, &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "by natural key only");
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
/// REQ-009-26: the medium of an interface, read with its properties and its explanation,
/// filters the interfaces.
#[kartenn_testing::pg_test]
async fn interfaces_filtered_by_media(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let (machine, eth0) = env.machine("sw1", acme).await;
    env.applied(vec![op(
        "create_interface",
        json!({"machine": {"id": machine}, "name": "eth2"}),
    )])
    .await;
    let eth2: Uuid = sqlx::query_scalar(
        "SELECT id FROM machines.interface
          WHERE machine_id = $1 AND name = 'eth2' AND valid_period @> now()",
    )
    .bind(machine)
    .fetch_one(&pool)
    .await
    .unwrap();
    let media: Uuid = sqlx::query_scalar(
        "SELECT id FROM property_declaration WHERE key = 'machines.interface_media'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let assign = |object: Uuid, value: &str| Operation::AssignProperty {
        object: Ref::Id(object),
        kind: ObjectKind::Module(ModuleKind::parse("machines.interface").unwrap()),
        property: Ref::Id(media),
        values: vec![RawInput::Text(value.into())],
    };
    env.applied(vec![assign(eth0, "ethernet"), assign(eth2, "sfp")])
        .await;
    let app = env.app(env.admin().await);
    let (status, _, body) = call(&app, "GET", "/v1/interfaces?media=sfp", None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids(&body, "name"), ["eth2"]);
    let (_, _, body) = call(
        &app,
        "GET",
        &format!("/v1/interfaces?media=ethernet&machine={machine}"),
        None,
        &[],
    )
    .await;
    assert_eq!(ids(&body, "name"), ["eth0"]);
    let (_, _, body) = call(&app, "GET", "/v1/interfaces?media=wifi", None, &[]).await;
    assert!(ids(&body, "name").is_empty(), "{body}");
    // The card of the interface shows it, and its explanation.
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/v1/interfaces/{eth2}/properties"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["properties"]["machines.interface_media"]
            .to_string()
            .contains("sfp"),
        "{body}"
    );
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/v1/interfaces/{eth2}/properties/machines.interface_media/explanation"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.to_string().contains("sfp"), "{body}");
}

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

/// REQ-019-1 (task 019-2): `hypervisor=true` keeps the physical machines that host a
/// virtual machine or are members of a cluster, at the dates of the read: a host leaves the
/// list when its virtual machine moves away. A container makes no hypervisor, and a guest
/// the caller may not read does not count; `false` keeps the others; a `QUERY` says it as an
/// equality.
#[kartenn_testing::pg_test]
async fn hypervisor_follows_hosting(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let other = env.entity("other").await;
    let owner = json!({"id": acme});
    env.applied(vec![
        op(
            "create_cluster",
            json!({"local": 1, "slug": "c1", "owner": owner}),
        ),
        op(
            "create_machine",
            json!({"local": 2, "name": "h1", "kind": "physical", "owner": owner}),
        ),
        op(
            "create_machine",
            json!({"name": "h2", "kind": "physical", "owner": owner, "member_of": {"local": 1}}),
        ),
        op(
            "create_machine",
            json!({"local": 3, "name": "h3", "kind": "physical", "owner": owner}),
        ),
        op(
            "create_machine",
            json!({"local": 4, "name": "h4", "kind": "physical", "owner": owner}),
        ),
        op(
            "create_machine",
            json!({"name": "vm1", "kind": "virtual", "owner": owner,
                   "host": {"machine": {"local": 2}}}),
        ),
        op(
            "create_machine",
            json!({"name": "vm2", "kind": "virtual", "owner": owner,
                   "host": {"cluster": {"local": 1}}}),
        ),
        op(
            "create_machine",
            json!({"name": "ct1", "kind": "container", "owner": owner,
                   "host": {"machine": {"local": 3}}}),
        ),
        op(
            "create_machine",
            json!({"name": "hidden", "kind": "virtual", "owner": {"id": other},
                   "host": {"machine": {"local": 4}}}),
        ),
        op(
            "create_machine",
            json!({"name": "app1", "kind": "appliance", "owner": owner}),
        ),
    ])
    .await;
    let admin = env.app(env.admin().await);
    let names = |body: &Value| ids(body, "name");
    let (status, _, body) = call(&admin, "GET", "/v1/machines?hypervisor=true", None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(names(&body), ["h1", "h2", "h4"]);
    let (_, _, body) = call(&admin, "GET", "/v1/machines?hypervisor=false", None, &[]).await;
    assert_eq!(
        names(&body),
        ["app1", "ct1", "h3", "hidden", "vm1", "vm2"],
        "{body}"
    );
    // A reader of acme does not see the guest of `other`: `h4` hosts nothing for them.
    let reader = env.person(&["machines.read"], Some(acme)).await;
    let (_, _, body) = call(
        &env.app(reader),
        "GET",
        "/v1/machines?hypervisor=true",
        None,
        &[],
    )
    .await;
    assert_eq!(names(&body), ["h1", "h2"], "{body}");
    // The virtual machine moves from h1 to h3.
    let before: time::OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let vm1 = env
        .id(
            "SELECT id FROM machines.machine WHERE name = $1 AND valid_period @> now()",
            "vm1",
        )
        .await;
    let h3 = env
        .id(
            "SELECT id FROM machines.machine WHERE name = $1 AND valid_period @> now()",
            "h3",
        )
        .await;
    env.applied(vec![op(
        "update_machine",
        json!({"id": {"id": vm1}, "host": {"machine": {"id": h3}}}),
    )])
    .await;
    let (_, _, body) = call(&admin, "GET", "/v1/machines?hypervisor=true", None, &[]).await;
    assert_eq!(names(&body), ["h2", "h3", "h4"], "{body}");
    let at = before
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let (status, _, body) = call(
        &admin,
        "GET",
        &format!("/v1/machines?hypervisor=true&valid_at={at}"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(names(&body), ["h1", "h2", "h4"], "{body}");
    // In conjunction with the other filters, and in a `QUERY`.
    let (_, _, body) = call(
        &admin,
        "GET",
        "/v1/machines?hypervisor=false&kind=physical",
        None,
        &[],
    )
    .await;
    assert_eq!(names(&body), ["h1"], "{body}");
    let query = json!({"filters": [{"field": "hypervisor", "value": true}]});
    let (status, _, body) = call(
        &admin,
        "QUERY",
        "/v1/machines",
        Some(("application/json", query)),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(names(&body), ["h2", "h3", "h4"]);
    let (status, _, body) = call(&admin, "GET", "/v1/machines?hypervisor=yes", None, &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

/// The status, the headers but the identifier of the request, and the bytes of the body
/// of `GET uri`.
async fn answer(app: &Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let request = Request::builder()
        .uri(uri)
        .header(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer test"),
        )
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let mut headers = response.headers().clone();
    assert!(headers.remove("request-id").is_some());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}

/// `QUERY /v1/{collection}` filtered by the values `values` of `field`.
async fn query_values(
    app: &Router,
    collection: &str,
    field: &str,
    values: &[Uuid],
) -> (StatusCode, Value) {
    let values: Vec<String> = values.iter().map(Uuid::to_string).collect();
    let query = json!({"filters": [{"field": field, "value": values}]});
    let (status, _, body) = call(
        app,
        "QUERY",
        &format!("/v1/{collection}"),
        Some(("application/json", query)),
        &[],
    )
    .await;
    (status, body)
}

/// The filter `id` of the clusters, the machines and the interfaces (REQ-006-41, task
/// 006-32b): the objects among the identifiers that the caller may read, in conjunction with
/// the other filters, in one page without a next one; an unreadable one is absent as an
/// unknown one, headers and bytes included.
#[kartenn_testing::pg_test]
async fn clusters_machines_and_interfaces_filtered_by_identifier(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (acme, other) = (env.entity("acme").await, env.entity("other").await);
    env.applied(vec![
        op(
            "create_cluster",
            json!({"slug": "c1", "owner": {"id": acme}}),
        ),
        op(
            "create_cluster",
            json!({"slug": "c2", "owner": {"id": other}}),
        ),
    ])
    .await;
    let cluster = "SELECT id FROM machines.cluster WHERE slug = $1 AND valid_period @> now()";
    let (c1, c2) = (env.id(cluster, "c1").await, env.id(cluster, "c2").await);
    let (web1, eth1) = env.machine("web1", acme).await;
    let (web2, eth2) = env.machine("web2", acme).await;
    let (webx, ethx) = env.machine("webx", other).await;
    let reader = env
        .person(
            &[
                "machines.cluster.read",
                "machines.read",
                "machines.interface.read",
            ],
            Some(acme),
        )
        .await;
    let app = env.app(reader);
    let unknown = Uuid::now_v7();
    for (uri, field, expected) in [
        (format!("clusters?id={c2}&id={c1}"), "id", vec![c1]),
        (
            format!("clusters?id={unknown}&id={c1}&slug=c1"),
            "id",
            vec![c1],
        ),
        (format!("clusters?id={c1}&slug=c2"), "id", vec![]),
        (
            format!("machines?id={webx}&id={web2}&id={web1}&id={web1}"),
            "id",
            vec![web1, web2],
        ),
        (
            format!("machines?id={web1}&id={web2}&name=web2"),
            "id",
            vec![web2],
        ),
        (format!("interfaces?id={ethx}&id={eth1}"), "id", vec![eth1]),
        (
            format!("interfaces?id={eth1}&id={eth2}&machine={web2}"),
            "id",
            vec![eth2],
        ),
    ] {
        let (status, _, body) = call(&app, "GET", &format!("/v1/{uri}"), None, &[]).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        let expected: Vec<String> = expected.iter().map(Uuid::to_string).collect();
        assert_eq!(ids(&body, field), expected, "{uri}");
    }
    // The unreadable object leaves the answer of an unknown identifier, in one page.
    for (collection, unreadable, known) in [
        ("clusters", c2, c1),
        ("machines", webx, web1),
        ("interfaces", ethx, eth1),
    ] {
        let mut answers = Vec::new();
        for absent in [unknown, unreadable] {
            let uri = format!("/v1/{collection}?id={absent}&id={known}");
            answers.push(answer(&app, &uri).await);
        }
        assert_eq!(
            answers[0], answers[1],
            "{collection}: unknown and unreadable"
        );
        assert_eq!(answers[0].0, StatusCode::OK, "{collection}");
        assert!(answers[0].1.get("link").is_none(), "{collection}");
    }
    // Past the default size of a page, one page without a next one.
    let many: Vec<Operation> = (0..101)
        .map(|n| {
            op(
                "create_machine",
                json!({"name": format!("m{n:03}"), "kind": "physical", "owner": {"id": acme}}),
            )
        })
        .collect();
    env.applied(many).await;
    let wanted: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM machines.machine WHERE name LIKE 'm%' AND valid_period @> now()",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let (status, body) = query_values(&app, "machines", "id", &wanted).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["items"].as_array().unwrap().len(), 101);
    assert!(body["links"]["next"].is_null(), "{}", body["links"]);
    // `page_size` or a cursor beside `id`, or an identifier that is not one → 400 naming it.
    let (_, _, first) = call(&app, "GET", "/v1/machines?page_size=1", None, &[]).await;
    let next = first["links"]["next"].as_str().unwrap().to_owned();
    for (uri, named) in [
        (format!("/v1/clusters?id={c1}&page_size=5"), "page_size"),
        (format!("/v1/machines?id={web1}&page_size=5"), "page_size"),
        (format!("/v1/interfaces?id={eth1}&page_size=5"), "page_size"),
        (format!("{next}&id={web1}"), "id"),
        ("/v1/clusters?id=c1".to_owned(), "id"),
    ] {
        let (status, _, body) = call(&app, "GET", &uri, None, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert!(body["detail"].to_string().contains(named), "{uri}: {body}");
    }
}

/// The filter `address`, repeated, of the interfaces (REQ-006-41, task 006-32b): the
/// interfaces that hold one of the addresses, a shared one on each of its interfaces, in
/// conjunction with the other filters, paged as any list, its cursor carrying the
/// addresses; 64 addresses at most, so that the link to the next page stays short.
#[kartenn_testing::pg_test]
async fn interfaces_filtered_by_several_addresses(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let (_, eth1) = env.machine("web1", acme).await;
    let (web2, eth2) = env.machine("web2", acme).await;
    let (_, eth3) = env.machine("web3", acme).await;
    let (first, second) = (
        env.address("10.0.0.10").await,
        env.address("10.0.0.20").await,
    );
    let third = env.address("10.0.0.30").await;
    env.applied(vec![Operation::CreateAddress {
        local: None,
        domain: Ref::Id(env.domain().await),
        address: "10.0.0.40".into(),
        usage: AddressUsage::Vip,
    }])
    .await;
    let shared = env
        .id(
            "SELECT id FROM address WHERE address = $1::inet AND valid_period @> now()",
            "10.0.0.40",
        )
        .await;
    env.applied(vec![
        attach(eth1, first),
        attach(eth2, second),
        attach(eth3, third),
        attach(eth1, shared),
        attach(eth3, shared),
    ])
    .await;
    let app = env.app(env.admin().await);
    let unknown = Uuid::now_v7();
    let sorted = |mut found: Vec<String>| {
        found.sort();
        found
    };
    let set = |wanted: &[Uuid]| sorted(wanted.iter().map(Uuid::to_string).collect());
    for (uri, expected) in [
        (
            format!("address={first}&address={second}"),
            set(&[eth1, eth2]),
        ),
        (format!("address={unknown}&address={first}"), set(&[eth1])),
        (format!("address={shared}"), set(&[eth1, eth3])),
        (
            format!("address={first}&address={second}&machine={web2}"),
            set(&[eth2]),
        ),
        (
            format!("address={first}&address={second}&id={eth1}&id={eth3}"),
            set(&[eth1]),
        ),
    ] {
        let uri = format!("/v1/interfaces?{uri}");
        let (status, _, body) = call(&app, "GET", &uri, None, &[]).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_eq!(sorted(ids(&body, "id")), expected, "{uri}");
    }
    // A shared address held by more interfaces than a page is read whole, page after page.
    let items = every_page(
        &app,
        &format!("/v1/interfaces?address={shared}&page_size=1"),
    )
    .await;
    let found = items.iter().map(|i| i["id"].as_str().unwrap().to_owned());
    assert_eq!(sorted(found.collect()), set(&[eth1, eth3]));
    // 64 addresses, two of them held, on two pages followed to the end: the cursor carries
    // the addresses, its link to the next page under 6 KB.
    let mut wanted: Vec<Uuid> = (0..62).map(|_| Uuid::now_v7()).collect();
    wanted.extend([first, second]);
    let repeated = |values: &[Uuid]| -> String {
        values
            .iter()
            .map(|a| format!("&address={a}"))
            .collect::<Vec<_>>()
            .concat()
    };
    let query = repeated(&wanted);
    let uri = format!("/v1/interfaces?page_size=1{query}");
    let (status, headers, body) = call(&app, "GET", &uri, None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids(&body, "id").len(), 1);
    let link = headers.get("link").expect("a link to the next page");
    assert!(link.len() < 6 * 1024, "{} bytes", link.len());
    let items = every_page(&app, &uri).await;
    let found = items.iter().map(|i| i["id"].as_str().unwrap().to_owned());
    assert_eq!(sorted(found.collect()), set(&[eth1, eth2]));
    // A 65th address, or a value that is not an identifier → 400 naming `address`.
    wanted.push(Uuid::now_v7());
    let query = repeated(&wanted);
    for (uri, detail) in [
        (
            format!("/v1/interfaces?page_size=1{query}"),
            "address is given more than 64 times",
        ),
        (
            "/v1/interfaces?address=10.0.0.10".to_owned(),
            "address is not an identifier",
        ),
    ] {
        let (status, _, body) = call(&app, "GET", &uri, None, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert!(body["detail"].to_string().contains(detail), "{uri}: {body}");
    }
}

/// The shared (`vip`) address `value` of `lab`, created now: several interfaces may hold it.
async fn vip(env: &Env, value: &str) -> Uuid {
    env.applied(vec![Operation::CreateAddress {
        local: None,
        domain: Ref::Id(env.domain().await),
        address: value.into(),
        usage: AddressUsage::Vip,
    }])
    .await;
    env.id(
        "SELECT id FROM address WHERE address = $1::inet AND valid_period @> now()",
        value,
    )
    .await
}

/// Under the filter `address` (REQ-006-41, task 006-32e, 021/A28 amended), each interface
/// carries `addresses`: those of the requested addresses it holds that the caller reads,
/// and those only, the same on every page; with `network` as well, those that are both
/// requested and in the network.
#[kartenn_testing::pg_test]
async fn interfaces_filtered_by_address_carry_the_requested_ones(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    env.applied(vec![Operation::CreatePrefix {
        local: None,
        domain: Ref::Id(env.domain().await),
        prefix: "10.0.1.0/24".into(),
        nature: Nature::Network,
        point_to_point: false,
        anycast: false,
    }])
    .await;
    let (_, eth1) = env.machine("web1", acme).await;
    let (_, eth2) = env.machine("web2", acme).await;
    let (_, eth3) = env.machine("web3", acme).await;
    let (_, eth4) = env.machine("web4", acme).await;
    let first = vip(&env, "10.0.0.17").await;
    let second = vip(&env, "10.0.0.18").await;
    let extra = env.address("10.0.0.19").await;
    let hidden = vip(&env, "10.0.0.40").await;
    let elsewhere = env.address("10.0.1.5").await;
    // web1 holds both requested addresses, one not requested and one of the other network;
    // web2 and web3 one each, web2 also the address the reader does not read; web4 only that.
    env.applied(vec![
        attach(eth1, first),
        attach(eth1, second),
        attach(eth1, extra),
        attach(eth1, elsewhere),
        attach(eth2, first),
        attach(eth2, hidden),
        attach(eth3, second),
        attach(eth4, hidden),
    ])
    .await;
    let map = |pairs: &[(Uuid, &[&str])]| -> std::collections::BTreeMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(i, v)| (i.to_string(), v.iter().map(|&s| s.to_owned()).collect()))
            .collect()
    };
    let admin = env.app(env.admin().await);
    let both = format!("/v1/interfaces?address={first}&address={second}");
    let expected = map(&[
        (eth1, &["10.0.0.17", "10.0.0.18"]),
        (eth2, &["10.0.0.17"]),
        (eth3, &["10.0.0.18"]),
    ]);
    // In one page, then page by page: the same addresses on each interface.
    let (status, _, body) = call(&admin, "GET", &both, None, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(held_values(body["items"].as_array().unwrap()), expected);
    let items = every_page(&admin, &format!("{both}&page_size=1")).await;
    assert_eq!(items.len(), 3, "each interface once");
    assert_eq!(held_values(&items), expected);
    let eth1_item = items.iter().find(|i| i["id"] == json!(eth1)).unwrap();
    assert_eq!(
        eth1_item["addresses"],
        json!([
            {"id": first, "value": "10.0.0.17", "primary": false},
            {"id": second, "value": "10.0.0.18", "primary": false},
        ])
    );
    // The addresses served by the last page are journaled as read.
    let journaled: Vec<Uuid> = sqlx::query_scalar(
        "SELECT target_ids FROM audit.event
          WHERE name = 'data.objects.read' AND target_kind = 'address'
          ORDER BY seq DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let last: Vec<Uuid> = items[2]["addresses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert_eq!(journaled, last);

    // A requested address the caller does not read is left out of `addresses`, and matches
    // no interface: it answers as an unknown one (ADR 0021, task 006-35).
    let narrow = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO prefix (id, domain_id, prefix, nature, public, registry_version,
                             valid_period, changeset_id)
         VALUES ($1, $2, '10.0.0.16/28', 'network', false, 'test',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(narrow)
    .bind(env.domain().await)
    .execute(&pool)
    .await
    .unwrap();
    let reader = env
        .person(&["machines.interface.read", "core.prefix.read"], None)
        .await;
    env.grant_on(reader, &["core.address.read"], "prefix", Some(narrow))
        .await;
    let three = format!("{both}&address={hidden}");
    let read = held_values(&every_page(&env.app(reader), &three).await);
    assert_eq!(
        read,
        map(&[
            (eth1, &["10.0.0.17", "10.0.0.18"]),
            (eth2, &["10.0.0.17"]),
            (eth3, &["10.0.0.18"]),
        ])
    );
    // Whoever reads it sees the interfaces that hold it.
    let all = held_values(&every_page(&admin, &three).await);
    assert_eq!(all[&eth2.to_string()], ["10.0.0.17", "10.0.0.40"]);
    assert_eq!(all[&eth4.to_string()], ["10.0.0.40"]);
    // For the reader, the unreadable address answers as an unknown one, status, headers
    // and bytes: alone, beside a readable one, and among 64 (no dichotomy finds it).
    let reading = env.app(reader);
    let unknown = Uuid::now_v7();
    let probes: Vec<Uuid> = (0..63).map(|_| Uuid::now_v7()).collect();
    for wanted in [vec![], vec![first], probes] {
        let mut answers = Vec::new();
        for absent in [hidden, unknown] {
            let query = wanted
                .iter()
                .chain([&absent])
                .map(|a| format!("&address={a}"))
                .collect::<Vec<_>>()
                .concat();
            answers.push(answer(&reading, &format!("/v1/interfaces?page_size=10{query}")).await);
        }
        assert_eq!(answers[0], answers[1], "{} addresses", wanted.len() + 1);
        assert_eq!(answers[0].0, StatusCode::OK);
    }
    let (_, _, alone) = answer(&reading, &format!("/v1/interfaces?address={hidden}")).await;
    let alone: Value = serde_json::from_slice(&alone).unwrap();
    assert_eq!(alone["items"], json!([]));

    // With `network`: the interfaces holding a requested address and one of the network,
    // each with the requested addresses in the network.
    let network = env
        .id(
            "SELECT id FROM prefix WHERE prefix = $1::cidr AND valid_period @> now()",
            "10.0.1.0/24",
        )
        .await;
    let uri = format!("/v1/interfaces?network={network}&address={first}&address={elsewhere}");
    let items = every_page(&admin, &uri).await;
    assert_eq!(held_values(&items), map(&[(eth1, &["10.0.1.5"])]));
    let uri = format!("/v1/interfaces?network={network}&address={first}");
    let items = every_page(&admin, &uri).await;
    assert_eq!(held_values(&items), map(&[(eth1, &[])]));

    // At a date: the requested addresses each interface held then.
    let between: time::OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    env.applied(vec![
        op(
            "detach_address",
            json!({"interface": {"id": eth1}, "address": {"id": first}}),
        ),
        attach(eth3, first),
    ])
    .await;
    let at = between
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let then = every_page(&admin, &format!("{both}&valid_at={at}")).await;
    assert_eq!(held_values(&then), expected);
    let now = every_page(&admin, &both).await;
    assert_eq!(
        held_values(&now),
        map(&[
            (eth1, &["10.0.0.18"]),
            (eth2, &["10.0.0.17"]),
            (eth3, &["10.0.0.17", "10.0.0.18"]),
        ])
    );
}

/// An operation of the core from its JSON fields (the VLANs).
fn core_op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = name.into();
    serde_json::from_value(object).unwrap()
}

/// `body` with every summary of one of `hidden` cut to its identifier: what a caller who
/// may not read them is served.
fn cut(body: &Value, hidden: &[Uuid]) -> Value {
    match body {
        Value::Object(members) => {
            let id = members
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| id.parse::<Uuid>().ok());
            match id {
                Some(id) if hidden.contains(&id) && members.contains_key("href") => {
                    json!({"id": id})
                }
                _ => Value::Object(
                    members
                        .iter()
                        .map(|(name, value)| (name.clone(), cut(value, hidden)))
                        .collect(),
                ),
            }
        }
        Value::Array(items) => Value::Array(items.iter().map(|v| cut(v, hidden)).collect()),
        other => other.clone(),
    }
}

#[kartenn_testing::pg_test]
async fn a_reference_leaves_alike_the_unreadable_and_the_absent(pool: PgPool) {
    // REQ-006-42, 005 (task 006-33d): the owner and the site of a cluster or a machine, the
    // host, the cluster of a member, the machine and the VLANs of an interface are
    // summarised in full for a caller who may read them, by their identifier alone for one
    // who may not, the host saying its kind only then; the linked objects gone, the reader
    // is served the very bytes the other one was.
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let other = env.entity("other").await;
    env.applied(vec![core_op(
        "create_vlan_group",
        json!({"slug": "edge", "anchor": {"entity": {"id": acme}}, "vid_min": 1, "vid_max": 4094}),
    )])
    .await;
    let now = "AND valid_period @> now()";
    let group = env
        .id(
            &format!("SELECT id FROM vlan_group WHERE slug = $1 {now}"),
            "edge",
        )
        .await;
    env.applied(vec![core_op(
        "create_vlan",
        json!({"group": {"id": group}, "vid": 10, "name": "users"}),
    )])
    .await;
    let vlan = env
        .id(
            &format!("SELECT id FROM vlan WHERE name = $1 {now}"),
            "users",
        )
        .await;
    env.applied(vec![
        op(
            "create_cluster",
            json!({"local": 1, "slug": "farm", "owner": {"id": other}}),
        ),
        op(
            "create_cluster",
            json!({"slug": "pool", "owner": {"id": acme}}),
        ),
        op(
            "create_machine",
            json!({"local": 2, "name": "hv", "kind": "physical", "owner": {"id": other}}),
        ),
        op(
            "create_interface",
            json!({"machine": {"local": 2}, "name": "eth0"}),
        ),
        op(
            "create_machine",
            json!({"name": "node", "kind": "physical", "owner": {"id": acme},
                   "member_of": {"local": 1}}),
        ),
        op(
            "create_machine",
            json!({"local": 3, "name": "vm", "kind": "virtual", "owner": {"id": acme},
                   "host": {"machine": {"local": 2}}}),
        ),
        op(
            "create_interface",
            json!({"machine": {"local": 3}, "name": "eth0",
                   "vlan": {"mode": "access", "access": {"id": vlan}}}),
        ),
        op(
            "create_machine",
            json!({"name": "vm2", "kind": "virtual", "owner": {"id": acme},
                   "host": {"cluster": {"local": 1}}}),
        ),
    ])
    .await;
    let cluster = |slug| env.id("SELECT id FROM machines.cluster WHERE slug = $1", slug);
    let (farm, pool_of_acme) = (cluster("farm").await, cluster("pool").await);
    let machine = |name| env.id("SELECT id FROM machines.machine WHERE name = $1", name);
    let (hv, node, vm, vm2) = (
        machine("hv").await,
        machine("node").await,
        machine("vm").await,
        machine("vm2").await,
    );
    let interface = |of: Uuid| {
        let of = of.to_string();
        let env = &env;
        async move {
            env.id(
                "SELECT id FROM machines.interface WHERE machine_id = $1::uuid",
                &of,
            )
            .await
        }
    };
    let (hv_eth0, vm_eth0) = (interface(hv).await, interface(vm).await);
    // Everything; and the machines and clusters of `acme`, every interface, nothing else.
    let reader = env.admin().await;
    env.grant(reader, &["core.entity.read", "core.site.read"], None)
        .await;
    let reader = env.app(reader);
    let blind = env
        .person(&["machines.read", "machines.cluster.read"], Some(acme))
        .await;
    env.grant(blind, &["machines.interface.read"], None).await;
    let blind = env.app(blind);
    let uris = [
        format!("/v1/clusters/{pool_of_acme}"),
        format!("/v1/clusters?id={pool_of_acme}"),
        format!("/v1/machines/{node}"),
        format!("/v1/machines/{vm}"),
        format!("/v1/machines/{vm2}"),
        format!("/v1/machines?id={node}&id={vm}&id={vm2}"),
        format!("/v1/interfaces/{vm_eth0}"),
        format!("/v1/interfaces/{hv_eth0}"),
        format!("/v1/interfaces?id={vm_eth0}&id={hv_eth0}"),
    ];
    let mut shown = std::collections::BTreeMap::new();
    let mut before = Vec::new();
    for uri in &uris {
        let (status, _, bytes) = answer(&reader, uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        let full: Value = serde_json::from_slice(&bytes).unwrap();
        let unreadable = answer(&blind, uri).await;
        assert_eq!(unreadable.0, StatusCode::OK, "{uri}");
        let expected = serde_json::to_vec(&cut(&full, &[acme, other, farm, hv, vlan])).unwrap();
        assert_eq!(
            String::from_utf8(unreadable.2.clone()).unwrap(),
            String::from_utf8(expected).unwrap(),
            "{uri}: the same body but the summaries, cut to their identifier"
        );
        shown.insert(uri.clone(), full);
        before.push(unreadable);
    }
    // The summaries in full: the key of each kind, and the kind of a host.
    let entity = json!({"id": acme, "slug": "acme", "label": "acme",
                        "href": format!("/v1/entities/{acme}")});
    assert_eq!(shown[&uris[0]]["owner"], entity);
    assert_eq!(shown[&uris[2]]["owner"], entity);
    assert_eq!(
        shown[&uris[2]]["member_of"],
        json!({"id": farm, "label": "farm", "href": format!("/v1/clusters/{farm}")})
    );
    assert_eq!(
        shown[&uris[3]]["host"],
        json!({"id": hv, "kind": "machine", "label": "hv", "href": format!("/v1/machines/{hv}")})
    );
    assert_eq!(
        shown[&uris[4]]["host"],
        json!({"id": farm, "kind": "cluster", "label": "farm",
               "href": format!("/v1/clusters/{farm}")})
    );
    assert_eq!(shown[&uris[6]]["vlan"]["access"]["label"], "10 · users");
    assert_eq!(
        shown[&uris[7]]["machine"],
        json!({"id": hv, "label": "hv", "href": format!("/v1/machines/{hv}")})
    );
    // A host the caller may not read says not its kind.
    let hidden: Value = serde_json::from_slice(&before[4].2).unwrap();
    assert_eq!(hidden["host"], json!({"id": farm}));
    // The linked objects gone: the reader is served what the other one was.
    for (table, id) in [
        ("entity", acme),
        ("entity", other),
        ("machines.cluster", farm),
        ("machines.machine", hv),
        ("vlan", vlan),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {table} WHERE id = $1"
        )))
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    }
    for (uri, unreadable) in uris.iter().zip(&before) {
        let absent = answer(&reader, uri).await;
        assert_eq!(&absent, unreadable, "{uri}: unreadable and absent");
    }
}

#[kartenn_testing::pg_test]
async fn the_addresses_of_a_machine_are_summarised_as_the_core_reads_them(pool: PgPool) {
    // REQ-006-42 (task 006-33d): the interface and the address of an attachment are
    // summarised as every other reference, the label of an address being its value as the
    // core gives it (IPv6 included), and a caller sees in full only
    // what it may read.
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let (srv, eth0) = env.machine("srv", acme).await;
    // Two zero runs of the same length: RFC 5952 §4.2.3 compresses the first. (No
    // IPv4-mapped address is stored, REQ-002-31.)
    let values = ["10.0.0.20", "2001:db8::20", "2001:db8:0:0:1:0:0:1"];
    env.applied(vec![Operation::CreatePrefix {
        local: None,
        domain: Ref::Id(env.domain().await),
        prefix: "2001:db8::/64".into(),
        nature: Nature::Network,
        point_to_point: false,
        anycast: false,
    }])
    .await;
    let mut addresses = Vec::new();
    for value in values {
        addresses.push(env.address(value).await);
    }
    env.applied(addresses.iter().map(|a| attach(eth0, *a)).collect())
        .await;
    let reader = env.app(env.admin().await);
    // The machines, every interface; the addresses of the network 10.0.0.0/24 only.
    let some = env
        .person(&["machines.read", "machines.interface.read"], None)
        .await;
    env.grant_on(
        some,
        &["core.address.read"],
        "prefix",
        Some(env.network().await),
    )
    .await;
    let some = env.app(some);
    // The machines, not their interfaces.
    let none = env.app(
        env.person(&["machines.read", "core.address.read"], None)
            .await,
    );
    let uri = format!("/v1/machines/{srv}/addresses");
    let interface = json!({"id": eth0, "label": "eth0", "href": format!("/v1/interfaces/{eth0}")});
    for (app, expected) in [(&reader, 3), (&some, 1), (&none, 0)] {
        let (status, _, body) = call(app, "GET", &uri, None, &[]).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), expected, "{body}");
        for item in items {
            assert_eq!(item["interface"], interface, "{item}");
            let id = item["address"]["id"].as_str().unwrap().to_owned();
            // Listed only if the core serves it to this caller, with the very value.
            let (status, _, core) =
                call(app, "GET", &format!("/v1/addresses/{id}"), None, &[]).await;
            assert_eq!(status, StatusCode::OK, "{item}");
            assert_eq!(
                item["address"],
                json!({"id": id, "label": core["value"], "href": format!("/v1/addresses/{id}")}),
                "{item}"
            );
        }
    }
    // What the reader is not served, the other one may not read either.
    for address in &addresses[1..] {
        let (status, _, _) =
            call(&some, "GET", &format!("/v1/addresses/{address}"), None, &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

/// A virtual machine `vm` hosted by the physical machine `hv`, both of `acme`.
async fn hosted(env: &Env) -> (Uuid, Uuid) {
    let acme = env.entity("acme").await;
    env.applied(vec![
        op(
            "create_machine",
            json!({"local": 1, "name": "hv", "kind": "physical", "owner": {"id": acme}}),
        ),
        op(
            "create_machine",
            json!({"name": "vm", "kind": "virtual", "owner": {"id": acme},
                   "host": {"machine": {"local": 1}}}),
        ),
    ])
    .await;
    let machine = |name| env.id("SELECT id FROM machines.machine WHERE name = $1", name);
    (machine("hv").await, machine("vm").await)
}

#[kartenn_testing::pg_test]
async fn a_host_as_read_is_refused_as_a_write(pool: PgPool) {
    // Design 006 (H23) and 009 « Routes »: a summary is read, a reference is written by its
    // identifier; the host sent back as read, in a Merge Patch, is refused, the member named.
    let env = Env::new(&pool).await;
    let (_, vm) = hosted(&env).await;
    let app = env.app(env.admin().await);
    let uri = format!("/v1/machines/{vm}");
    let (_, headers, read) = call(&app, "GET", &uri, None, &[]).await;
    let etag = headers[header::ETAG].to_str().unwrap().to_owned();
    let patch = json!({"motive": "back as read", "host": read["host"]});
    let (status, _, body) = call(
        &app,
        "PATCH",
        &uri,
        Some(("application/merge-patch+json", patch)),
        &[("If-Match", etag)],
    )
    .await;
    // Refused before the changeset, the member pointed at and the form it takes said (task
    // 006-33f3), and not the text of the deserialisation.
    let refused = |status: StatusCode, body: &Value| {
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body["type"], "urn:kartenn:problem:invalid-member", "{body}");
        assert_eq!(body["pointer"], "/host", "{body}");
        let detail = body["detail"].as_str().unwrap();
        assert!(detail.starts_with("host:"), "{body}");
        assert!(
            detail.contains(r#"{"machine": {"id": …}} or {"cluster": {"id": …}}"#),
            "{body}"
        );
    };
    refused(status, &body);
    let (_, _, after) = call(&app, "GET", &uri, None, &[]).await;
    assert_eq!(after["host"], read["host"]);
    // A creation as well.
    let create = json!({"motive": "as read", "name": "vm-2", "kind": "virtual",
                        "host": read["host"]});
    let (status, _, body) = call(
        &app,
        "POST",
        "/v1/machines",
        Some(("application/json", create)),
        &[],
    )
    .await;
    refused(status, &body);
    // Through `POST /v1/changesets`, the operation pointed at, the form said alike.
    let submission = json!({"motive": "as read", "operations": [
        {"op": "machines.update_machine", "id": {"id": vm}, "host": read["host"]},
    ]});
    let (status, _, body) = call(
        &app,
        "POST",
        "/v1/changesets",
        Some(("application/json", submission)),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let violation = &body["violations"][0];
    assert_eq!(violation["rule"], "machines.operation.invalid", "{body}");
    assert_eq!(violation["pointer"], "/operations/0", "{body}");
    assert!(
        violation["detail"]
            .as_str()
            .unwrap()
            .contains(r#"{"machine": {"id": …}} or {"cluster": {"id": …}}"#),
        "{body}"
    );
}

#[kartenn_testing::pg_test]
async fn renaming_the_host_changes_the_etag_of_its_guest(pool: PgPool) {
    // REQ-006-42: the label of a linked object is in the body, and so in its ETag.
    let env = Env::new(&pool).await;
    let (hv, vm) = hosted(&env).await;
    let app = env.app(env.admin().await);
    let uri = format!("/v1/machines/{vm}");
    let before = etag_of(&app, &uri).await;
    env.applied(vec![op(
        "update_machine",
        json!({"id": {"id": hv}, "name": "hv2"}),
    )])
    .await;
    let (status, body) = revalidated(&app, &uri, &before).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["host"]["label"], "hv2");
    assert_ne!(etag_of(&app, &uri).await, before);
}
