//! The references of the operations of the module (REQ-002-12, amended; task 002-29d1):
//! each `Ref::Id` of `create_cluster`, `update_cluster`, `sync` and the settings designates
//! an object of its kind in force at the start of the period, refused otherwise by
//! `core.reference.unknown`; and an author without the right on the object a reference
//! names gets the same answer whatever its identifier names (INV-63). The cluster of a
//! `sync` absent at the start of its period is left to task 002-29d2: every `sync` here
//! names a living cluster.

use std::collections::BTreeSet;
use std::sync::Arc;

use kartenn_core::objects::Nature;
use kartenn_kube::Kube;
use kartenn_machines::Machines;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{ModuleDeclarations, core_declarations_for, sync_modules};
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::{DeclarationInput, RawInput, RawLabels, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

const UNKNOWN: &str = "core.reference.unknown";

/// The axis of the settings, a closed list carried by the workloads.
const EXPOSURE: &str = "custom.exposure";

struct Env {
    pool: PgPool,
    store: PgStore,
    registry: Arc<ModuleRegistry>,
    author: Uuid,
    /// The service account of the connector, which submits the batches.
    connector: Uuid,
    /// The connector group of every cluster.
    group: Uuid,
    acme: Uuid,
    /// A domain of pods and a domain of services of `acme`, with their prefix.
    pods: Uuid,
    services: Uuid,
    container: Uuid,
    network: Uuid,
}

impl Env {
    /// Both modules migrated and declared, the entity `acme`, its pod domain (container
    /// `198.51.100.0/24`) and its service domain (network `203.0.113.0/26`), the axis
    /// `custom.exposure`.
    async fn new(pool: &PgPool) -> Self {
        let modules: Vec<Arc<dyn Module>> = vec![Arc::new(Machines), Arc::new(Kube)];
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
        let (connector, group) = crate::support::connector(pool, "connector").await;
        let mut env = Self {
            pool: pool.clone(),
            store,
            registry,
            author,
            connector,
            group,
            acme: Uuid::nil(),
            pods: Uuid::nil(),
            services: Uuid::nil(),
            container: Uuid::nil(),
            network: Uuid::nil(),
        };
        env.acme = env.entity("acme").await;
        env.pods = env
            .domain(
                "pods",
                env.acme,
                Some(("198.51.100.0/24", Nature::Container)),
            )
            .await;
        env.services = env
            .domain(
                "services",
                env.acme,
                Some(("203.0.113.0/26", Nature::Network)),
            )
            .await;
        env.container = env.prefix(env.pods).await;
        env.network = env.prefix(env.services).await;
        let mut axis = vec![Operation::DeclareProperty {
            local: Some(1),
            declaration: DeclarationInput {
                key: EXPOSURE.into(),
                value_type: TypeInput::Enum { values_of: None },
                cardinality: kartenn_core::values::Cardinality::Single,
                carriers: vec!["kube.workload".into()],
                inherit: None,
                module: None,
                obligations: Vec::new(),
                expected: None,
                labels: None,
            },
        }];
        axis.extend(["externe", "interne"].map(|key| Operation::AddEnumValue {
            local: None,
            property: Ref::Local(1),
            key: key.into(),
            labels: RawLabels::new(),
        }));
        env.applied(axis).await;
        env
    }

    /// `operations` submitted without RBAC, by the connector when they are a batch, by
    /// the author otherwise.
    async fn submit(&self, operations: Vec<Operation>) -> Outcome {
        let author = if crate::support::batch(&operations) {
            self.connector
        } else {
            self.author
        };
        let request = ChangesetRequest {
            author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    /// `operations` submitted by `identity`, under the RBAC of the store.
    async fn submit_as(&self, identity: Uuid, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author: identity,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        let authorizer = kartenn_store_pg::rbac::PgAuthorizer::new(self.pool.clone())
            .with_modules(self.registry.clone());
        submit(&self.store, &authorizer, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    /// A new identity holding `on_acme` on `acme` and what it owns, and `global`
    /// everywhere: a service account of the connector group when `connector`, a person
    /// otherwise.
    async fn granted(&self, on_acme: &[&str], global: &[&str], connector: bool) -> Uuid {
        let identity: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name, kind)
             VALUES (gen_random_uuid(), 'r-' || gen_random_uuid(), $1)
             RETURNING id",
        )
        .bind(if connector {
            "service_account"
        } else {
            "person"
        })
        .fetch_one(&self.pool)
        .await
        .unwrap();
        if connector {
            crate::support::member(&self.pool, self.group, identity).await;
        }
        for (permissions, scope) in [(on_acme, Some(self.acme)), (global, None)] {
            if permissions.is_empty() {
                continue;
            }
            let role = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO role (id, key, labels, valid_period, changeset_id)
                 VALUES ($1, $2, jsonb_build_object('fr', 'r'),
                         tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
            )
            .bind(role)
            .bind(format!("r_{}", role.simple()))
            .execute(&self.pool)
            .await
            .unwrap();
            for permission in permissions {
                sqlx::query(
                    "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
                     VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL),
                             gen_random_uuid())",
                )
                .bind(role)
                .bind(permission)
                .execute(&self.pool)
                .await
                .unwrap();
            }
            sqlx::query(kartenn_testing::BIND_IDENTITY)
                .bind(identity)
                .bind(role)
                .bind(if scope.is_some() { "entity" } else { "global" })
                .bind(scope)
                .execute(&self.pool)
                .await
                .unwrap();
        }
        identity
    }

    async fn applied(&self, operations: Vec<Operation>) {
        let outcome = self.submit(operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    async fn id(&self, table: &str, column: &str, value: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE {column} = $1 AND valid_period @> now()"
        )))
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn entity(&self, slug: &str) -> Uuid {
        self.applied(vec![Operation::CreateEntity {
            local: None,
            slug: slug.into(),
            parent: None,
        }])
        .await;
        self.id("entity", "slug", slug).await
    }

    /// A domain `slug` owned by `owner`, with the prefix `prefix` of its nature if any.
    async fn domain(&self, slug: &str, owner: Uuid, prefix: Option<(&str, Nature)>) -> Uuid {
        let property = self.id("property_declaration", "key", "core.owner").await;
        let mut operations = vec![
            Operation::CreateDomain {
                local: Some(1),
                slug: slug.into(),
            },
            Operation::AssignProperty {
                object: Ref::Local(1),
                kind: ObjectKind::Domain,
                property: Ref::Id(property),
                values: vec![RawInput::Text(owner.to_string())],
            },
        ];
        if let Some((prefix, nature)) = prefix {
            operations.push(Operation::CreatePrefix {
                local: None,
                domain: Ref::Local(1),
                prefix: prefix.into(),
                nature,
                point_to_point: false,
                anycast: false,
            });
        }
        self.applied(operations).await;
        self.id("addressing_domain", "slug", slug).await
    }

    async fn prefix(&self, domain: Uuid) -> Uuid {
        sqlx::query_scalar("SELECT id FROM prefix WHERE domain_id = $1 AND valid_period @> now()")
            .bind(domain)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// The fields of a valid cluster `slug` of `acme`, with `extra`.
    fn cluster(&self, slug: &str, extra: Value) -> Value {
        let mut fields = json!({
            "slug": slug,
            "owner": {"id": self.acme},
            "pod_domain": {"id": self.pods},
            "service_domain": {"id": self.services},
            "pod_containers": [{"id": self.container}],
            "service_networks": [{"id": self.network}],
            "connector_group": {"id": self.group},
        });
        merge(&mut fields, extra);
        fields
    }

    /// A new cluster `slug` of `fields`.
    async fn new_cluster(&self, slug: &str, fields: Value) -> Uuid {
        self.applied(vec![op("create_cluster", fields)]).await;
        self.id("kube.cluster", "slug", slug).await
    }

    /// A new namespace `name` of `cluster`.
    async fn namespace(&self, cluster: Uuid, name: &str) -> Uuid {
        self.applied(vec![sync("upsert", "namespace", cluster, name, json!({}))])
            .await;
        sqlx::query_scalar(
            "SELECT id FROM kube.namespace
              WHERE cluster_id = $1 AND name = $2 AND valid_period @> now()",
        )
        .bind(cluster)
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// A machine `name` of `kind` owned by `owner`.
    async fn machine(&self, name: &str, kind: &str, owner: Uuid) -> Uuid {
        self.applied(vec![machine_op(
            "create_machine",
            json!({"name": name, "kind": kind, "owner": {"id": owner}}),
        )])
        .await;
        self.id("machines.machine", "name", name).await
    }

    /// The living rows of the tables of the module and of `module_object`.
    async fn written(&self) -> Vec<i64> {
        let mut counts = Vec::new();
        for table in [
            "kube.cluster",
            "kube.node",
            "kube.namespace",
            "kube.label_mapping",
            "kube.admitted_values",
            "module_object",
        ] {
            counts.push(
                sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                    "SELECT count(*) FROM {table} WHERE valid_period @> now()"
                )))
                .fetch_one(&self.pool)
                .await
                .unwrap(),
            );
        }
        counts
    }
}

