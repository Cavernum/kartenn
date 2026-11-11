//! The journal of the pods observed (REQ-019-16, ADR 0043; task 019-6a): a batch of the
//! connector written under the role of the application, checked at the entry and refused
//! whole, append only down to the rights of PostgreSQL, outside the model and the
//! changesets, its fingerprint recomputed from what it wrote.

use std::net::IpAddr;
use std::sync::Arc;

use kartenn_core::objects::Nature;
use kartenn_kube::Kube;
use kartenn_kube::observations::{
    Batch, Closing, Invalid, Line, Opening, Reason, RecordError, Recorded, content_hash, record,
};
use kartenn_machines::Machines;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{ModuleDeclarations, core_declarations_for, sync_modules};
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

/// `n` hours.
const fn hours(n: u64) -> std::time::Duration {
    std::time::Duration::from_secs(n * 3_600)
}
use sqlx::{PgPool, Postgres, Transaction};

/// A cluster `prod` and its connector; its node `worker-1` (pod range `198.51.100.0/26`,
/// address `192.0.2.21`), its namespace `payments` and workload `web`.
struct Env {
    pool: PgPool,
    store: PgStore,
    /// The administrator who declares the domains and the clusters.
    admin: Uuid,
    connector: Uuid,
    acme: Uuid,
    pods: Uuid,
    services: Uuid,
    prod: Uuid,
    node: Uuid,
    web: Uuid,
}

