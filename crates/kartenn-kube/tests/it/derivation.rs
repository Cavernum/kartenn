//! The axes the server derives for a workload (spec 019, REQ-019-33, 34; Q-019-33, 34,
//! 37 to 39; task 019-7d2): at each `upsert` of a workload, `kube.sync` sets its axis
//! `kube.namespace` and the values its labels give, admitted in its namespace, of the
//! provenance `module:kube#label:<key>`; a value typed wins; `kube.namespace` is written
//! by `kube.sync` alone.

use std::sync::Arc;

use kartenn_core::objects::Nature;
use kartenn_core::values::Cardinality;
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
    registry: Arc<ModuleRegistry>,
    author: Uuid,
    connector: Uuid,
    cluster: Uuid,
    /// The owner of the cluster and its domains.
    acme: Uuid,
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
            registry,
            author,
            connector,
            cluster: Uuid::nil(),
            acme: Uuid::nil(),
        };
        env.applied(vec![Operation::CreateEntity {
            local: None,
            slug: "acme".into(),
            parent: None,
        }])
        .await;
        let acme = env.id("entity", "slug", "acme").await;
        env.acme = acme;
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
        env.applied(declaration(EXPOSURE, Cardinality::Single))
            .await;
        env.applied(vec![tracked(&["tier"])]).await;
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

    /// `identity` granted `permissions` on `acme` and what it owns, or everywhere.
    async fn grant(&self, identity: Uuid, permissions: &[&str], acme: bool) {
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
                 VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
            )
            .bind(role)
            .bind(permission)
            .execute(&self.pool)
            .await
            .unwrap();
        }
        let (scope, id) = if acme {
            ("entity", Some(self.acme))
        } else {
            ("global", None)
        };
        sqlx::query(kartenn_testing::BIND_IDENTITY)
            .bind(identity)
            .bind(role)
            .bind(scope)
            .bind(id)
            .execute(&self.pool)
            .await
            .unwrap();
    }

    /// A new person holding `permissions` everywhere.
    async fn granted(&self, permissions: &[&str]) -> Uuid {
        let identity: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'g') RETURNING id",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap();
        self.grant(identity, permissions, false).await;
        identity
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

    /// The keys of the set of `key` on `object` living now, and its provenance.
    async fn set_of(&self, object: Uuid, key: &str) -> Option<(Vec<String>, String)> {
        sqlx::query_as(
            "SELECT ARRAY(SELECT v.key FROM property_enum_value v
                           WHERE v.id = ANY(a.v_uuids) AND v.valid_period @> now()
                           ORDER BY v.key),
                    a.provenance
               FROM property_assignment a
               JOIN property_declaration d ON d.id = a.property_id AND d.valid_period @> now()
              WHERE a.object_id = $1 AND d.key = $2 AND a.valid_period @> now()",
        )
        .bind(object)
        .bind(key)
        .fetch_optional(&self.pool)
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

