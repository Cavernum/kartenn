//! The storage of the module in PostgreSQL (feature `pg`): the tables of the schema
//! `machines` (migration `0001_machines.sql`), on the connection of the changeset.

use kartenn_core::period::{Instant, Period};
use kartenn_core::rules::Violation;
use kartenn_service::port::{StoreError, WriteError};
use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::hosting::{Host, MachineKind, check_host, check_membership};
use crate::rules::{CLUSTER_DUPLICATE, HOST_INVALID};

/// The table of the clusters, as the generic writes of the core name it.
pub(crate) const CLUSTER_TABLE: &str = "machines.cluster";
/// The table of the machines.
pub(crate) const MACHINE_TABLE: &str = "machines.machine";

/// An instant as PostgreSQL reads it.
fn time(i: Instant) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).expect("an instant")
}

/// The bounds of `period` to bind; a `None` start is the instant of the transaction.
pub(crate) fn bounds(period: Option<Period>) -> (Option<OffsetDateTime>, Option<OffsetDateTime>) {
    period.map_or((None, None), |p| (Some(time(p.start())), p.end().map(time)))
}

pub(crate) fn store(e: &sqlx::Error) -> StoreError {
    StoreError(e.to_string())
}

/// The identifier of the property `key` declared at `at`.
pub(crate) async fn property(
    conn: &mut PgConnection,
    key: &str,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT id FROM public.property_declaration
          WHERE key = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(key)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// A refusal of the module before writing: a failed statement would abort the
/// transaction of the changeset, whose other violations must still be found.
fn refused(rule: &'static kartenn_core::rules::Rule, message: String, id: Uuid) -> WriteError {
    WriteError::Rule(Violation::new(rule, message).with_subjects([id]))
}

/// Whether `slug` is taken by another cluster than `id` over `period`.
async fn cluster_slug_taken(
    conn: &mut PgConnection,
    id: Uuid,
    slug: &str,
    period: Option<Period>,
) -> Result<bool, StoreError> {
    let (from, to) = bounds(period);
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM machines.cluster
                         WHERE slug = $1 AND id <> $2
                           AND valid_period && tstzrange(coalesce($3::timestamptz, now()), $4))",
    )
    .bind(slug)
    .bind(id)
    .bind(from)
    .bind(to)
    .fetch_one(conn)
    .await
    .map_err(|e| store(&e))
}

/// A new cluster over `period`.
pub(crate) async fn insert_cluster(
    conn: &mut PgConnection,
    id: Uuid,
    slug: &str,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    if cluster_slug_taken(conn, id, slug, period).await? {
        return Err(refused(
            &CLUSTER_DUPLICATE,
            format!("the slug {slug} of a cluster is taken over the period"),
            id,
        ));
    }
    let (from, to) = bounds(period);
    sqlx::query(
        "INSERT INTO machines.cluster (id, slug, valid_period, changeset_id)
         VALUES ($1, $2, tstzrange(coalesce($3::timestamptz, now()), $4), $5)",
    )
    .bind(id)
    .bind(slug)
    .bind(from)
    .bind(to)
    .bind(changeset)
    .execute(conn)
    .await
    .map(drop)
    .map_err(|e| WriteError::Store(store(&e)))
}

/// Applies `patch` (columns and their values) to the row `id` of `table` over `period`
/// ("FOR PORTION OF", migration 0018 of the core).
async fn portion(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    patch: &Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    let versions: i32 = sqlx::query_scalar(
        "SELECT public.kartenn_portion($1::regclass, $2, coalesce($3::timestamptz, now()), $4,
                                       $5::jsonb, $6)",
    )
    .bind(table)
    .bind(id)
    .bind(from)
    .bind(to)
    .bind(patch)
    .bind(changeset)
    .fetch_one(conn)
    .await
    .map_err(|e| WriteError::Store(store(&e)))?;
    if versions == 0 {
        return Err(WriteError::NoVersion(id));
    }
    Ok(())
}

/// Applies `patch` to the cluster `id` over `period`.
pub(crate) async fn patch_cluster(
    conn: &mut PgConnection,
    id: Uuid,
    patch: &Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    if let Some(slug) = patch.get("slug").and_then(Value::as_str)
        && cluster_slug_taken(conn, id, slug, period).await?
    {
        return Err(refused(
            &CLUSTER_DUPLICATE,
            format!("the slug {slug} of a cluster is taken over the period"),
            id,
        ));
    }
    portion(conn, CLUSTER_TABLE, id, patch, period, changeset).await
}