impl Env {
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
        let (connector, group) = crate::support::connector(pool, "connector").await;
        let admin = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let mut env = Self {
            pool: pool.clone(),
            store,
            admin,
            connector,
            acme: Uuid::nil(),
            pods: Uuid::nil(),
            services: Uuid::nil(),
            prod: Uuid::nil(),
            node: Uuid::nil(),
            web: Uuid::nil(),
        };
        env.applied(
            env.admin,
            vec![Operation::CreateEntity {
                local: Some(1),
                slug: "acme".into(),
                parent: None,
            }],
        )
        .await;
        env.acme = env.id("entity", "slug", "acme").await;
        env.pods = env
            .domain("pods", "198.51.100.0/24", Nature::Container)
            .await;
        env.services = env
            .domain("services", "203.0.113.0/26", Nature::Network)
            .await;
        env.prod = env.cluster("prod", group).await;
        (env.node, env.web) = env.populated(env.prod, connector, "198.51.100.0/26").await;
        env
    }

    async fn applied(&self, author: Uuid, operations: Vec<Operation>) {
        let request = ChangesetRequest {
            author,
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

    async fn id(&self, table: &str, column: &str, value: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE {column} = $1 AND valid_period @> now()"
        )))
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// A domain `slug` owned by `acme`, with the prefix `prefix` of `nature`.
    async fn domain(&self, slug: &str, prefix: &str, nature: Nature) -> Uuid {
        let owner: Uuid =
            sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'core.owner'")
                .fetch_one(&self.pool)
                .await
                .unwrap();
        self.applied(
            self.admin,
            vec![
                Operation::CreateDomain {
                    local: Some(1),
                    slug: slug.into(),
                },
                Operation::AssignProperty {
                    object: Ref::Local(1),
                    kind: ObjectKind::Domain,
                    property: Ref::Id(owner),
                    values: vec![RawInput::Text(self.acme.to_string())],
                },
                Operation::CreatePrefix {
                    local: None,
                    domain: Ref::Local(1),
                    prefix: prefix.into(),
                    nature,
                    point_to_point: false,
                    anycast: false,
                },
            ],
        )
        .await;
        self.id("addressing_domain", "slug", slug).await
    }

    /// A cluster `slug` whose connector group is `group`, in the pod container and the
    /// service network of `acme`.
    async fn cluster(&self, slug: &str, group: Uuid) -> Uuid {
        let prefix = |domain: Uuid| {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM prefix WHERE domain_id = $1 AND valid_period @> now()",
            )
            .bind(domain)
            .fetch_one(&self.pool)
        };
        let (container, network) = (
            prefix(self.pods).await.unwrap(),
            prefix(self.services).await.unwrap(),
        );
        self.applied(
            self.admin,
            vec![op(
                "create_cluster",
                json!({
                    "slug": slug,
                    "owner": {"id": self.acme},
                    "pod_domain": {"id": self.pods},
                    "service_domain": {"id": self.services},
                    "pod_containers": [{"id": container}],
                    "service_networks": [{"id": network}],
                    "connector_group": {"id": group},
                }),
            )],
        )
        .await;
        self.id("kube.cluster", "slug", slug).await
    }

    /// The node `worker-1` of `cluster` (pod range `range`, address `192.0.2.21`), its
    /// namespace `payments` and its workload `web`, written by `connector`: `(node, web)`.
    async fn populated(&self, cluster: Uuid, connector: Uuid, range: &str) -> (Uuid, Uuid) {
        self.applied(
            connector,
            vec![
                sync(
                    "node",
                    cluster,
                    "worker-1",
                    json!({"pod_cidrs": [range],
                           "addresses": [{"type": "InternalIP", "value": "192.0.2.21"},
                                         {"type": "Hostname", "value": "worker-1"}]}),
                ),
                sync("namespace", cluster, "payments", json!({})),
            ],
        )
        .await;
        self.applied(
            connector,
            vec![sync(
                "workload",
                cluster,
                "web",
                json!({"namespace": "payments", "kind": "deployment"}),
            )],
        )
        .await;
        let one = |sql: &'static str| {
            sqlx::query_scalar::<_, Uuid>(sql)
                .bind(cluster)
                .fetch_one(&self.pool)
        };
        (
            one("SELECT id FROM kube.node WHERE cluster_id = $1 AND valid_period @> now()")
                .await
                .unwrap(),
            one(
                "SELECT o.id FROM kube.workload o JOIN kube.namespace n ON n.id = o.namespace_id
                  WHERE n.cluster_id = $1 AND o.valid_period @> now()",
            )
            .await
            .unwrap(),
        )
    }

    /// `batch` of `cluster` by `author`, written under the role of the application.
    async fn record_by(
        &self,
        cluster: Uuid,
        author: Uuid,
        batch: &Batch,
    ) -> Result<Recorded, RecordError> {
        let mut tx = as_app(&self.pool).await;
        let recorded = record(&mut tx, cluster, author, batch).await;
        if recorded.is_ok() {
            tx.commit().await.unwrap();
        }
        recorded
    }

    /// `batch` of `prod` by its connector, accepted.
    async fn recorded(&self, batch: &Batch) -> Recorded {
        self.record_by(self.prod, self.connector, batch)
            .await
            .unwrap()
    }

    /// The lines that refuse `batch` of `prod` by its connector.
    async fn invalid(&self, batch: &Batch) -> Vec<Invalid> {
        match self.record_by(self.prod, self.connector, batch).await {
            Err(RecordError::Invalid(lines)) => lines,
            other => panic!("not refused by its lines: {other:?}"),
        }
    }

    /// The rows of the journal and its batches.
    async fn counts(&self) -> (i64, i64) {
        sqlx::query_as(
            "SELECT (SELECT count(*) FROM kube.pod_observation),
                    (SELECT count(*) FROM kube.observation_batch)",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// A pod `pod` of `web` on `worker-1` with the IP `ip`, seen an hour ago.
    fn pod(&self, pod: u128, ip: &str) -> Opening {
        Opening {
            namespace: "payments".into(),
            pod_name: format!("web-{pod}"),
            pod_uid: Uuid::from_u128(pod),
            workload: Some(self.web),
            node: Some(self.node),
            ip: Some(ip.parse().unwrap()),
            host_network: false,
            from: OffsetDateTime::now_utc() - hours(1),
        }
    }
}

/// A transaction under the role of the application.
async fn as_app(pool: &PgPool) -> Transaction<'static, Postgres> {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE kartenn_app")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx
}

/// `kube.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("kube.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// `kube.sync` upserting the object of `kind` named `name` in `cluster`.
fn sync(kind: &str, cluster: Uuid, name: &str, extra: Value) -> Operation {
    let mut fields = json!({
        "action": "upsert",
        "type": format!("kube.{kind}"),
        "cluster": {"id": cluster},
        "name": name,
    });
    if let (Value::Object(into), Value::Object(extra)) = (&mut fields, extra) {
        into.extend(extra);
    }
    op("sync", fields)
}

