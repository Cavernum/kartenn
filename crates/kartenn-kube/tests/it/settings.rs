//! The settings of the module through the write pipeline (spec 019, REQ-019-34, 39,
//! Q-019-35; task 019-7c): mappings of labels to values of axes, admitted values of an
//! axis in a namespace and tracked labels, three global types written by changeset under
//! `kube.settings.edit` and read under `kube.cluster.read`.

use std::sync::Arc;

use kartenn_core::objects::Nature;
use kartenn_kube::Kube;
use kartenn_machines::Machines;
use kartenn_service::authz::{Authorizer, Grants, Unrestricted};
use kartenn_service::declarations::{ModuleDeclarations, core_declarations_for, sync_modules};
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::{DeclarationInput, RawInput, RawLabels, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

/// The axis of the tests, a closed list carried by the workloads (and by the namespaces).
const EXPOSURE: &str = "custom.exposure";

struct Env {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    connector: Uuid,
    cluster: Uuid,
    /// The namespace `payments` of the cluster.
    payments: Uuid,
}

impl Env {
    /// Both modules migrated and declared, a cluster and its namespace `payments`, the
    /// axis `custom.exposure` of the values `externe` and `interne`.
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
            payments: Uuid::nil(),
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
        env.applied(vec![namespace(env.cluster, "upsert")]).await;
        env.payments = env.id("kube.namespace", "name", "payments").await;
        env.declare(
            EXPOSURE,
            &["kube.namespace", "kube.workload"],
            &["externe", "interne"],
        )
        .await;
        env
    }

    /// `operations` submitted by the connector when they are a batch, by the author
    /// otherwise, under `authorizer`.
    async fn submit_with(
        &self,
        authorizer: &impl Authorizer,
        operations: Vec<Operation>,
    ) -> Outcome {
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
        submit(&self.store, authorizer, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn submit(&self, operations: Vec<Operation>) -> Outcome {
        self.submit_with(&Unrestricted, operations).await
    }

    async fn applied(&self, operations: Vec<Operation>) {
        let outcome = self.submit(operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    /// Refused by `rule`, its message holding `says`.
    async fn refused(&self, operations: Vec<Operation>, rule: &str, says: &str) {
        let outcome = self.submit(operations).await;
        assert_eq!(rules(&outcome), [rule], "{outcome:?}");
        assert!(messages(&outcome).contains(says), "{says}: {outcome:?}");
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

    /// A domain `slug` owned by `owner`, with the prefix `prefix` of `nature`.
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

    /// The closed list `key` of `values`, carried by `carriers`.
    async fn declare(&self, key: &str, carriers: &[&str], values: &[&str]) {
        self.applied(declaration(key, carriers, values)).await;
    }

    /// The rows of `table` living now, as JSON without their columns of time, by id.
    async fn living(&self, table: &str) -> Vec<Value> {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT to_jsonb(t) - 'valid_period' - 'sys_start' - 'changeset_id'
               FROM {table} t WHERE valid_period @> now() ORDER BY 1::text"
        )))
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    /// Whether the namespace `id` living now is absent from its cluster.
    async fn absent(&self, id: Uuid) -> bool {
        sqlx::query_scalar(
            "SELECT absent_since IS NOT NULL FROM kube.namespace
              WHERE id = $1 AND valid_period @> now()",
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

    /// The versions of `id` in `table`, current and past.
    async fn versions(&self, table: &str, id: Uuid) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM {table} WHERE id = $1"
        )))
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

/// The declaration of the closed list `key` of `values`, carried by `carriers`.
fn declaration(key: &str, carriers: &[&str], values: &[&str]) -> Vec<Operation> {
    let mut operations = vec![Operation::DeclareProperty {
        local: Some(1),
        declaration: DeclarationInput {
            key: key.into(),
            value_type: TypeInput::Enum { values_of: None },
            cardinality: kartenn_core::values::Cardinality::Single,
            carriers: carriers.iter().map(|&c| c.to_owned()).collect(),
            inherit: None,
            module: None,
            obligations: Vec::new(),
            expected: None,
            labels: None,
        },
    }];
    operations.extend(values.iter().map(|&value| add_value(Ref::Local(1), value)));
    operations
}

fn add_value(property: Ref, key: &str) -> Operation {
    Operation::AddEnumValue {
        local: None,
        property,
        key: key.into(),
        labels: RawLabels::new(),
    }
}

/// `kube.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("kube.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// `kube.sync` of `action` on the namespace `payments` of `cluster`.
fn namespace(cluster: Uuid, action: &str) -> Operation {
    op(
        "sync",
        json!({
            "action": action,
            "type": "kube.namespace",
            "cluster": {"id": cluster},
            "name": "payments",
        }),
    )
}

/// The mapping of the label `tier=<label>` to `<axis>: <value>`, upserted.
fn mapping(label: &str, axis: &str, value: &str) -> Operation {
    op(
        "set_label_mapping",
        json!({
            "action": "upsert",
            "label_key": "tier",
            "label_value": label,
            "axis": axis,
            "value": value,
        }),
    )
}

fn close_mapping(id: Uuid) -> Operation {
    op(
        "set_label_mapping",
        json!({"action": "close", "id": {"id": id}}),
    )
}

fn admitted(namespace: Uuid, axis: &str, values: &[&str]) -> Operation {
    op(
        "set_admitted_values",
        json!({"namespace": {"id": namespace}, "axis": axis, "values": values}),
    )
}

fn tracked(keys: &[&str]) -> Operation {
    op("set_tracked_labels", json!({"keys": keys}))
}

fn rules(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        other => panic!("not refused: {other:?}"),
    }
}

fn messages(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Refused { violations, .. } => violations
            .iter()
            .map(|v| v.message.clone())
            .collect::<Vec<_>>()
            .join("; "),
        other => panic!("not refused: {other:?}"),
    }
}

/// Refused as a write of an object without its permission, `permission` named.
fn refused_for(outcome: &Outcome, permission: &str) {
    assert!(
        rules(outcome).iter().all(|r| *r == "core.object.forbidden"),
        "{outcome:?}"
    );
    assert!(
        messages(outcome).contains(permission),
        "{permission}: {outcome:?}"
    );
}

/// REQ-019-34, 39 (REQ-027-5): the settings of the module are written under
/// `kube.settings.*` alone, global, and read with the clusters. A team that holds only
/// the settings of the axis declares it and its values but writes no mapping, no admitted
/// values, no tracked label, `kube.settings.edit` named; the other way round, the settings
/// of the module write no value of the axis.
#[kartenn_testing::pg_test]
async fn label_mapping_needs_kube_settings(pool: PgPool) {
    let env = Env::new(&pool).await;
    for kind in [
        "kube.label_mapping",
        "kube.admitted_values",
        "kube.tracked_label",
    ] {
        assert_eq!(
            kartenn_core::authz::type_read_permission(kind),
            "kube.cluster.read"
        );
        assert_eq!(
            kartenn_core::authz::type_edit_permission(kind),
            "kube.settings.edit"
        );
    }
    let module = Kube.permissions();
    let settings: Vec<_> = module
        .iter()
        .filter(|p| p.name.starts_with("kube.settings."))
        .collect();
    assert_eq!(
        settings.iter().map(|p| p.name).collect::<Vec<_>>(),
        [
            "kube.settings.edit",
            "kube.settings.apply",
            "kube.settings.approval.self"
        ]
    );
    assert!(settings.iter().all(|p| p.global_only));
    assert!(kartenn_core::authz::permission("kube.settings.read").is_none());

    // The custom axes are the settings of the core (spec 027).
    let axis_team = Grants::new()
        .everywhere(env.author, "core.settings.edit")
        .everywhere(env.author, "core.settings.approval.self");
    let declared = env
        .submit_with(
            &axis_team,
            declaration("custom.zone", &["kube.workload"], &["a"]),
        )
        .await;
    assert!(matches!(declared, Outcome::Applied { .. }), "{declared:?}");
    for operation in [
        mapping("db", EXPOSURE, "interne"),
        admitted(env.payments, EXPOSURE, &["interne"]),
        tracked(&["tier"]),
    ] {
        let refused = env.submit_with(&axis_team, vec![operation]).await;
        refused_for(&refused, "kube.settings.edit");
    }

    let kube_team = Grants::new()
        .everywhere(env.author, "kube.settings.edit")
        .everywhere(env.author, "kube.settings.approval.self")
        .everywhere(env.author, "kube.namespace.read");
    for operation in [
        mapping("db", EXPOSURE, "interne"),
        admitted(env.payments, EXPOSURE, &["interne"]),
        tracked(&["tier"]),
    ] {
        let applied = env.submit_with(&kube_team, vec![operation]).await;
        assert!(matches!(applied, Outcome::Applied { .. }), "{applied:?}");
    }
    let exposure = env.id("property_declaration", "key", EXPOSURE).await;
    let refused = env
        .submit_with(&kube_team, vec![add_value(Ref::Id(exposure), "dmz")])
        .await;
    refused_for(&refused, "core.settings.edit");
    // The admitted values cite the namespace: its read is asked.
    let blind = Grants::new()
        .everywhere(env.author, "kube.settings.edit")
        .everywhere(env.author, "kube.settings.approval.self");
    let refused = env
        .submit_with(&blind, vec![admitted(env.payments, EXPOSURE, &["externe"])])
        .await;
    assert!(
        messages(&refused).contains("kube.namespace.read"),
        "{refused:?}"
    );
    // Without `approval.self`, a proposal.
    let proposer = Grants::new().everywhere(env.author, "kube.settings.edit");
    let pending = env
        .submit_with(&proposer, vec![tracked(&["tier", "app"])])
        .await;
    assert!(matches!(pending, Outcome::Pending { .. }), "{pending:?}");
}

/// Q-019-35: a mapping is created by its label and axis, changed in place, closed by its
/// identifier; admitted values are replaced as a set, closed when empty, and keep their
/// namespace from closing; the tracked labels are replaced as a list. Axes, values and
/// labels are checked.
#[kartenn_testing::pg_test]
async fn settings_written_by_changeset(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.declare("custom.site_only", &["domain"], &["a"]).await;
    env.declare("custom.free", &["kube.workload"], &[]).await;

    // Mappings.
    env.applied(vec![mapping("db", EXPOSURE, "interne")]).await;
    let rows = env.living("kube.label_mapping").await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let id: Uuid = serde_json::from_value(rows[0]["id"].clone()).unwrap();
    assert_eq!(
        rows[0],
        json!({"id": id, "label_key": "tier", "label_value": "db",
               "axis_key": EXPOSURE, "axis_value": "interne"})
    );
    env.applied(vec![mapping("db", EXPOSURE, "externe")]).await;
    let rows = env.living("kube.label_mapping").await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["id"], json!(id), "changed in place");
    assert_eq!(rows[0]["axis_value"], "externe");
    // A new version from now on, the former one kept in the history.
    assert_eq!(env.versions("kube.label_mapping", id).await, 2);
    for (operation, says) in [
        (
            mapping("db", "kube.namespace", "x"),
            "kube.namespace is set by kube.sync alone",
        ),
        (
            mapping("db", "custom.none", "x"),
            "no property custom.none is declared",
        ),
        (
            mapping("db", "custom.site_only", "a"),
            "not carried by kube.workload",
        ),
        (
            mapping("db", EXPOSURE, "dmz"),
            "dmz is not a value of custom.exposure",
        ),
        (
            op(
                "set_label_mapping",
                json!({"action": "upsert", "label_key": "tier"}),
            ),
            "label_value: required",
        ),
        (
            op(
                "set_label_mapping",
                json!({"action": "close", "id": {"id": env.payments}}),
            ),
            "is not a living label mapping",
        ),
        (
            op("set_label_mapping", json!({"action": "drop"})),
            "upsert or close",
        ),
    ] {
        env.refused(vec![operation], "kube.operation.invalid", says)
            .await;
    }
    env.refused(
        vec![mapping("-db", EXPOSURE, "interne")],
        "kube.label.invalid",
        "-db",
    )
    .await;
    // The same new mapping twice in a changeset.
    env.refused(
        vec![
            mapping("web", EXPOSURE, "interne"),
            mapping("web", EXPOSURE, "externe"),
        ],
        "kube.operation.invalid",
        "given twice",
    )
    .await;
    env.applied(vec![close_mapping(id)]).await;
    assert!(env.living("kube.label_mapping").await.is_empty());
    env.refused(
        vec![close_mapping(id)],
        "kube.operation.invalid",
        "is not a living label mapping",
    )
    .await;

    // Admitted values.
    env.applied(vec![admitted(env.payments, EXPOSURE, &["interne"])])
        .await;
    let rows = env.living("kube.admitted_values").await;
    let admitted_id = rows[0]["id"].clone();
    assert_eq!(
        rows,
        [
            json!({"id": admitted_id, "namespace_id": env.payments, "axis_key": EXPOSURE,
                "axis_values": ["interne"]})
        ]
    );
    env.applied(vec![admitted(
        env.payments,
        EXPOSURE,
        &["interne", "externe", "interne"],
    )])
    .await;
    let rows = env.living("kube.admitted_values").await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["id"], admitted_id, "replaced in place");
    assert_eq!(
        rows[0]["axis_values"],
        json!(["externe", "interne"]),
        "a set"
    );
    for (operation, says) in [
        (
            admitted(env.payments, EXPOSURE, &["dmz"]),
            "dmz is not a value",
        ),
        (
            admitted(env.cluster, EXPOSURE, &["interne"]),
            "is not a living namespace",
        ),
        (
            admitted(env.payments, "kube.namespace", &["x"]),
            "set by kube.sync alone",
        ),
        (
            admitted(env.payments, "custom.free", &["x"]),
            "x is not a value of custom.free",
        ),
    ] {
        env.refused(vec![operation], "kube.operation.invalid", says)
            .await;
    }
    // The namespace stays while admitted values cite it: closed by hand, refused.
    let by_hand = Operation::Close {
        kind: ObjectKind::Module(
            kartenn_service::operations::ModuleKind::parse("kube.namespace").unwrap(),
        ),
        id: Ref::Id(env.payments),
        expected: None,
        cascade: false,
    };
    let outcome = env.submit(vec![by_hand.clone()]).await;
    assert_eq!(rules(&outcome), ["core.object.dependents"], "{outcome:?}");
    // Closed by the connector, marked absent instead, the batch accepted
    // (`kube.namespace.held`), twice the same.
    for _ in 0..2 {
        let outcome = env.submit(vec![namespace(env.cluster, "close")]).await;
        let Outcome::Applied { warnings, .. } = &outcome else {
            panic!("not applied: {outcome:?}");
        };
        let held: Vec<_> = warnings
            .iter()
            .filter(|w| w.rule.id == "kube.namespace.held")
            .collect();
        assert_eq!(held.len(), 1, "{outcome:?}");
        assert!(
            held[0].message.contains(admitted_id.as_str().unwrap()),
            "{held:?}"
        );
        assert_eq!(
            env.id("kube.namespace", "name", "payments").await,
            env.payments
        );
        assert!(env.absent(env.payments).await, "absent");
        assert!(env.anomaly(env.payments, "kube.namespace.held").await);
    }
    env.applied(vec![admitted(env.payments, EXPOSURE, &[])])
        .await;
    assert!(env.living("kube.admitted_values").await.is_empty());
    assert!(
        !env.anomaly(env.payments, "kube.namespace.held").await,
        "let go"
    );
    // Nothing admitted already: nothing written.
    env.applied(vec![admitted(env.payments, EXPOSURE, &[])])
        .await;
    // Let go, the connector closes it.
    env.applied(vec![namespace(env.cluster, "close")]).await;
    assert!(env.living("kube.namespace").await.is_empty());

    // Tracked labels.
    env.applied(vec![tracked(&["tier", "app", "tier"])]).await;
    let keys = |rows: &[Value]| -> Vec<String> {
        let mut keys: Vec<String> = rows
            .iter()
            .map(|r| r["label_key"].as_str().unwrap().to_owned())
            .collect();
        keys.sort();
        keys
    };
    let rows = env.living("kube.tracked_label").await;
    assert_eq!(keys(&rows), ["app", "tier"]);
    let tier = env.id("kube.tracked_label", "label_key", "tier").await;
    env.applied(vec![tracked(&["tier", "example.org/team"])])
        .await;
    assert_eq!(
        keys(&env.living("kube.tracked_label").await),
        ["example.org/team", "tier"]
    );
    assert_eq!(
        env.id("kube.tracked_label", "label_key", "tier").await,
        tier
    );
    env.applied(vec![tracked(&["tier"])]).await;
    assert_eq!(keys(&env.living("kube.tracked_label").await), ["tier"]);
    env.applied(vec![tracked(&[])]).await;
    assert!(env.living("kube.tracked_label").await.is_empty());
    env.applied(vec![tracked(&[])]).await;
    env.refused(vec![tracked(&["a/b/c"])], "kube.label.invalid", "a/b/c")
        .await;
    env.refused(
        vec![op("set_tracked_labels", json!({"keys": ["a"], "extra": 1}))],
        "kube.operation.invalid",
        "unknown field `extra`",
    )
    .await;
    // The operation a `set_tracked_labels` adds is not one a client writes.
    env.refused(
        vec![op(
            "set_tracked_labels",
            json!({"completed": {"create": "x"}}),
        )],
        "kube.operation.invalid",
        "keys: required",
    )
    .await;
}