fn merge(into: &mut Value, extra: Value) {
    if let (Value::Object(into), Value::Object(extra)) = (into, extra) {
        into.extend(extra);
    }
}

/// `kube.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("kube.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// `machines.<name>` from its JSON fields.
fn machine_op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// `kube.sync` of `action` on the object of `kind` named `name` in `cluster`.
fn sync(action: &str, kind: &str, cluster: Uuid, name: &str, extra: Value) -> Operation {
    let mut fields = json!({
        "action": action,
        "type": format!("kube.{kind}"),
        "cluster": {"id": cluster},
        "name": name,
    });
    merge(&mut fields, extra);
    op("sync", fields)
}

fn close(kind: &str, id: Uuid, cascade: bool) -> Operation {
    Operation::Close {
        kind: ObjectKind::from_name(kind).unwrap(),
        id: Ref::Id(id),
        expected: None,
        cascade,
    }
}

fn admitted(namespace: Uuid) -> Operation {
    op(
        "set_admitted_values",
        json!({"namespace": {"id": namespace}, "axis": EXPOSURE, "values": ["interne"]}),
    )
}

fn close_mapping(id: Uuid) -> Operation {
    op(
        "set_label_mapping",
        json!({"action": "close", "id": {"id": id}}),
    )
}

/// The identifiers of the references (`Ref::Id`, `{"id": <uuid>}`) a JSON value holds.
fn ref_ids(value: &Value, out: &mut BTreeSet<Uuid>) {
    match value {
        Value::Object(map) => {
            if map.len() == 1
                && let Some(id) = map
                    .get("id")
                    .and_then(Value::as_str)
                    .and_then(|s| Uuid::parse_str(s).ok())
            {
                out.insert(id);
            }
            map.values().for_each(|v| ref_ids(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| ref_ids(v, out)),
        _ => {}
    }
}

/// The rules and the objects of a refusal.
fn refusal_of(outcome: &Outcome) -> (Vec<&'static str>, BTreeSet<Uuid>) {
    let Outcome::Refused { violations, .. } = outcome else {
        panic!("expected a refusal, got {outcome:?}")
    };
    (
        violations.iter().map(|v| v.rule.id).collect(),
        violations
            .iter()
            .flat_map(|v| v.subjects.iter().copied())
            .collect(),
    )
}

/// The refusal of `outcome` as its author sees it: rules, messages, operations and the
/// number of objects of each violation.
fn seen(outcome: &Outcome) -> Vec<(&'static str, String, Option<usize>, usize)> {
    match outcome {
        Outcome::Refused { violations, .. } => violations
            .iter()
            .map(|v| (v.rule.id, v.message.clone(), v.operation, v.subjects.len()))
            .collect(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The operation types without a reference by identifier.
const NO_REFERENCE: &[&str] = &["kube.set_tracked_labels"];

/// One operation or more per operation type of the module, every reference an identifier
/// nothing bears, but the cluster of a `sync` (`cluster`, living: task 002-29d2) and the
/// connector group, a group of the RBAC (task 002-29b1).
fn reference_samples(cluster: Uuid) -> Vec<Operation> {
    let id = || json!({ "id": Uuid::now_v7() });
    let fields = || {
        json!({"owner": id(), "site": id(),
               "pod_domain": id(), "service_domain": id(), "external_domain": id(),
               "pod_containers": [id(), id()], "service_networks": [id()],
               "machines_cluster": id()})
    };
    let mut create = fields();
    merge(&mut create, json!({"slug": "c"}));
    let mut update = fields();
    merge(&mut update, json!({"id": id()}));
    vec![
        op("create_cluster", create),
        op("update_cluster", update),
        sync(
            "upsert",
            "node",
            cluster,
            "worker-1",
            json!({"machine": id()}),
        ),
        op("set_label_mapping", json!({"action": "close", "id": id()})),
        op(
            "set_admitted_values",
            json!({"namespace": id(), "axis": EXPOSURE, "values": ["interne"]}),
        ),
    ]
}

/// REQ-002-12 (amendment of 2026-11-14), task 002-29d1: every reference of every operation
/// of the module is read before it is written; a new operation type fails here until it
/// has a sample or is listed without a reference.
#[kartenn_testing::pg_test]
async fn every_reference_of_every_operation_is_checked(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env
        .new_cluster("prod", env.cluster("prod", json!({})))
        .await;
    let samples = reference_samples(prod);
    let types: BTreeSet<String> = Kube
        .operations()
        .iter()
        .map(|o| format!("kube.{}", o.name))
        .filter(|t| !NO_REFERENCE.contains(&t.as_str()))
        .collect();
    let covered: BTreeSet<String> = samples
        .iter()
        .map(|op| match op {
            Operation::Module(m) => m.tag(),
            other => panic!("not an operation of the module: {other:?}"),
        })
        .collect();
    assert_eq!(covered, types, "an operation of the module has no sample");
    let before = env.written().await;
    for op in samples {
        let mut expected = BTreeSet::new();
        ref_ids(&serde_json::to_value(&op).unwrap(), &mut expected);
        expected.remove(&prod);
        let (rules, named) = refusal_of(&env.submit(vec![op.clone()]).await);
        assert!(rules.iter().all(|r| *r == UNKNOWN), "{op:?}: {rules:?}");
        assert_eq!(named, expected, "{op:?}: every reference is checked");
    }
    assert_eq!(env.written().await, before, "nothing is written");
}

/// REQ-002-12 (amendment of 2026-11-14), task 002-29d1: a reference of another kind or
/// closed is refused as one that names nothing, and nothing is written.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // the objects, then the cases
async fn the_references_designate_objects_in_force(pool: PgPool) {
    let env = Env::new(&pool).await;
    let gone_domain = env.domain("gone", env.acme, None).await;
    env.applied(vec![close("domain", gone_domain, false)]).await;
    let prod = env
        .new_cluster("prod", env.cluster("prod", json!({})))
        .await;
    let old = env.new_cluster("old", env.cluster("old", json!({}))).await;
    env.applied(vec![close("kube.cluster", old, true)]).await;
    let payments = env.namespace(prod, "payments").await;
    let left = env.namespace(prod, "left").await;
    env.applied(vec![sync("close", "namespace", prod, "left", json!({}))])
        .await;
    let server = env.machine("server-1", "physical", env.acme).await;
    let retired = env.machine("server-2", "physical", env.acme).await;
    env.applied(vec![close("machines.machine", retired, true)])
        .await;
    env.applied(vec![machine_op(
        "create_cluster",
        json!({"slug": "mc", "owner": {"id": env.acme}}),
    )])
    .await;
    let mc = env.id("machines.cluster", "slug", "mc").await;
    env.applied(vec![op(
        "set_label_mapping",
        json!({"action": "upsert", "label_key": "tier", "label_value": "db",
               "axis": EXPOSURE, "value": "interne"}),
    )])
    .await;
    let mapping = env.id("kube.label_mapping", "label_value", "db").await;
    env.applied(vec![close_mapping(mapping)]).await;
    let created = |slug: &str, extra: Value| op("create_cluster", env.cluster(slug, extra));
    let cases: Vec<(&str, Operation, Uuid)> = vec![
        (
            "pod_domain: a prefix",
            created("c1", json!({"pod_domain": {"id": env.container}})),
            env.container,
        ),
        (
            "service_domain: closed",
            created("c2", json!({"service_domain": {"id": gone_domain}})),
            gone_domain,
        ),
        (
            "pod_containers: a domain",
            created("c3", json!({"pod_containers": [{"id": env.pods}]})),
            env.pods,
        ),
        (
            "machines_cluster: a machine",
            created("c4", json!({"machines_cluster": {"id": server}})),
            server,
        ),
        (
            "owner: a domain",
            created("c5", json!({"owner": {"id": env.pods}})),
            env.pods,
        ),
        (
            "update_cluster: a machine cluster",
            op(
                "update_cluster",
                json!({"id": {"id": mc}, "retention_days": 9}),
            ),
            mc,
        ),
        (
            "update_cluster: closed",
            op(
                "update_cluster",
                json!({"id": {"id": old}, "retention_days": 9}),
            ),
            old,
        ),
        (
            "node machine: a machine cluster",
            sync("upsert", "node", prod, "w1", json!({"machine": {"id": mc}})),
            mc,
        ),
        (
            "node machine: closed",
            sync(
                "upsert",
                "node",
                prod,
                "w2",
                json!({"machine": {"id": retired}}),
            ),
            retired,
        ),
        ("admitted namespace: a cluster", admitted(prod), prod),
        ("admitted namespace: closed", admitted(left), left),
        ("mapping: a namespace", close_mapping(payments), payments),
        ("mapping: closed", close_mapping(mapping), mapping),
    ];
    let before = env.written().await;
    for (case, operation, named) in cases {
        let (rules, subjects) = refusal_of(&env.submit(vec![operation]).await);
        assert_eq!((rules, subjects), (vec![UNKNOWN], [named].into()), "{case}");
    }
    assert_eq!(env.written().await, before, "nothing is written");
    // In force, accepted.
    env.applied(vec![admitted(payments)]).await;
}

/// An operation citing the identifier it is given.
type Make<'a> = Box<dyn Fn(Uuid) -> Operation + 'a>;

/// A case of a leak: its name, its author, the operation citing `x`, the object in force
/// out of reach, the hidden ones, and whether the reference needs a right.
type Case<'a> = (&'a str, Uuid, Make<'a>, Uuid, Vec<Uuid>, bool);

/// INV-63 on the references of the module (task 002-29d1): an author without the right on
/// the object a reference names gets the same answer whatever its identifier names
/// (nothing, an object of another kind, an object closed, an object in force out of his
/// reach): the refusal of his rights where the reference needs one (rules, messages,
/// operations, number of objects, in order), otherwise, once every right is granted,
/// `core.reference.unknown`.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // the objects of each case, then the cases
async fn no_right_reaches_through_a_hidden_reference(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.acme;
    let other = env.entity("other").await;
    let paris = {
        env.applied(vec![Operation::CreateSite {
            local: None,
            slug: "paris".into(),
            parent: None,
        }])
        .await;
        env.id("site", "slug", "paris").await
    };
    let far_pods = env
        .domain("far-pods", other, Some(("192.0.2.0/25", Nature::Container)))
        .await;
    let far_services = env
        .domain(
            "far-services",
            other,
            Some(("192.0.2.128/26", Nature::Network)),
        )
        .await;
    let (far_container, far_network) = (env.prefix(far_pods).await, env.prefix(far_services).await);
    let gone_domain = env.domain("gone", acme, None).await;
    env.applied(vec![close("domain", gone_domain, false)]).await;
    let prod = env
        .new_cluster("prod", env.cluster("prod", json!({})))
        .await;
    let old = env.new_cluster("old", env.cluster("old", json!({}))).await;
    env.applied(vec![close("kube.cluster", old, true)]).await;
    let far = env
        .new_cluster(
            "far",
            env.cluster(
                "far",
                json!({"owner": {"id": other},
                       "pod_domain": {"id": far_pods}, "service_domain": {"id": far_services},
                       "pod_containers": [{"id": far_container}],
                       "service_networks": [{"id": far_network}]}),
            ),
        )
        .await;
    let payments = env.namespace(prod, "payments").await;
    let away = env.namespace(far, "away").await;
    let left = env.namespace(prod, "left").await;
    env.applied(vec![sync("close", "namespace", prod, "left", json!({}))])
        .await;
    let theirs = env.machine("theirs", "physical", other).await;
    let retired = env.machine("retired", "physical", acme).await;
    env.applied(vec![close("machines.machine", retired, true)])
        .await;
    env.applied(vec![
        machine_op(
            "create_cluster",
            json!({"slug": "mc-theirs", "owner": {"id": other}}),
        ),
        machine_op(
            "create_cluster",
            json!({"slug": "mc-gone", "owner": {"id": acme}}),
        ),
    ])
    .await;
    let mc_theirs = env.id("machines.cluster", "slug", "mc-theirs").await;
    let mc_gone = env.id("machines.cluster", "slug", "mc-gone").await;
    env.applied(vec![close("machines.cluster", mc_gone, false)])
        .await;
    let mapping = |value: &str| {
        op(
            "set_label_mapping",
            json!({"action": "upsert", "label_key": "tier", "label_value": value,
                   "axis": EXPOSURE, "value": "interne"}),
        )
    };
    env.applied(vec![mapping("db"), mapping("web")]).await;
    let (db, web) = (
        env.id("kube.label_mapping", "label_value", "db").await,
        env.id("kube.label_mapping", "label_value", "web").await,
    );
    env.applied(vec![close_mapping(web)]).await;
    // An administrator of the clusters of `acme`, a connector of them, a team of the
    // settings reading the namespaces of `acme`.
    let admin = env
        .granted(
            &[
                "kube.cluster.read",
                "kube.cluster.edit",
                "kube.cluster.approval.self",
                "core.domain.read",
                "core.prefix.read",
                "machines.cluster.read",
                "prop.core.owner.edit",
                "prop.core.owner.approval.self",
                "prop.core.site.edit",
                "prop.core.site.approval.self",
            ],
            &[],
            false,
        )
        .await;
    let connector = env
        .granted(
            &[
                "kube.cluster.read",
                "kube.node.read",
                "kube.node.edit",
                "kube.node.approval.self",
                "machines.read",
            ],
            &[],
            true,
        )
        .await;
    let settings = env
        .granted(
            &["kube.cluster.read", "kube.namespace.read"],
            &["kube.settings.edit", "kube.settings.approval.self"],
            false,
        )
        .await;
    let unknown = Uuid::now_v7();
    let base = env.cluster("cx", json!({}));
    let fields = move |extra: Value| {
        let mut fields = base.clone();
        merge(&mut fields, extra);
        fields
    };
    let created = move |extra: Value| op("create_cluster", fields(extra));
    let updated = move |extra: Value| {
        let mut fields = json!({"id": {"id": prod}});
        merge(&mut fields, extra);
        op("update_cluster", fields)
    };
    // The case, its author, the operation citing `x`, the object in force out of reach,
    // the hidden ones, and whether the reference needs a right (otherwise the operation
    // out of reach is applied).
    let cases: Vec<Case> = vec![
        (
            "create.pod_domain",
            admin,
            Box::new(|x| created(json!({"pod_domain": {"id": x}}))),
            far_pods,
            vec![unknown, other, gone_domain],
            true,
        ),
        (
            "create.pod_containers",
            admin,
            Box::new(|x| created(json!({"pod_containers": [{"id": x}]}))),
            far_container,
            vec![unknown, other, far_pods],
            true,
        ),
        (
            "create.machines_cluster",
            admin,
            Box::new(|x| created(json!({"machines_cluster": {"id": x}}))),
            mc_theirs,
            vec![unknown, theirs, mc_gone],
            true,
        ),
        (
            "create.owner",
            admin,
            Box::new(|x| created(json!({"owner": {"id": x}}))),
            other,
            vec![unknown, paris],
            true,
        ),
        (
            "update.id",
            admin,
            Box::new(move |x| {
                op(
                    "update_cluster",
                    json!({"id": {"id": x}, "retention_days": 9}),
                )
            }),
            far,
            vec![unknown, mc_theirs, old],
            true,
        ),
        (
            "update.service_domain",
            admin,
            Box::new(move |x| updated(json!({"service_domain": {"id": x}}))),
            far_services,
            vec![unknown, other, gone_domain],
            true,
        ),
        (
            "sync.node.machine",
            connector,
            Box::new(move |x| sync("upsert", "node", prod, "w1", json!({"machine": {"id": x}}))),
            theirs,
            vec![unknown, mc_theirs, retired],
            true,
        ),
        (
            "admitted.namespace",
            settings,
            Box::new(admitted),
            away,
            // An object of another kind out of reach: one in reach (the cluster `prod`) is
            // read by its author, who learns nothing from `core.reference.unknown`.
            vec![unknown, far, left],
            true,
        ),
        (
            "mapping.id",
            settings,
            Box::new(close_mapping),
            db,
            vec![unknown, payments, web],
            false,
        ),
        // The site needs no right (task 002-29c2, task 31): an oracle left open.
        (
            "create.site",
            admin,
            Box::new(|x| created(json!({"slug": "cs", "site": {"id": x}}))),
            paris,
            vec![unknown, other],
            false,
        ),
    ];
    let mut leaks = Vec::new();
    for (name, who, make, reach, hidden, needs_right) in &cases {
        // Hidden first: an applied operation out of reach changes what follows.
        let mut answers = Vec::new();
        for h in hidden {
            answers.push((*h, env.submit_as(*who, vec![make(*h)]).await));
        }
        let out_of_reach = env.submit_as(*who, vec![make(*reach)]).await;
        let expected = if *needs_right {
            seen(&out_of_reach)
        } else {
            assert!(
                matches!(out_of_reach, Outcome::Applied { .. }),
                "{name}: {out_of_reach:?}"
            );
            vec![]
        };
        for (i, (h, answer)) in answers.into_iter().enumerate() {
            let equal = if *needs_right {
                seen(&answer) == expected
            } else {
                matches!(&answer, Outcome::Refused { .. }) && refusal_of(&answer).0 == [UNKNOWN]
            };
            if !equal {
                leaks.push(format!(
                    "{name}, hidden {i} ({h}): {answer:?} / expected {expected:?}"
                ));
            }
        }
    }
    assert!(leaks.is_empty(), "{leaks:#?}");
}