fn closing(pod: u128, ip: &str, at: OffsetDateTime) -> Closing {
    Closing {
        pod_uid: Uuid::from_u128(pod),
        ip: Some(ip.parse().unwrap()),
        at,
    }
}

fn line(line: Line, reason: Reason) -> Invalid {
    Invalid { line, reason }
}

/// REQ-019-16 (validation): an IP outside the pod containers and ranges of the cluster
/// and the addresses of its nodes, a workload or a node of another cluster, an IP in
/// `hostNetwork`, refuse the batch whole, each line named; nothing is written. An IP in a
/// pod range of a node, an address of a node, `hostNetwork` without IP are accepted.
#[kartenn_testing::pg_test]
async fn batch_outside_pod_ranges_refused_whole(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (other, group) = crate::support::connector(&pool, "other").await;
    let stage = env.cluster("stage", group).await;
    let (stage_node, stage_web) = env.populated(stage, other, "198.51.100.64/26").await;
    let mut host = env.pod(5, "192.0.2.21");
    host.ip = None;
    host.host_network = true;
    let mut with_ip = env.pod(6, "198.51.100.30");
    with_ip.host_network = true;
    let mut foreign_workload = env.pod(7, "198.51.100.31");
    foreign_workload.workload = Some(stage_web);
    let mut foreign_node = env.pod(8, "198.51.100.32");
    foreign_node.node = Some(stage_node);
    let batch = Batch {
        opened: vec![
            env.pod(1, "198.51.100.17"),
            env.pod(2, "192.0.2.10"),
            env.pod(3, "192.0.2.21"),
            host.clone(),
            with_ip,
            foreign_workload,
            foreign_node,
            env.pod(9, "2001:db8::1"),
        ],
        closed: Vec::new(),
    };
    assert_eq!(
        env.invalid(&batch).await,
        [
            line(Line::Opened(1), Reason::OutsideCluster),
            line(Line::Opened(4), Reason::HostNetwork),
            line(Line::Opened(5), Reason::ForeignWorkload),
            line(Line::Opened(6), Reason::ForeignNode),
            line(Line::Opened(7), Reason::OutsideCluster),
        ]
    );
    assert_eq!(
        env.counts().await,
        (0, 0),
        "nothing of the batch is written"
    );
    // The valid lines alone: written.
    let batch = Batch {
        opened: vec![env.pod(1, "198.51.100.17"), env.pod(3, "192.0.2.21"), host],
        closed: Vec::new(),
    };
    let recorded = env.recorded(&batch).await;
    assert_eq!((recorded.opened, recorded.closed), (3, 0));
    assert_eq!(env.counts().await, (3, 1));
    // A pod of `stage` in its own range, by its connector: accepted there only.
    let mut theirs = env.pod(10, "198.51.100.70");
    (theirs.workload, theirs.node) = (Some(stage_web), Some(stage_node));
    let batch = Batch {
        opened: vec![theirs],
        closed: Vec::new(),
    };
    env.record_by(stage, other, &batch).await.unwrap();
}

/// REQ-019-16 (droit propre): a batch of the connector of another cluster, of a person
/// member of the connector group, or of no connector, on a cluster that does not live,
/// is refused and writes nothing.
#[kartenn_testing::pg_test]
async fn batch_from_other_cluster_connector_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (other, group) = crate::support::connector(&pool, "other").await;
    env.cluster("stage", group).await;
    let person: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let prod_group: Uuid =
        sqlx::query_scalar("SELECT connector_group_id FROM kube.cluster WHERE id = $1")
            .bind(env.prod)
            .fetch_one(&pool)
            .await
            .unwrap();
    crate::support::member(&pool, prod_group, person).await;
    let batch = Batch {
        opened: vec![env.pod(1, "198.51.100.17")],
        closed: Vec::new(),
    };
    for author in [other, person, Uuid::now_v7()] {
        let refused = env.record_by(env.prod, author, &batch).await;
        assert!(
            matches!(refused, Err(RecordError::NotConnector)),
            "{refused:?}"
        );
    }
    let unknown = Uuid::now_v7();
    let refused = env.record_by(unknown, env.connector, &batch).await;
    assert!(
        matches!(refused, Err(RecordError::UnknownCluster(id)) if id == unknown),
        "{refused:?}"
    );
    assert_eq!(env.counts().await, (0, 0));
}

