//! Migration 0072 (spec 026, REQ-026-3; decision C4): a base populated as the module `vlan`
//! of spec 008 left it — groups and VLANs in the schema `vlan` with their versions and
//! histories, recorded in `module_object` and in the registry of the modules, a network
//! carrying `vlan.vlan`, interfaces of the module machines in mode access and trunk, a
//! journal, an anomaly and a custom role under the names of 008 — reads back the same
//! objects at every date once migrated, under the names of the core; a second run changes
//! nothing.
//!
//! The base is written by the code of the core, then put back as 008 left it (the inverse
//! of the migration): the tables go back to the schema `vlan`, the names of 008 back in
//! place, the records of the module added.

use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::operations::{PeriodSpec, RawInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::read;
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

const MIGRATION: &str = include_str!("../../../../kartenn-store-pg/migrations/0072_vlan_core.sql");

/// 2030-01-01T00:00:00Z, in microseconds: the VLAN 10 is renamed from then on.
const LATER: i64 = 1_893_456_000_000_000;

fn op(fields: Value) -> Operation {
    serde_json::from_value(fields).unwrap()
}

async fn apply(store: &PgStore, author: Uuid, operations: Vec<Operation>, start: Option<i64>) {
    let outcome = submit(
        store,
        &Unrestricted,
        &ChangesetRequest {
            author,
            motive: "seed".into(),
            period: start.map(|start| PeriodSpec { start, end: None }),
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        },
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}

async fn scalar<T>(pool: &PgPool, sql: &str) -> T
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
{
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Every row of the tables the migration moves or renames, as JSON, ordered.
async fn dump(pool: &PgPool) -> Value {
    let mut out = serde_json::Map::new();
    for (name, sql) in [
        ("vlan_group", "SELECT to_jsonb(x) FROM vlan_group x"),
        ("vlan", "SELECT to_jsonb(x) FROM vlan x"),
        (
            "vlan_group_history",
            "SELECT to_jsonb(x) FROM vlan_group_history x",
        ),
        ("vlan_history", "SELECT to_jsonb(x) FROM vlan_history x"),
        (
            "declarations",
            "SELECT to_jsonb(x) FROM property_declaration x
              UNION ALL SELECT to_jsonb(x) FROM property_declaration_history x",
        ),
        (
            "obligations",
            "SELECT to_jsonb(x) FROM property_obligation x
              UNION ALL SELECT to_jsonb(x) FROM property_obligation_history x",
        ),
        (
            "assignments",
            "SELECT to_jsonb(x) FROM property_assignment x
              UNION ALL SELECT to_jsonb(x) FROM property_assignment_history x",
        ),
        (
            "operations",
            "SELECT to_jsonb(x) FROM changeset_operation x",
        ),
        (
            "changesets",
            "SELECT jsonb_build_object('id', id, 'violations', violations,
                                       'required_rights', required_rights, 'request', request)
               FROM changeset x",
        ),
        ("anomalies", "SELECT to_jsonb(x) FROM anomaly x"),
        ("interfaces", "SELECT to_jsonb(x) FROM machines.interface x"),
    ] {
        let mut rows: Vec<Value> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
            .fetch_all(pool)
            .await
            .unwrap();
        rows.sort_by_key(Value::to_string);
        out.insert(name.to_owned(), Value::Array(rows));
    }
    Value::Object(out)
}

/// Puts the VLANs back as spec 008 left them: the inverse of migration 0072.
const AS_IN_008: &str = r#"
CREATE SCHEMA vlan;
DO $$
DECLARE
    part oid;
BEGIN
    FOR part IN
        SELECT inhrelid FROM pg_inherits
         WHERE inhparent IN ('public.vlan_group_history'::regclass, 'public.vlan_history'::regclass)
    LOOP
        EXECUTE format('ALTER TABLE %s SET SCHEMA vlan', part::regclass);
    END LOOP;
END $$;
ALTER TABLE public.vlan_group_history SET SCHEMA vlan;
ALTER TABLE public.vlan_history SET SCHEMA vlan;
ALTER TABLE public.vlan_group SET SCHEMA vlan;
ALTER TABLE public.vlan SET SCHEMA vlan;

INSERT INTO module_object (id, module, kind, parent_id, domain_id, valid_period, changeset_id)
SELECT id, 'vlan', 'vlan.group', NULL, CASE WHEN anchor_kind = 'domain' THEN anchor_id END,
       valid_period, changeset_id
  FROM vlan.vlan_group;
INSERT INTO module_object (id, module, kind, parent_id, domain_id, valid_period, changeset_id)
SELECT v.id, 'vlan', 'vlan.vlan', v.group_id,
       (SELECT CASE WHEN g.anchor_kind = 'domain' THEN g.anchor_id END
          FROM vlan.vlan_group g WHERE g.id = v.group_id LIMIT 1),
       v.valid_period, v.changeset_id
  FROM vlan.vlan v;
INSERT INTO module_object_history (id, module, kind, parent_id, domain_id, valid_period,
                                   sys_start, changeset_id, sys_end)
SELECT id, 'vlan', 'vlan.vlan', group_id, NULL, valid_period, sys_start, changeset_id, sys_end
  FROM vlan.vlan_history;
INSERT INTO module_migration (module, version, name, checksum)
VALUES ('vlan', 1, 'vlan', '\x00'), ('vlan', 2, 'group_keyset', '\x00');

ALTER TABLE property_declaration DISABLE TRIGGER versioning;
ALTER TABLE property_obligation DISABLE TRIGGER versioning;
ALTER TABLE property_assignment DISABLE TRIGGER versioning;
UPDATE property_declaration
   SET key = 'vlan.vlan', type_params = jsonb_set(type_params, '{target}', '"vlan.vlan"')
 WHERE key = 'core.vlan';
UPDATE property_declaration_history
   SET key = 'vlan.vlan', type_params = jsonb_set(type_params, '{target}', '"vlan.vlan"')
 WHERE key = 'core.vlan';
UPDATE property_declaration
   SET carriers = array_replace(array_replace(carriers, 'vlan_group', 'vlan.group'), 'vlan', 'vlan.vlan')
 WHERE carriers && ARRAY['vlan_group', 'vlan'];
UPDATE property_declaration_history
   SET carriers = array_replace(array_replace(carriers, 'vlan_group', 'vlan.group'), 'vlan', 'vlan.vlan')
 WHERE carriers && ARRAY['vlan_group', 'vlan'];
UPDATE property_obligation SET scope_type = 'vlan.group' WHERE scope_type = 'vlan_group';
UPDATE property_obligation_history SET scope_type = 'vlan.group' WHERE scope_type = 'vlan_group';
UPDATE property_assignment SET object_type = 'vlan.' || CASE object_type WHEN 'vlan_group' THEN 'group' ELSE 'vlan' END
 WHERE object_type IN ('vlan_group', 'vlan');
UPDATE property_assignment_history SET object_type = 'vlan.' || CASE object_type WHEN 'vlan_group' THEN 'group' ELSE 'vlan' END
 WHERE object_type IN ('vlan_group', 'vlan');
ALTER TABLE property_declaration ENABLE TRIGGER versioning;
ALTER TABLE property_obligation ENABLE TRIGGER versioning;
ALTER TABLE property_assignment ENABLE TRIGGER versioning;

UPDATE changeset_operation
   SET payload = replace(replace(replace(replace(payload::text,
           '"create_vlan_group"', '"vlan.create_group"'),
           '"update_vlan_group"', '"vlan.update_group"'),
           '"create_vlan"', '"vlan.create_vlan"'),
           '"update_vlan"', '"vlan.update_vlan"')::jsonb
 WHERE payload::text LIKE '%vlan%';
"#;

/// The reads of `objects` at the dates `dates`, `(valid_at, as_of)`.
async fn reads(
    pool: &PgPool,
    objects: &[Uuid],
    dates: &[(OffsetDateTime, Option<OffsetDateTime>)],
) -> Vec<String> {
    let mut out = Vec::new();
    for id in objects {
        for (valid_at, as_of) in dates {
            let found = read::object(pool, *id, *valid_at, *as_of).await.unwrap();
            out.push(format!("{id} {valid_at} {as_of:?}: {found:?}"));
        }
    }
    out
}

#[kartenn_testing::pg_test]
async fn a_base_of_008_reads_back_under_the_names_of_the_core(pool: PgPool) {
    // As the deployment of a build with its modules.
    let registry = kartenn_server::modules::registry().unwrap();
    kartenn_store_pg::modules::migrate(&pool, &registry)
        .await
        .unwrap();
    let store = PgStore::new(pool.clone()).with_modules(registry.clone());
    let declarations: Vec<_> = std::iter::once(core_declarations_for(&registry))
        .chain(registry.modules().filter_map(|m| m.declarations()))
        .collect();
    for outcome in sync_modules(&store, &Unrestricted, &declarations, MAX_OPERATIONS)
        .await
        .unwrap()
    {
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }
    let author: Uuid = scalar(
        &pool,
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .await;
    apply(
        &store,
        author,
        vec![Operation::CreateEntity {
            local: Some(1),
            slug: "acme".into(),
            parent: None,
        }],
        None,
    )
    .await;
    let acme: Uuid = scalar(&pool, "SELECT id FROM entity WHERE slug = 'acme'").await;
    let owner: Uuid = scalar(
        &pool,
        "SELECT id FROM property_declaration WHERE key = 'core.owner'",
    )
    .await;
    apply(
        &store,
        author,
        vec![
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
        ],
        None,
    )
    .await;
    let lab: Uuid = scalar(&pool, "SELECT id FROM addressing_domain WHERE slug = 'lab'").await;
    // A group of the domain and its VLANs 10, 20 and 30.
    apply(
        &store,
        author,
        vec![
            op(
                json!({"op": "create_vlan_group", "local": 1, "slug": "edge",
                      "anchor": {"domain": {"id": lab}}}),
            ),
            op(json!({"op": "create_vlan", "group": {"local": 1}, "vid": 10, "name": "users"})),
            op(json!({"op": "create_vlan", "group": {"local": 1}, "vid": 20, "name": "voice"})),
            op(
                json!({"op": "create_vlan", "group": {"local": 1}, "vid": 30, "name": "guests",
                      "status": "reserved"}),
            ),
        ],
        None,
    )
    .await;
    let vlan = |vid: i32| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM vlan WHERE vid = $1")
                .bind(i16::try_from(vid).unwrap())
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let (v10, v20, v30) = (vlan(10).await, vlan(20).await, vlan(30).await);
    let group: Uuid = scalar(&pool, "SELECT id FROM vlan_group WHERE slug = 'edge'").await;
    let asserted: OffsetDateTime = scalar(&pool, "SELECT clock_timestamp()").await;
    // The VLAN 10 renamed from 2030 on (valid time); the VLAN 30 put in use (system time).
    apply(
        &store,
        author,
        vec![op(
            json!({"op": "update_vlan", "id": {"id": v10}, "name": "staff"}),
        )],
        Some(LATER),
    )
    .await;
    apply(
        &store,
        author,
        vec![op(
            json!({"op": "update_vlan", "id": {"id": v30}, "status": "active"}),
        )],
        None,
    )
    .await;
    // A network of the domain carries the VLAN 10.
    let vlan_property: Uuid = scalar(
        &pool,
        "SELECT id FROM property_declaration WHERE key = 'core.vlan'",
    )
    .await;
    apply(
        &store,
        author,
        vec![
            op(
                json!({"op": "create_prefix", "local": 1, "domain": {"id": lab},
                      "prefix": "10.0.0.0/24", "nature": "network"}),
            ),
            Operation::AssignProperty {
                object: Ref::Local(1),
                kind: ObjectKind::Prefix,
                property: Ref::Id(vlan_property),
                values: vec![RawInput::Text(v10.to_string())],
            },
        ],
        None,
    )
    .await;
    let network: Uuid = scalar(&pool, "SELECT id FROM prefix WHERE prefix = '10.0.0.0/24'").await;
    // A machine with an interface in mode access, another in mode trunk.
    apply(
        &store,
        author,
        vec![
            op(
                json!({"op": "machines.create_machine", "local": 1, "name": "srv01",
                      "kind": "physical", "owner": {"id": acme}}),
            ),
            op(
                json!({"op": "machines.create_interface", "machine": {"local": 1},
                      "name": "eth0", "vlan": {"mode": "access", "access": {"id": v10}}}),
            ),
            op(
                json!({"op": "machines.create_interface", "machine": {"local": 1},
                      "name": "eth1", "vlan": {"mode": "trunk", "native": {"id": v20},
                                               "tagged": [{"id": v30}]}}),
            ),
        ],
        None,
    )
    .await;
    // A refusal and an anomaly under the codes of the core, a custom role of 008.
    let any_changeset: Uuid = scalar(&pool, "SELECT id FROM changeset LIMIT 1").await;
    sqlx::query(
        "INSERT INTO anomaly (rule_id, object_id, message, opened_by)
         VALUES ('core.vlan.site_mismatch', $1, 'm', $2)",
    )
    .bind(network)
    .bind(any_changeset)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE changeset
            SET violations = '[{\"rule\": \"core.vlan.vid_duplicate\", \"message\": \"m\"}]',
                required_rights = '{\"required\": [[\"core.vlan.edit\", []]]}'
          WHERE id = $1",
    )
    .bind(any_changeset)
    .execute(&pool)
    .await
    .unwrap();

    // What the core reads, then the base as 008 left it.
    let now: OffsetDateTime = scalar(&pool, "SELECT clock_timestamp()").await;
    let later = OffsetDateTime::from_unix_timestamp_nanos(i128::from(LATER) * 1_000).unwrap();
    let dates = [
        (now, None),
        (later, None),
        (now, Some(asserted)),
        (later, Some(asserted)),
    ];
    let objects = [group, v10, v20, v30, network];
    let before = reads(&pool, &objects, &dates).await;
    let written = dump(&pool).await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(AS_IN_008))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE anomaly SET rule_id = 'vlan.site.mismatch' WHERE rule_id = 'core.vlan.site_mismatch'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE changeset
            SET violations = '[{\"rule\": \"vlan.vid.duplicate\", \"message\": \"m\"}]',
                required_rights = '{\"required\": [[\"vlan.edit\", []]]}'
          WHERE id = $1",
    )
    .bind(any_changeset)
    .execute(&pool)
    .await
    .unwrap();
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, 'vlan_writers', '{\"fr\": \"VLAN\"}', tstzrange(now(), NULL), $2)",
    )
    .bind(role)
    .bind(any_changeset)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
         VALUES ($1, 'vlan.write', tstzrange(now(), NULL), $2),
                ($1, 'vlan.group.read', tstzrange(now(), NULL), $2)",
    )
    .bind(role)
    .bind(any_changeset)
    .execute(&pool)
    .await
    .unwrap();
    let in_008: bool = scalar(&pool, "SELECT to_regclass('vlan.vlan') IS NOT NULL").await;
    assert!(in_008);

    // The migration.
    sqlx::raw_sql(sqlx::AssertSqlSafe(MIGRATION))
        .execute(&pool)
        .await
        .unwrap();
    let migrated = dump(&pool).await;
    assert_eq!(
        migrated, written,
        "the rows read back as the core wrote them"
    );
    assert_eq!(reads(&pool, &objects, &dates).await, before);
    let gone: bool = scalar(&pool, "SELECT to_regnamespace('vlan') IS NULL").await;
    assert!(gone, "the schema of the module is dropped");
    let recorded: i64 = scalar(
        &pool,
        "SELECT (SELECT count(*) FROM module_object WHERE module = 'vlan')
              + (SELECT count(*) FROM module_object_history WHERE module = 'vlan')
              + (SELECT count(*) FROM module_migration WHERE module = 'vlan')",
    )
    .await;
    assert_eq!(recorded, 0, "no record of the module is left");
    // The interfaces still name their VLANs, which the core holds.
    let named: i64 = scalar(
        &pool,
        "SELECT count(DISTINCT (i.name, v.vid)) FROM machines.interface i
           JOIN vlan v ON v.id IN (i.access_vlan_id, i.native_vlan_id) OR v.id = ANY(i.tagged_vlan_ids)",
    )
    .await;
    assert_eq!(named, 3);
    // The custom role takes the names of the core, by the system changeset of the job.
    kartenn_service::rbac::rename_permissions(&store, MAX_OPERATIONS)
        .await
        .unwrap()
        .expect("a changeset renames the role");
    let mut entries: Vec<String> = sqlx::query_scalar(
        "SELECT permission FROM role_permission WHERE role_id = $1 AND valid_period @> now()",
    )
    .bind(role)
    .fetch_all(&pool)
    .await
    .unwrap();
    entries.sort();
    assert_eq!(
        entries,
        [
            "core.vlan.approval.self",
            "core.vlan.edit",
            "core.vlan_group.read"
        ]
    );
    // The core writes in the migrated base.
    apply(
        &store,
        author,
        vec![op(
            json!({"op": "create_vlan", "group": {"id": group}, "vid": 40, "name": "admin"}),
        )],
        None,
    )
    .await;

    // A second run changes nothing.
    let once = dump(&pool).await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(MIGRATION))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(dump(&pool).await, once);
}