/// The columns of a machine that its constraints read together.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub(crate) struct Hosting {
    pub kind: String,
    pub host_machine_id: Option<Uuid>,
    pub host_cluster_id: Option<Uuid>,
    pub member_cluster_id: Option<Uuid>,
}

impl Hosting {
    /// `self` with the columns of `patch` that it names.
    fn patched(&self, patch: &Value) -> Self {
        let uuid = |name: &str, old: Option<Uuid>| match patch.get(name) {
            None => old,
            Some(v) => v.as_str().and_then(|s| s.parse().ok()),
        };
        Self {
            kind: patch
                .get("kind")
                .and_then(Value::as_str)
                .map_or_else(|| self.kind.clone(), str::to_owned),
            host_machine_id: uuid("host_machine_id", self.host_machine_id),
            host_cluster_id: uuid("host_cluster_id", self.host_cluster_id),
            member_cluster_id: uuid("member_cluster_id", self.member_cluster_id),
        }
    }

    /// What the constraints of the table check, as a violation of the module (M5): a host
    /// for a kind that runs on none, a membership for a kind that joins no cluster, a
    /// machine that hosts itself. The kind of a host machine is checked on the final state.
    pub(crate) fn admissible(&self, id: Uuid) -> Result<(), Violation> {
        let kind = MachineKind::from_name(&self.kind)
            .ok_or_else(|| Violation::new(&HOST_INVALID, format!("unknown kind {}", self.kind)))?;
        if self.host_machine_id == Some(id) {
            return Err(
                Violation::new(&HOST_INVALID, "a machine does not host itself").with_subjects([id]),
            );
        }
        if self.host_cluster_id.is_some() {
            check_host(kind, Host::Cluster).map_err(|v| v.with_subjects([id]))?;
        }
        if self.host_machine_id.is_some()
            && !matches!(kind, MachineKind::Virtual | MachineKind::Container)
        {
            return Err(
                Violation::new(&HOST_INVALID, format!("a machine {kind} is not hosted"))
                    .with_subjects([id]),
            );
        }
        if self.member_cluster_id.is_some() {
            check_membership(kind).map_err(|v| v.with_subjects([id]))?;
        }
        Ok(())
    }
}

/// A new machine over `period`.
pub(crate) async fn insert_machine(
    conn: &mut PgConnection,
    id: Uuid,
    name: &str,
    hosting: &Hosting,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    hosting.admissible(id).map_err(WriteError::Rule)?;
    let (from, to) = bounds(period);
    sqlx::query(
        "INSERT INTO machines.machine (id, name, kind, host_machine_id, host_cluster_id,
                                       member_cluster_id, valid_period, changeset_id)
         VALUES ($1, $2, $3, $4, $5, $6, tstzrange(coalesce($7::timestamptz, now()), $8), $9)",
    )
    .bind(id)
    .bind(name)
    .bind(&hosting.kind)
    .bind(hosting.host_machine_id)
    .bind(hosting.host_cluster_id)
    .bind(hosting.member_cluster_id)
    .bind(from)
    .bind(to)
    .bind(changeset)
    .execute(conn)
    .await
    .map(drop)
    .map_err(|e| WriteError::Store(store(&e)))
}

/// Applies `patch` to the machine `id` over `period`, after checking every version it
/// changes against the constraints of the table.
pub(crate) async fn patch_machine(
    conn: &mut PgConnection,
    id: Uuid,
    patch: &Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    let versions: Vec<Hosting> = sqlx::query_as(
        "SELECT kind, host_machine_id, host_cluster_id, member_cluster_id
           FROM machines.machine
          WHERE id = $1 AND valid_period && tstzrange(coalesce($2::timestamptz, now()), $3)",
    )
    .bind(id)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| WriteError::Store(store(&e)))?;
    for version in &versions {
        version
            .patched(patch)
            .admissible(id)
            .map_err(WriteError::Rule)?;
    }
    portion(conn, MACHINE_TABLE, id, patch, period, changeset).await
}