/// REQ-019-16 (ajout seul): the role of the application inserts and sets the end of an
/// open period, nothing else; a closed period is neither opened again nor closed twice by
/// a batch, nor rewritten, reopened or deleted in SQL.
#[kartenn_testing::pg_test]
async fn app_role_cannot_rewrite_or_delete(pool: PgPool) {
    let env = Env::new(&pool).await;
    let now = OffsetDateTime::now_utc();
    env.recorded(&Batch {
        opened: vec![env.pod(1, "198.51.100.17"), env.pod(2, "198.51.100.18")],
        closed: vec![closing(1, "198.51.100.17", now)],
    })
    .await;
    // By a batch.
    assert_eq!(
        env.invalid(&Batch {
            opened: vec![env.pod(1, "198.51.100.17")],
            closed: vec![closing(1, "198.51.100.17", now)],
        })
        .await,
        [
            line(Line::Opened(0), Reason::Closed),
            line(Line::Closed(0), Reason::NotOpen),
        ]
    );
    // In SQL, under the role of the application.
    let refused = [
        "UPDATE kube.pod_observation SET ip = '198.51.100.19' WHERE pod_uid = $1",
        "UPDATE kube.pod_observation SET workload_id = NULL WHERE pod_uid = $1",
        "UPDATE kube.pod_observation SET observed = tstzrange(lower(observed), NULL, '[)'),
                closed_batch = NULL WHERE pod_uid = $1",
        "UPDATE kube.pod_observation SET observed = tstzrange(lower(observed), now() + interval '1 hour', '[)')
          WHERE pod_uid = $1",
        "UPDATE kube.pod_observation SET closed_batch = gen_random_uuid() WHERE pod_uid = $1",
        "UPDATE kube.pod_observation SET observed = tstzrange(lower(observed) - interval '1 day', NULL, '[)')
          WHERE pod_uid = $1",
        "DELETE FROM kube.pod_observation WHERE pod_uid = $1",
        "DELETE FROM kube.pod_observation_open WHERE pod_uid = $1",
        "UPDATE kube.observation_batch SET opened = 0 WHERE $1 IS NOT NULL",
        "DELETE FROM kube.observation_batch WHERE $1 IS NOT NULL",
    ];
    for pod in [1, 2] {
        for sql in refused {
            let mut tx = as_app(&pool).await;
            let result = sqlx::query(sqlx::AssertSqlSafe(sql))
                .bind(Uuid::from_u128(pod))
                .execute(&mut *tx)
                .await;
            assert!(result.is_err(), "pod {pod}: {sql}");
        }
    }
    for sql in [
        "TRUNCATE kube.pod_observation",
        "TRUNCATE kube.observation_batch",
        "CREATE TABLE kube.pod_observation_20000101 PARTITION OF kube.pod_observation_closed
             FOR VALUES FROM ('2000-01-01') TO ('2000-01-02')",
    ] {
        let mut tx = as_app(&pool).await;
        let result = sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&mut *tx)
            .await;
        assert!(result.is_err(), "{sql}");
    }
    // An end that only a batch accepted may set: a closing that cites no batch is refused
    // by the database, so no end stands outside a fingerprint.
    let mut tx = as_app(&pool).await;
    let unknown = sqlx::query(
        "UPDATE kube.pod_observation
            SET observed = tstzrange(lower(observed), now(), '[)'), closed_batch = gen_random_uuid()
          WHERE pod_uid = $1",
    )
    .bind(Uuid::from_u128(2))
    .execute(&mut *tx)
    .await
    .expect_err("a closing of no batch");
    assert_eq!(
        unknown
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .as_deref(),
        Some("23503"),
        "{unknown}"
    );
    drop(tx);
    // The owner of the schema is held by the trigger too.
    let reopened = sqlx::query(
        "UPDATE kube.pod_observation SET observed = tstzrange(lower(observed), NULL, '[)'),
                closed_batch = NULL WHERE pod_uid = $1",
    )
    .bind(Uuid::from_u128(1))
    .execute(&pool)
    .await;
    assert!(reopened.is_err());
    let kept: Vec<(bool, Option<String>)> = sqlx::query_as(
        "SELECT upper_inf(observed), host(ip) FROM kube.pod_observation ORDER BY pod_uid",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        kept,
        [
            (false, Some("198.51.100.17".into())),
            (true, Some("198.51.100.18".into()))
        ]
    );
}