/// The closed list `key` of the values `externe` and `interne`, carried by the workloads.
fn declaration(key: &str, cardinality: Cardinality) -> Vec<Operation> {
    let values = ["externe", "interne"];
    let mut operations = vec![Operation::DeclareProperty {
        local: Some(1),
        declaration: DeclarationInput {
            key: key.into(),
            value_type: TypeInput::Enum { values_of: None },
            cardinality,
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

/// The mapping of the label `<key>=db` to `<axis>: <value>`.
fn mapping_of(key: &str, axis: &str, value: &str) -> Operation {
    op(
        "set_label_mapping",
        json!({
            "action": "upsert", "label_key": key, "label_value": "db", "axis": axis,
            "value": value,
        }),
    )
}

/// The mapping of the label `tier=<label>` to `<axis>: <value>`.
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
    admitted_on(namespace, EXPOSURE, values)
}

fn admitted_on(namespace: Uuid, axis: &str, values: &[&str]) -> Operation {
    op(
        "set_admitted_values",
        json!({"namespace": {"id": namespace}, "axis": axis, "values": values}),
    )
}

fn tracked(keys: &[&str]) -> Operation {
    op("set_tracked_labels", json!({"keys": keys}))
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
    // Admitted in `shop` afterwards: set by the changeset of the settings (Q-019-36), the
    // next upsert unchanged, the anomaly closed.
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
    // A restoration rewrites the assignment: refused too.
    let restore = Operation::Restore {
        kind,
        id: object,
        property: Some(env.id("property_declaration", "key", NAMESPACE).await),
        before: set.changeset,
    };
    match env.submit(vec![restore]).await {
        Outcome::Refused { violations, .. } => assert!(
            violations
                .iter()
                .any(|v| v.rule.id == "kube.namespace.managed"),
            "{violations:?}"
        ),
        other => panic!("not refused: {other:?}"),
    }
    let kept = env.assignment(ledger, NAMESPACE).await.expect("kept");
    assert_eq!(kept.external, set.external);
    assert_eq!(kept.changeset, set.changeset);
    // What `kube.sync` plans still passes: the cluster renamed, its next upsert labels the
    // namespace anew.
    env.applied(vec![op(
        "update_cluster",
        json!({"id": {"id": env.cluster}, "slug": "prod-new"}),
    )])
    .await;
    env.applied(vec![deployment(env.cluster, "payments", "ledger", None)])
        .await;
    let renamed = env.assignment(ledger, NAMESPACE).await.expect("set");
    assert_eq!(
        renamed
            .external
            .and_then(|e| e["label"].as_str().map(str::to_owned)),
        Some("prod-new/payments".to_owned())
    );
    assert_eq!(renamed.provenance, "module:kube#namespace");
}

/// Q-019-40: two tracked labels on one axis. On the set `custom.zones`, the union of
/// their admitted values, of a provenance that names both, written once for a batch
/// repeated; on the single value `custom.exposure`, nothing when they disagree, the value
/// the module had set removed and `kube.label.conflict` naming both labels, lifted once a
/// label leaves.
#[kartenn_testing::pg_test]
async fn labels_union_or_nothing(pool: PgPool) {
    const ZONES: &str = "custom.zones";
    let env = Env::new(&pool).await;
    let payments = env.namespace("payments").await;
    env.applied(declaration(ZONES, Cardinality::Set)).await;
    env.applied(vec![
        tracked(&["app", "tier"]),
        mapping_of("app", EXPOSURE, "interne"),
        mapping_of("tier", EXPOSURE, "externe"),
        mapping_of("app", ZONES, "interne"),
        mapping_of("tier", ZONES, "externe"),
        admitted(payments, &["externe", "interne"]),
        admitted_on(payments, ZONES, &["externe", "interne"]),
    ])
    .await;
    let upsert = |labels: Value| deployment(env.cluster, "payments", "ledger", Some(labels));
    env.applied(vec![upsert(json!({"tier": "db"}))]).await;
    let ledger = env.workload("ledger").await;
    let exposure = env.assignment(ledger, EXPOSURE).await.expect("one label");
    assert_eq!(exposure.value.as_deref(), Some("externe"));
    assert_eq!(
        env.set_of(ledger, ZONES).await,
        Some((vec!["externe".into()], "module:kube#label:tier".into()))
    );
    // Both labels: the union on the set, nothing on the single value.
    let both = || json!({"app": "db", "tier": "db"});
    env.applied(vec![upsert(both())]).await;
    assert!(env.assignment(ledger, EXPOSURE).await.is_none());
    assert!(env.anomaly(ledger, "kube.label.conflict").await);
    let message: String = sqlx::query_scalar(
        "SELECT message FROM anomaly
          WHERE object_id = $1 AND rule_id = 'kube.label.conflict' AND closed_at IS NULL",
    )
    .bind(ledger)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        message.contains("app=db") && message.contains("tier=db"),
        "{message}"
    );
    assert_eq!(
        env.set_of(ledger, ZONES).await,
        Some((
            vec!["externe".into(), "interne".into()],
            "module:kube#label:app,tier".into()
        ))
    );
    // The same batch again: neither axis written anew.
    let (zones, exposures) = (
        env.versions(ledger, ZONES).await,
        env.versions(ledger, EXPOSURE).await,
    );
    env.applied(vec![upsert(both())]).await;
    env.applied(vec![deployment(env.cluster, "payments", "ledger", None)])
        .await;
    assert_eq!(env.versions(ledger, ZONES).await, zones);
    assert_eq!(env.versions(ledger, EXPOSURE).await, exposures);
    // A label gone: the conflict lifted, the value of the other set.
    env.applied(vec![upsert(json!({"app": "db"}))]).await;
    assert!(!env.anomaly(ledger, "kube.label.conflict").await);
    let exposure = env.assignment(ledger, EXPOSURE).await.expect("one label");
    assert_eq!(exposure.value.as_deref(), Some("interne"));
    assert_eq!(exposure.provenance, "module:kube#label:app");
}

/// Q-019-41: a label whose key is not tracked gives nothing, and warns of nothing; once
/// tracked, the next upsert sets its value.
#[kartenn_testing::pg_test]
async fn untracked_label_ignored(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (payments, shop) = (env.namespace("payments").await, env.namespace("shop").await);
    env.applied(vec![
        mapping_of("app", EXPOSURE, "interne"),
        admitted(payments, &["interne"]),
        admitted(shop, &["externe"]),
    ])
    .await;
    let app = || Some(json!({"app": "db"}));
    env.applied(vec![
        deployment(env.cluster, "payments", "ledger", app()),
        deployment(env.cluster, "shop", "cart", app()),
    ])
    .await;
    let (ledger, cart) = (env.workload("ledger").await, env.workload("cart").await);
    assert!(env.assignment(ledger, EXPOSURE).await.is_none());
    assert!(!env.anomaly(cart, "kube.label.not_admitted").await);
    env.applied(vec![tracked(&["app", "tier"])]).await;
    env.applied(vec![
        deployment(env.cluster, "payments", "ledger", None),
        deployment(env.cluster, "shop", "cart", None),
    ])
    .await;
    let exposure = env.assignment(ledger, EXPOSURE).await.expect("tracked");
    assert_eq!(exposure.value.as_deref(), Some("interne"));
    assert_eq!(exposure.provenance, "module:kube#label:app");
    assert!(env.anomaly(cart, "kube.label.not_admitted").await);
}

/// The changeset of the living mapping of `tier=<label>`.
async fn mapping_changeset(env: &Env, label: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT changeset_id FROM kube.label_mapping
          WHERE label_value = $1 AND valid_period @> now()",
    )
    .bind(label)
    .fetch_one(&env.pool)
    .await
    .unwrap()
}

/// REQ-019-34, Q-019-36, 42: the changeset that writes settings re-derives, in its
/// transaction, the workloads they concern from their stored labels, on the final state of
/// all its settings: a mapping and the values that admit it, written together, set the
/// value; a value not admitted warns, the anomaly closed once admitted; a mapping closed
/// removes what it gave; a label key tracked afterwards gives its value. A workload no
/// setting concerns is left as it is.
#[kartenn_testing::pg_test]
async fn settings_rederive_workloads(pool: PgPool) {
    let env = Env::new(&pool).await;
    let payments = env.namespace("payments").await;
    env.applied(vec![
        deployment(
            env.cluster,
            "payments",
            "ledger",
            Some(json!({"tier": "db"})),
        ),
        deployment(
            env.cluster,
            "payments",
            "cache",
            Some(json!({"tier": "cache"})),
        ),
        deployment(
            env.cluster,
            "payments",
            "front",
            Some(json!({"app": "front"})),
        ),
    ])
    .await;
    let (ledger, cache, front) = (
        env.workload("ledger").await,
        env.workload("cache").await,
        env.workload("front").await,
    );
    let batch = env.written_by(ledger).await;
    // Together: neither alone would give a value.
    env.applied(vec![
        mapping("db", EXPOSURE, "interne"),
        admitted(payments, &["interne"]),
    ])
    .await;
    let exposure = env.assignment(ledger, EXPOSURE).await.expect("re-derived");
    assert_eq!(exposure.value.as_deref(), Some("interne"));
    assert_eq!(exposure.provenance, "module:kube#label:tier");
    assert_eq!(exposure.changeset, mapping_changeset(&env, "db").await);
    assert_eq!(
        env.written_by(ledger).await,
        batch,
        "the workload unchanged"
    );
    assert_eq!(env.versions(ledger, EXPOSURE).await, 1);
    assert!(env.assignment(cache, EXPOSURE).await.is_none());
    // A value not admitted: ignored, the warning raised on the workload not written.
    env.applied(vec![mapping("cache", EXPOSURE, "externe")])
        .await;
    assert!(env.assignment(cache, EXPOSURE).await.is_none());
    assert!(env.anomaly(cache, "kube.label.not_admitted").await);
    env.applied(vec![admitted(payments, &["externe", "interne"])])
        .await;
    let exposure = env.assignment(cache, EXPOSURE).await.expect("admitted");
    assert_eq!(exposure.value.as_deref(), Some("externe"));
    assert!(!env.anomaly(cache, "kube.label.not_admitted").await);
    assert_eq!(env.versions(ledger, EXPOSURE).await, 1, "ledger untouched");
    // The mapping closed: what it gave is removed.
    let db = env.id("kube.label_mapping", "label_value", "db").await;
    env.applied(vec![op(
        "set_label_mapping",
        json!({"action": "close", "id": {"id": db}}),
    )])
    .await;
    assert!(env.assignment(ledger, EXPOSURE).await.is_none());
    assert!(env.assignment(cache, EXPOSURE).await.is_some());
    // A key tracked with its mapping, in one changeset.
    env.applied(vec![
        op(
            "set_label_mapping",
            json!({
                "action": "upsert", "label_key": "app", "label_value": "front",
                "axis": EXPOSURE, "value": "externe",
            }),
        ),
        tracked(&["app", "tier"]),
    ])
    .await;
    let exposure = env.assignment(front, EXPOSURE).await.expect("tracked");
    assert_eq!(exposure.provenance, "module:kube#label:app");
    // The key no longer tracked: removed again.
    env.applied(vec![tracked(&["tier"])]).await;
    assert!(env.assignment(front, EXPOSURE).await.is_none());
    assert!(env.assignment(cache, EXPOSURE).await.is_some());
    // The settings are written in the present: a period of their own is refused.
    let start: i64 = sqlx::query_scalar(
        "SELECT (extract(epoch FROM now() + interval '1 day') * 1000000)::bigint",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for setting in [
        mapping("db", EXPOSURE, "interne"),
        admitted(payments, &["interne"]),
        tracked(&["app", "tier"]),
    ] {
        env.refused(
            vec![Operation::Within {
                period: kartenn_service::operations::PeriodSpec { start, end: None },
                operation: Box::new(setting),
            }],
            "kube.operation.invalid",
        )
        .await;
    }
    assert!(env.assignment(ledger, EXPOSURE).await.is_none());
}

/// Q-019-36 (precision of 2026-11-12), under the RBAC of the store: the author of the
/// settings needs the rights of the property on the effect, refused without
/// `prop.<axis>.edit`, pending without `prop.<axis>.approval.self`; the connector, under a
/// role equivalent to `kube_connector` (ADR 0044, 7), is not held by a mapping to an axis
/// `custom.*`.
#[kartenn_testing::pg_test]
async fn settings_rederive_rights(pool: PgPool) {
    let env = Env::new(&pool).await;
    let payments = env.namespace("payments").await;
    env.applied(vec![admitted(payments, &["externe", "interne"])])
        .await;
    env.applied(vec![deployment(
        env.cluster,
        "payments",
        "ledger",
        Some(json!({"tier": "db"})),
    )])
    .await;
    let ledger = env.workload("ledger").await;
    let settings = [
        "kube.cluster.read",
        "kube.namespace.read",
        "kube.workload.read",
        "kube.settings.edit",
        "kube.settings.approval.self",
    ];
    let blind = env.granted(&settings).await;
    let outcome = env
        .submit_as(blind, vec![mapping("db", EXPOSURE, "interne")])
        .await;
    let Outcome::Refused { violations, .. } = &outcome else {
        panic!("not refused: {outcome:?}");
    };
    let forbidden = violations
        .iter()
        .find(|v| v.rule.id == "prop.assignment.forbidden_effect")
        .unwrap_or_else(|| panic!("{outcome:?}"));
    assert!(forbidden.subjects.contains(&ledger), "{forbidden:?}");
    assert!(
        forbidden.message.contains("prop.custom.exposure.edit"),
        "{forbidden:?}"
    );
    let editor = env
        .granted(&[&settings[..], &["prop.custom.exposure.edit"]].concat())
        .await;
    let outcome = env
        .submit_as(editor, vec![mapping("db", EXPOSURE, "interne")])
        .await;
    assert!(matches!(outcome, Outcome::Pending { .. }), "{outcome:?}");
    assert!(env.assignment(ledger, EXPOSURE).await.is_none());
    let applier = env
        .granted(
            &[
                &settings[..],
                &[
                    "prop.custom.exposure.edit",
                    "prop.custom.exposure.approval.self",
                ],
            ]
            .concat(),
        )
        .await;
    let outcome = env
        .submit_as(applier, vec![mapping("db", EXPOSURE, "interne")])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let exposure = env.assignment(ledger, EXPOSURE).await.expect("re-derived");
    assert_eq!(exposure.value.as_deref(), Some("interne"));
    // The connector, under its role on the domains of the cluster: a mapping it never
    // asked for does not hold its batch.
    env.applied(vec![mapping("cache", EXPOSURE, "externe")])
        .await;
    let connector = [
        "kube.cluster.read",
        "kube.node.read",
        "kube.node.edit",
        "kube.node.approval.self",
        "kube.namespace.read",
        "kube.namespace.edit",
        "kube.namespace.approval.self",
        "kube.workload.read",
        "kube.workload.edit",
        "kube.workload.approval.self",
        "kube.service.read",
        "kube.service.edit",
        "kube.service.approval.self",
        "kube.ingress.read",
        "kube.ingress.edit",
        "kube.ingress.approval.self",
        "core.domain.read",
        "core.prefix.read",
        "core.prefix.edit",
        "core.prefix.approval.self",
        "core.address.read",
        "core.address.edit",
        "core.address.approval.self",
        "prop.*.edit",
        "prop.*.approval.self",
        "kube.observation.record",
    ];
    env.grant(env.connector, &connector, true).await;
    let outcome = env
        .submit_as(
            env.connector,
            vec![deployment(
                env.cluster,
                "payments",
                "cache",
                Some(json!({"tier": "cache"})),
            )],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let cache = env.workload("cache").await;
    let exposure = env.assignment(cache, EXPOSURE).await.expect("derived");
    assert_eq!(exposure.value.as_deref(), Some("externe"));
    assert!(env.assignment(cache, NAMESPACE).await.is_some());
}