/// The machines whose `core.owner` the changeset changes (the detail of its differential,
/// recorded in the transaction, REQ-003-19): their names are checked again under their
/// new owner (M3).
pub(crate) async fn owner_changed(conn: &mut PgConnection) -> Result<Vec<Uuid>, StoreError> {
    // The detail exists once the differential was computed; reading a missing table would
    // abort the transaction.
    let recorded: bool =
        sqlx::query_scalar("SELECT to_regclass('pg_temp.diff_detail') IS NOT NULL")
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
    if !recorded {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT DISTINCT d.object_id FROM pg_temp.diff_detail d
          WHERE d.property_key = 'core.owner'
            AND EXISTS (SELECT 1 FROM machines.machine m WHERE m.id = d.object_id)",
    )
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The pairs of living machines, one among `ids`, of the same name and the same resolved
/// owner where their periods overlap (M3), as `(machine, other, name)`. The owners are
/// compared now when the overlap holds now, else at the start of the overlap.
pub(crate) async fn name_duplicates(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid, String)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT a.id, b.id, a.name
           FROM machines.machine a
           JOIN machines.machine b
             ON b.name = a.name AND b.id <> a.id AND b.valid_period && a.valid_period
           CROSS JOIN LATERAL (
               SELECT CASE WHEN a.valid_period * b.valid_period @> now() THEN now()
                           ELSE lower(a.valid_period * b.valid_period) END AS at) t
          WHERE a.id = ANY($1)
            AND public.kartenn_owner(a.id, t.at) = public.kartenn_owner(b.id, t.at)
          ORDER BY a.id, b.id",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The machines among `ids`, and the machines they host, with the kind of their host
/// machine where both live: `(guest, guest kind, host, host kind)`.
pub(crate) async fn host_kinds(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String, Uuid, String)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT g.id, g.kind, h.id, h.kind
           FROM machines.machine g
           JOIN machines.machine h ON h.id = g.host_machine_id AND h.valid_period && g.valid_period
          WHERE g.id = ANY($1) OR g.host_machine_id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The machines among `ids`, or hosted by one of them, whose host or cluster does not live
/// over one of their versions: `(machine, what it misses)`.
pub(crate) async fn dead_hosts(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT g.id, t.target
           FROM machines.machine g
           CROSS JOIN LATERAL (VALUES (g.host_machine_id, 'machine'),
                                      (g.host_cluster_id, 'cluster'),
                                      (g.member_cluster_id, 'cluster')) AS t(target, kind)
          WHERE (g.id = ANY($1) OR g.host_machine_id = ANY($1) OR g.host_cluster_id = ANY($1)
                 OR g.member_cluster_id = ANY($1))
            AND t.target IS NOT NULL
            AND NOT EXISTS (
                SELECT 1 FROM (
                    SELECT id, valid_period FROM machines.machine WHERE t.kind = 'machine'
                    UNION ALL
                    SELECT id, valid_period FROM machines.cluster WHERE t.kind = 'cluster'
                ) a
                WHERE a.id = t.target
                GROUP BY a.id
                HAVING range_agg(a.valid_period) @> g.valid_period)",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The machines among `ids` that their chain of host machines leads back to, now.
pub(crate) async fn hosting_cycles(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    sqlx::query_scalar(
        "WITH RECURSIVE up (start, cur, depth) AS (
             SELECT id, host_machine_id, 1 FROM machines.machine
              WHERE id = ANY($1) AND host_machine_id IS NOT NULL AND valid_period @> now()
             UNION ALL
             SELECT up.start, m.host_machine_id, up.depth + 1
               FROM up JOIN machines.machine m ON m.id = up.cur AND m.valid_period @> now()
              WHERE m.host_machine_id IS NOT NULL AND up.cur <> up.start AND up.depth < 8)
         SELECT DISTINCT start FROM up WHERE cur = start",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The resolved owner of `object` at `at` (ADR 0014).
pub(crate) async fn owner(
    conn: &mut PgConnection,
    object: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar("SELECT public.kartenn_owner($1, coalesce($2::timestamptz, now()))")
        .bind(object)
        .bind(at)
        .fetch_one(conn)
        .await
        .map_err(|e| store(&e))
}