/// Two batches of two clusters that close in the same new day: the second waits for the
/// first, which creates the day, then closes in it; neither deadlocks, one day exists.
#[kartenn_testing::pg_test]
async fn concurrent_batches_create_one_day(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (other, group) = crate::support::connector(&pool, "other").await;
    let stage = env.cluster("stage", group).await;
    let (stage_node, stage_web) = env.populated(stage, other, "198.51.100.64/26").await;
    let day = OffsetDateTime::now_utc() - hours(72);
    let mut ours = env.pod(1, "198.51.100.17");
    ours.from = day - hours(24);
    let mut theirs = env.pod(2, "198.51.100.70");
    (theirs.workload, theirs.node, theirs.from) = (Some(stage_web), Some(stage_node), ours.from);
    let opened = |pod| Batch {
        opened: vec![pod],
        closed: Vec::new(),
    };
    env.recorded(&opened(ours)).await;
    env.record_by(stage, other, &opened(theirs)).await.unwrap();
    let closing_of = |pod, ip| Batch {
        opened: Vec::new(),
        closed: vec![closing(pod, ip, day)],
    };
    // The first holds its new day until it commits.
    let mut first = as_app(&pool).await;
    record(
        &mut first,
        env.prod,
        env.connector,
        &closing_of(1, "198.51.100.17"),
    )
    .await
    .unwrap();
    let second = tokio::spawn({
        let pool = pool.clone();
        let batch = closing_of(2, "198.51.100.70");
        async move {
            let mut tx = as_app(&pool).await;
            let recorded = record(&mut tx, stage, other, &batch).await;
            if recorded.is_ok() {
                tx.commit().await.unwrap();
            }
            recorded.map(|r| r.closed)
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(!second.is_finished(), "the second waits for the day");
    first.commit().await.unwrap();
    assert_eq!(second.await.unwrap().unwrap(), 1);
    let days: Vec<String> = sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_inherits i JOIN pg_class c ON c.oid = i.inhrelid
          WHERE i.inhparent = 'kube.pod_observation_closed'::regclass",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(days.len(), 1, "{days:?}");
    let open: i64 =
        sqlx::query_scalar("SELECT count(*) FROM kube.pod_observation WHERE upper_inf(observed)")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(open, 0);
}

/// REQ-019-16 (séparation): a batch writes no changeset, no operation, nothing to the
/// outbox, and no object of the model.
#[kartenn_testing::pg_test]
async fn batch_emits_no_changes_event(pool: PgPool) {
    let env = Env::new(&pool).await;
    let model = "SELECT (SELECT count(*) FROM changeset), (SELECT count(*) FROM changeset_operation),
                        (SELECT count(*) FROM outbox),
                        (SELECT count(*) FROM kube.workload) + (SELECT count(*) FROM kube.node)
                          + (SELECT count(*) FROM kube.namespace) + (SELECT count(*) FROM kube.cluster)
                          + (SELECT count(*) FROM address) + (SELECT count(*) FROM prefix)";
    let before: (i64, i64, i64, i64) = sqlx::query_as(model).fetch_one(&pool).await.unwrap();
    env.recorded(&Batch {
        opened: vec![env.pod(1, "198.51.100.17")],
        closed: vec![closing(1, "198.51.100.17", OffsetDateTime::now_utc())],
    })
    .await;
    let after: (i64, i64, i64, i64) = sqlx::query_as(model).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
    assert_eq!(env.counts().await, (1, 1));
}

/// ADR 0043, decision 5: the fingerprint of a batch, kept in `observation_batch`, is
/// recomputed from the rows it opened and closed. An opening of an observation open with
/// the same values writes nothing; with other values, it refuses the batch; an end before
/// the start too; an instant to come is the instant of the batch.
#[kartenn_testing::pg_test]
async fn batch_hash_recomputes_from_rows(pool: PgPool) {
    let env = Env::new(&pool).await;
    let first = env
        .recorded(&Batch {
            opened: vec![env.pod(1, "198.51.100.17"), env.pod(2, "198.51.100.18")],
            closed: Vec::new(),
        })
        .await;
    assert_eq!(first.opened, 2);
    // The connector lists again after a restart: the open pod 2 again, pod 1 gone, pod 3
    // seen and gone within the batch, its end to come.
    let mut moved = env.pod(2, "198.51.100.18");
    moved.from = OffsetDateTime::now_utc();
    let gone = OffsetDateTime::now_utc() - std::time::Duration::from_secs(300);
    let second = env
        .recorded(&Batch {
            opened: vec![moved.clone(), env.pod(3, "198.51.100.19")],
            closed: vec![
                closing(1, "198.51.100.17", gone),
                closing(3, "198.51.100.19", gone + hours(48)),
            ],
        })
        .await;
    assert_eq!((second.opened, second.closed), (1, 2));
    let ends: Vec<(Uuid, bool)> = sqlx::query_as(
        "SELECT pod_uid, upper(observed) <= $1
           FROM kube.pod_observation WHERE closed_batch = $2 ORDER BY pod_uid",
    )
    .bind(second.received_at)
    .bind(second.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        ends,
        [(Uuid::from_u128(1), true), (Uuid::from_u128(3), true)],
        "the end to come is the instant of the batch"
    );
    for batch in [&first, &second] {
        let stored: Vec<u8> =
            sqlx::query_scalar("SELECT content_hash FROM kube.observation_batch WHERE id = $1")
                .bind(batch.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored, batch.hash);
        assert_eq!(rehash(&pool, env.prod, batch.id).await, batch.hash);
    }
    // Pod 2 open with another node, an end before its start: refused.
    moved.node = None;
    let start = OffsetDateTime::now_utc() - hours(2);
    assert_eq!(
        env.invalid(&Batch {
            opened: vec![moved],
            closed: vec![closing(2, "198.51.100.18", start)],
        })
        .await,
        [
            line(Line::Opened(0), Reason::OpenOtherwise),
            line(Line::Closed(0), Reason::EndNotAfterStart),
        ]
    );
}

/// The fingerprint of the batch `id` of `cluster` from the rows of the journal.
async fn rehash(pool: &PgPool, cluster: Uuid, id: Uuid) -> [u8; 32] {
    type Row = (
        String,
        String,
        Uuid,
        Option<Uuid>,
        Option<Uuid>,
        Option<String>,
        bool,
        OffsetDateTime,
    );
    let opened: Vec<Row> = sqlx::query_as(
        "SELECT namespace, pod_name, pod_uid, workload_id, node_id, host(ip), host_network,
                lower(observed)
           FROM kube.pod_observation WHERE opened_batch = $1",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .unwrap();
    let ip = |ip: Option<String>| ip.map(|ip| ip.parse::<IpAddr>().unwrap());
    let opened: Vec<Opening> = opened
        .into_iter()
        .map(
            |(namespace, pod_name, pod_uid, workload, node, address, host_network, from)| Opening {
                namespace,
                pod_name,
                pod_uid,
                workload,
                node,
                ip: ip(address),
                host_network,
                from,
            },
        )
        .collect();
    let closed: Vec<(Uuid, Option<String>, OffsetDateTime)> = sqlx::query_as(
        "SELECT pod_uid, host(ip), upper(observed) FROM kube.pod_observation
          WHERE closed_batch = $1",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .unwrap();
    let closed: Vec<Closing> = closed
        .into_iter()
        .map(|(pod_uid, address, at)| Closing {
            pod_uid,
            ip: ip(address),
            at,
        })
        .collect();
    content_hash(cluster, &opened, &closed).unwrap()
}
