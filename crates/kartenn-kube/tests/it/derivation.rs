//! The axes the server derives for a workload (spec 019, REQ-019-33, 34; Q-019-33, 34,
//! 37 to 39; task 019-7d2): at each `upsert` of a workload, `kube.sync` sets its axis
//! `kube.namespace` and the values its labels give, admitted in its namespace, of the
//! provenance `module:kube#label:<key>`; a value typed wins; `kube.namespace` is written
//! by `kube.sync` alone.

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

/// The axis of the tests, a closed list carried by the workloads.
const EXPOSURE: &str = "custom.exposure";
/// The axis of the namespace, which the module declares.
const NAMESPACE: &str = "kube.namespace";

struct Env {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    connector: Uuid,
    cluster: Uuid,
}

/// An assignment as stored: provenance, key of its value of a closed list, external
/// reference, changeset.
#[derive(Debug)]
struct Stored {
    provenance: String,
    value: Option<String>,
    external: Option<Value>,
    changeset: Uuid,
}

impl Env {
    /// Both modules migrated and declared (the axis `kube.namespace`, the system `kube`),
    /// the cluster `prod` and its namespaces `payments` and `shop`, the axis
    /// `custom.exposure` of the values `externe` and `interne`.
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
            author,
            connector,
            cluster: Uuid::nil(),
        };
        env.applied(vec![Operation::CreateEntity {
            local: None,
            slug: "acme".into(),
            parent: None,
        }])
        .await;
        let acme = env.id("entity", "slug", "acme").await;
        let pods = env
            .domain(acme, "pods", "198.51.100.0/24", Nature::Container)
            .await;
        let services = env
            .domain(acme, "services", "203.0.113.0/26", Nature::Network)
            .await;
        let (container, network) = (env.prefix(pods).await, env.prefix(services).await);
        env.applied(vec![op(
            "create_cluster",
            json!({
                "slug": "prod",
                "owner": {"id": acme},
                "pod_domain": {"id": pods},
                "service_domain": {"id": services},
                "pod_containers": [{"id": container}],
                "service_networks": [{"id": network}],
                "connector_group": {"id": group},
            }),
        )])
        .await;
        env.cluster = env.id("kube.cluster", "slug", "prod").await;
        env.applied(vec![
            namespace(env.cluster, "payments"),
            namespace(env.cluster, "shop"),
        ])
        .await;
        env.applied(declaration(EXPOSURE, &["externe", "interne"]))
            .await;
        env
    }

    /// `operations` submitted by the connector when they are a batch, by the author
    /// otherwise.
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

    async fn applied(&self, operations: Vec<Operation>) {
        let outcome = self.submit(operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    /// Refused by `rule` alone.
    async fn refused(&self, operations: Vec<Operation>, rule: &str) {
        match self.submit(operations).await {
            Outcome::Refused { violations, .. } => {
                let rules: Vec<&str> = violations.iter().map(|v| v.rule.id).collect();
                assert_eq!(rules, [rule], "{violations:?}");
            }
            other => panic!("not refused: {other:?}"),
        }
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

    async fn domain(&self, owner: Uuid, slug: &str, prefix: &str, nature: Nature) -> Uuid {
        let property = self.id("property_declaration", "key", "core.owner").await;
        self.applied(vec![
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
            Operation::CreatePrefix {
                local: None,
                domain: Ref::Local(1),
                prefix: prefix.into(),
                nature,
                point_to_point: false,
                anycast: false,
            },
        ])
        .await;
        self.id("addressing_domain", "slug", slug).await
    }

    async fn prefix(&self, domain: Uuid) -> Uuid {
        sqlx::query_scalar("SELECT id FROM prefix WHERE domain_id = $1 AND valid_period @> now()")
            .bind(domain)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn namespace(&self, name: &str) -> Uuid {
        self.id("kube.namespace", "name", name).await
    }

    async fn workload(&self, name: &str) -> Uuid {
        self.id("kube.workload", "name", name).await
    }

    /// The assignment of `key` on `object` living now.
    async fn assignment(&self, object: Uuid, key: &str) -> Option<Stored> {
        let row: Option<(String, Option<String>, Option<Value>, Uuid)> = sqlx::query_as(
            "SELECT a.provenance,
                    (SELECT v.key FROM property_enum_value v WHERE v.id = a.v_uuid LIMIT 1),
                    a.v_json, a.changeset_id
               FROM property_assignment a
               JOIN property_declaration d ON d.id = a.property_id AND d.valid_period @> now()
              WHERE a.object_id = $1 AND d.key = $2 AND a.valid_period @> now()",
        )
        .bind(object)
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .unwrap();
        row.map(|(provenance, value, external, changeset)| Stored {
            provenance,
            value,
            external,
            changeset,
        })
    }

    /// The versions of the assignments of `key` on `object`, current and past.
    async fn versions(&self, object: Uuid, key: &str) -> i64 {
        sqlx::query_scalar(
            "SELECT count(*) FROM property_assignment a
               JOIN property_declaration d ON d.id = a.property_id AND d.valid_period @> now()
              WHERE a.object_id = $1 AND d.key = $2",
        )
        .bind(object)
        .bind(key)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The changeset of the living version of the workload `id`.
    async fn written_by(&self, id: Uuid) -> Uuid {
        sqlx::query_scalar(
            "SELECT changeset_id FROM kube.workload WHERE id = $1 AND valid_period @> now()",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// Whether an anomaly of `rule` is open on `object`.
    async fn anomaly(&self, object: Uuid, rule: &str) -> bool {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM anomaly
                             WHERE object_id = $1 AND rule_id = $2 AND closed_at IS NULL)",
        )
        .bind(object)
        .bind(rule)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The identifier of the value `kube` of `core.external_system`.
    async fn system(&self) -> Uuid {
        sqlx::query_scalar(
            "SELECT v.id FROM property_enum_value v
               JOIN property_declaration d ON d.id = v.property_id
              WHERE d.key = 'core.external_system' AND v.key = 'kube'
                AND v.valid_period @> now()",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn declaration(key: &str, values: &[&str]) -> Vec<Operation> {
    let mut operations = vec![Operation::DeclareProperty {
        local: Some(1),
        declaration: DeclarationInput {
            key: key.into(),
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
    operations.extend(values.iter().map(|&value| Operation::AddEnumValue {
        local: None,
        property: Ref::Local(1),
        key: value.into(),
        labels: RawLabels::new(),
    }));
    operations
}

/// `kube.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("kube.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// `kube.sync` upserting the namespace `name` of `cluster`.
fn namespace(cluster: Uuid, name: &str) -> Operation {
    op(
        "sync",
        json!({
            "action": "upsert", "type": "kube.namespace", "cluster": {"id": cluster},
            "name": name,
        }),
    )
}

/// `kube.sync` upserting the deployment `name` of `namespace`, with `labels` (none:
/// those stored kept).
fn deployment(cluster: Uuid, namespace: &str, name: &str, labels: Option<Value>) -> Operation {
    let mut body = json!({
        "action": "upsert", "type": "kube.workload", "cluster": {"id": cluster},
        "namespace": namespace, "kind": "deployment", "name": name,
    });
    if let Some(labels) = labels {
        body["labels"] = labels;
    }
    op("sync", body)
}

fn mapping(label: &str, axis: &str, value: &str) -> Operation {
    op(
        "set_label_mapping",
        json!({
            "action": "upsert", "label_key": "tier", "label_value": label, "axis": axis,
            "value": value,
        }),
    )
}

fn admitted(namespace: Uuid, values: &[&str]) -> Operation {
    op(
        "set_admitted_values",
        json!({"namespace": {"id": namespace}, "axis": EXPOSURE, "values": values}),
    )
}

fn workload_kind() -> ObjectKind {
    ObjectKind::from_name("kube.workload").unwrap()
}

/// REQ-019-33, 34 (Q-019-33, 34, 37): `tier=db` gives `custom.exposure: interne`,
/// admitted in `payments`: set by the upsert, in its changeset, of the provenance
/// `module:kube#label:tier`, with `kube.namespace` (`<cluster>/payments`, labelled
/// `prod/payments`, system `kube`); in `shop`, where `interne` is not admitted: ignored,
/// `kube.label.not_admitted`, until admitted. An upsert unchanged writes no assignment
/// again; a label gone removes what the module set; a typed value is neither replaced nor
/// removed.
#[kartenn_testing::pg_test]
async fn label_mapping_admitted(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (payments, shop) = (env.namespace("payments").await, env.namespace("shop").await);
    env.applied(vec![
        mapping("db", EXPOSURE, "interne"),
        admitted(payments, &["interne"]),
        admitted(shop, &["externe"]),
    ])
    .await;
    let db = || Some(json!({"tier": "db"}));
    env.applied(vec![
        deployment(env.cluster, "payments", "ledger", db()),
        deployment(env.cluster, "shop", "cart", db()),
    ])
    .await;
    let (ledger, cart) = (env.workload("ledger").await, env.workload("cart").await);
    let exposure = env.assignment(ledger, EXPOSURE).await.expect("derived");
    assert_eq!(exposure.provenance, "module:kube#label:tier");
    assert_eq!(exposure.value.as_deref(), Some("interne"));
    assert_eq!(exposure.changeset, env.written_by(ledger).await);
    let system = env.system().await;
    for (workload, name) in [(ledger, "payments"), (cart, "shop")] {
        let axis = env.assignment(workload, NAMESPACE).await.expect("set");
        assert_eq!(axis.provenance, "module:kube#namespace");
        assert_eq!(
            axis.external,
            Some(json!({
                "system": system,
                "id": format!("{}/{name}", env.cluster),
                "label": format!("prod/{name}"),
            }))
        );
    }
    // Not admitted in `shop`: ignored, with a warning on the workload.
    assert!(env.assignment(cart, EXPOSURE).await.is_none());
    assert!(env.anomaly(cart, "kube.label.not_admitted").await);
    assert!(!env.anomaly(ledger, "kube.label.not_admitted").await);
    // The same batch again: nothing derived is written anew.
    env.applied(vec![deployment(env.cluster, "payments", "ledger", db())])
        .await;
    env.applied(vec![deployment(env.cluster, "payments", "ledger", None)])
        .await;
    assert_eq!(env.versions(ledger, EXPOSURE).await, 1);
    assert_eq!(env.versions(ledger, NAMESPACE).await, 1);
    // Admitted in `shop` afterwards: set at the next upsert, the anomaly closed.
    env.applied(vec![admitted(shop, &["externe", "interne"])])
        .await;
    env.applied(vec![deployment(env.cluster, "shop", "cart", None)])
        .await;
    let exposure = env.assignment(cart, EXPOSURE).await.expect("derived");
    assert_eq!(exposure.value.as_deref(), Some("interne"));
    assert!(!env.anomaly(cart, "kube.label.not_admitted").await);
    // The label gone: what the module set is removed, the namespace kept.
    env.applied(vec![deployment(
        env.cluster,
        "payments",
        "ledger",
        Some(json!({})),
    )])
    .await;
    assert!(env.assignment(ledger, EXPOSURE).await.is_none());
    assert!(env.assignment(ledger, NAMESPACE).await.is_some());
    // A value typed wins: the label back neither replaces it, nor its departure removes it.
    let property = env.id("property_declaration", "key", EXPOSURE).await;
    env.applied(vec![Operation::AssignProperty {
        object: Ref::Id(ledger),
        kind: workload_kind(),
        property: Ref::Id(property),
        values: vec![RawInput::Text("externe".into())],
    }])
    .await;
    for labels in [db(), Some(json!({}))] {
        env.applied(vec![deployment(env.cluster, "payments", "ledger", labels)])
            .await;
        let typed = env.assignment(ledger, EXPOSURE).await.expect("typed");
        assert_eq!(typed.provenance, "input");
        assert_eq!(typed.value.as_deref(), Some("externe"));
    }
    // The mapping closed: the value the module set on `cart` is removed at its next
    // upsert.
    let id = env.id("kube.label_mapping", "label_value", "db").await;
    env.applied(vec![op(
        "set_label_mapping",
        json!({"action": "close", "id": {"id": id}}),
    )])
    .await;
    env.applied(vec![deployment(env.cluster, "shop", "cart", None)])
        .await;
    assert!(env.assignment(cart, EXPOSURE).await.is_none());
}

/// REQ-019-33 (Q-019-39): `kube.namespace` is set by `kube.sync` alone: an assignment,
/// a "no value" or a removal the author writes, alone or within a period, is refused by
/// `kube.namespace.managed`; the value the server set stays.
#[kartenn_testing::pg_test]
async fn namespace_axis_managed(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.applied(vec![deployment(env.cluster, "payments", "ledger", None)])
        .await;
    let ledger = env.workload("ledger").await;
    let set = env.assignment(ledger, NAMESPACE).await.expect("set");
    let property = env.id("property_declaration", "key", NAMESPACE).await;
    let (object, kind, property) = (Ref::Id(ledger), workload_kind(), Ref::Id(property));
    let forged = Operation::AssignProperty {
        object,
        kind,
        property,
        values: vec![RawInput::External {
            system: "kube".into(),
            id: format!("{}/shop", env.cluster),
            label: Some("prod/shop".into()),
        }],
    };
    // From tomorrow on, while the workload lives.
    let tomorrow = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros()
        + 86_400_000_000;
    let within = Operation::Within {
        period: kartenn_service::operations::PeriodSpec {
            start: i64::try_from(tomorrow).unwrap(),
            end: None,
        },
        operation: Box::new(forged.clone()),
    };
    for operation in [
        forged,
        Operation::ClearProperty {
            object,
            kind,
            property,
        },
        Operation::UnassignProperty {
            object,
            kind,
            property,
        },
        within,
    ] {
        env.refused(vec![operation], "kube.namespace.managed").await;
    }
    let kept = env.assignment(ledger, NAMESPACE).await.expect("kept");
    assert_eq!(kept.external, set.external);
    assert_eq!(kept.changeset, set.changeset);
}
