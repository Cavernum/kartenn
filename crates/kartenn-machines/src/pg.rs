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
use crate::rules::{
    ADDRESS_CLOSED, ADDRESS_PRIMARY, ADDRESS_TAKEN, CLUSTER_DUPLICATE, HOST_INVALID,
    INTERFACE_DUPLICATE,
};

/// The table of the clusters, as the generic writes of the core name it.
pub(crate) const CLUSTER_TABLE: &str = "machines.cluster";
/// The table of the machines.
pub(crate) const MACHINE_TABLE: &str = "machines.machine";
/// The table of the interfaces.
pub(crate) const INTERFACE_TABLE: &str = "machines.interface";
/// The table of the attachments of addresses (migration `0003_attachment.sql`).
pub(crate) const ATTACHMENT_TABLE: &str = "machines.attachment";

/// An instant as PostgreSQL reads it.
pub(crate) fn time(i: Instant) -> OffsetDateTime {
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

/// Whether another interface than `id` of `machine` is named `name` over `period`.
async fn interface_name_taken(
    conn: &mut PgConnection,
    id: Uuid,
    machine: Uuid,
    name: &str,
    period: Option<Period>,
) -> Result<bool, StoreError> {
    let (from, to) = bounds(period);
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM machines.interface
                         WHERE machine_id = $1 AND name = $2 AND id <> $3
                           AND valid_period && tstzrange(coalesce($4::timestamptz, now()), $5))",
    )
    .bind(machine)
    .bind(name)
    .bind(id)
    .bind(from)
    .bind(to)
    .fetch_one(conn)
    .await
    .map_err(|e| store(&e))
}

fn interface_taken(name: &str, id: Uuid) -> WriteError {
    refused(
        &INTERFACE_DUPLICATE,
        format!("the machine already has an interface named {name} over the period"),
        id,
    )
}

/// A new interface over `period`, from its planned columns: `name`, `mac` (the canonical
/// text of an EUI-48), `enabled` and the 802.1Q columns.
pub(crate) async fn insert_interface(
    conn: &mut PgConnection,
    id: Uuid,
    machine: Uuid,
    create: &Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let name = create["name"].as_str().unwrap_or_default();
    if interface_name_taken(conn, id, machine, name, period).await? {
        return Err(interface_taken(name, id));
    }
    let uuid = |field: &str| create[field].as_str().and_then(|s| s.parse::<Uuid>().ok());
    let tagged: Vec<Uuid> = create["tagged_vlan_ids"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str()?.parse().ok())
        .collect();
    let (from, to) = bounds(period);
    sqlx::query(
        "INSERT INTO machines.interface (id, machine_id, name, mac, enabled, vlan_mode,
                                         access_vlan_id, native_vlan_id, tagged_vlan_ids,
                                         valid_period, changeset_id)
         VALUES ($1, $2, $3, $4::macaddr, $5, $6, $7, $8, $9,
                 tstzrange(coalesce($10::timestamptz, now()), $11), $12)",
    )
    .bind(id)
    .bind(machine)
    .bind(name)
    .bind(create["mac"].as_str())
    .bind(create["enabled"].as_bool().unwrap_or(true))
    .bind(create["vlan_mode"].as_str().unwrap_or("none"))
    .bind(uuid("access_vlan_id"))
    .bind(uuid("native_vlan_id"))
    .bind(tagged)
    .bind(from)
    .bind(to)
    .bind(changeset)
    .execute(conn)
    .await
    .map(drop)
    .map_err(|e| WriteError::Store(store(&e)))
}

/// The machine of the interface `id` (fixed for its life).
pub(crate) async fn machine_of(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar("SELECT machine_id FROM machines.interface WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(|e| store(&e))
}

/// Applies `patch` to the interface `id` of `machine` over `period`.
pub(crate) async fn patch_interface(
    conn: &mut PgConnection,
    id: Uuid,
    machine: Uuid,
    patch: &Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    if let Some(name) = patch.get("name").and_then(Value::as_str)
        && interface_name_taken(conn, id, machine, name, period).await?
    {
        return Err(interface_taken(name, id));
    }
    portion(conn, INTERFACE_TABLE, id, patch, period, changeset).await
}

/// A refusal of the module before writing, naming `subjects`.
fn refused_on(
    rule: &'static kartenn_core::rules::Rule,
    message: String,
    subjects: [Uuid; 2],
) -> WriteError {
    WriteError::Rule(Violation::new(rule, message).with_subjects(subjects))
}

/// Attaches `address` to `interface` over `period` (REQ-009-9 to 11), after refusing what
/// the constraints of `machines.attachment` would refuse: an address that does not live
/// then, already on the interface, held by another interface unless both rows are shared,
/// or a second primary address of its family on the interface.
pub(crate) async fn attach(
    conn: &mut PgConnection,
    interface: Uuid,
    address: Uuid,
    primary: bool,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    let found: Option<(i16, bool)> = sqlx::query_as(
        "SELECT family(address)::smallint, bool_and(usage IN ('vip', 'anycast'))
           FROM public.address
          WHERE id = $1 AND valid_period && tstzrange(coalesce($2::timestamptz, now()), $3)
          GROUP BY family(address)",
    )
    .bind(address)
    .bind(from)
    .bind(to)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| WriteError::Store(store(&e)))?;
    let Some((family, shared)) = found else {
        return Err(refused_on(
            &ADDRESS_CLOSED,
            format!("the address {address} does not live over the period"),
            [address, interface],
        ));
    };
    let holder: Option<Uuid> = sqlx::query_scalar(
        "SELECT interface_id FROM machines.attachment
          WHERE address_id = $1 AND valid_period && tstzrange(coalesce($2::timestamptz, now()), $3)
            AND (interface_id = $4 OR NOT shared OR NOT $5)
          ORDER BY interface_id = $4 DESC
          LIMIT 1",
    )
    .bind(address)
    .bind(from)
    .bind(to)
    .bind(interface)
    .bind(shared)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| WriteError::Store(store(&e)))?;
    if let Some(holder) = holder {
        let message = if holder == interface {
            "the address is already attached to this interface over the period".to_owned()
        } else {
            format!("the address is attached to the interface {holder}")
        };
        return Err(refused_on(&ADDRESS_TAKEN, message, [address, holder]));
    }
    if primary {
        let other: Option<Uuid> = sqlx::query_scalar(
            "SELECT address_id FROM machines.attachment
              WHERE interface_id = $1 AND family = $2 AND primary_for_family
                AND valid_period && tstzrange(coalesce($3::timestamptz, now()), $4)
              LIMIT 1",
        )
        .bind(interface)
        .bind(family)
        .bind(from)
        .bind(to)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| WriteError::Store(store(&e)))?;
        if let Some(other) = other {
            return Err(refused_on(
                &ADDRESS_PRIMARY,
                format!("the interface already has the primary IPv{family} address {other}"),
                [interface, other],
            ));
        }
    }
    sqlx::query(
        "INSERT INTO machines.attachment (id, interface_id, address_id, primary_for_family,
                                          family, shared, valid_period, changeset_id)
         VALUES ($1, $2, $3, $4, $5, $6, tstzrange(coalesce($7::timestamptz, now()), $8), $9)",
    )
    .bind(Uuid::now_v7())
    .bind(interface)
    .bind(address)
    .bind(primary)
    .bind(family)
    .bind(shared)
    .bind(from)
    .bind(to)
    .bind(changeset)
    .execute(conn)
    .await
    .map(drop)
    .map_err(|e| WriteError::Store(store(&e)))
}

/// The attachments of `address` to `interface` over `period`.
pub(crate) async fn attachments(
    conn: &mut PgConnection,
    interface: Uuid,
    address: Uuid,
    period: Option<Period>,
) -> Result<Vec<Uuid>, StoreError> {
    let (from, to) = bounds(period);
    sqlx::query_scalar(
        "SELECT DISTINCT id FROM machines.attachment
          WHERE interface_id = $1 AND address_id = $2
            AND valid_period && tstzrange(coalesce($3::timestamptz, now()), $4)",
    )
    .bind(interface)
    .bind(address)
    .bind(from)
    .bind(to)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The attachments of `interface` the changeset `changeset` wrote, current or replaced
/// since (undo).
pub(crate) async fn attachments_written(
    conn: &mut PgConnection,
    interface: Uuid,
    changeset: Uuid,
) -> Result<Vec<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT id FROM machines.attachment WHERE interface_id = $1 AND changeset_id = $2
         UNION
         SELECT id FROM machines.attachment_history WHERE interface_id = $1 AND changeset_id = $2",
    )
    .bind(interface)
    .bind(changeset)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The attachments of the interfaces `interfaces` or of the addresses `addresses` whose
/// address does not live over their period (REQ-009-9): `(interface, address)`.
pub(crate) async fn dead_addresses(
    conn: &mut PgConnection,
    interfaces: &[Uuid],
    addresses: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT a.interface_id, a.address_id
           FROM machines.attachment a
          WHERE (a.interface_id = ANY($1) OR a.address_id = ANY($2))
            AND NOT EXISTS (
                SELECT 1 FROM public.address x
                 WHERE x.id = a.address_id
                 GROUP BY x.id
                HAVING range_agg(x.valid_period) @> a.valid_period)",
    )
    .bind(interfaces)
    .bind(addresses)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The attachments of the interfaces `interfaces` outside the life of their interface:
/// `(interface, address)`.
pub(crate) async fn attachments_outside(
    conn: &mut PgConnection,
    interfaces: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT a.interface_id, a.address_id
           FROM machines.attachment a
          WHERE a.interface_id = ANY($1)
            AND NOT EXISTS (
                SELECT 1 FROM machines.interface i
                 WHERE i.id = a.interface_id
                 GROUP BY i.id
                HAVING range_agg(i.valid_period) @> a.valid_period)",
    )
    .bind(interfaces)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// Copies the usage of the addresses `addresses` into their attachments (`shared`,
/// REQ-009-10), portion by portion of each version of the address, as `changeset`: a
/// portion no longer shared while another interface holds the address then is refused,
/// `(address, interface, other)`, once per address; the others are rewritten.
pub(crate) async fn reshare(
    conn: &mut PgConnection,
    addresses: &[Uuid],
    changeset: Uuid,
) -> Result<Vec<(Uuid, Uuid, Uuid)>, StoreError> {
    type Portion = (
        Uuid,
        OffsetDateTime,
        Option<OffsetDateTime>,
        Uuid,
        Uuid,
        bool,
    );
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let portions: Vec<Portion> = sqlx::query_as(
        "SELECT a.id, lower(q.p), upper(q.p), a.address_id, a.interface_id, q.shared
           FROM machines.attachment a
           JOIN public.address x ON x.id = a.address_id AND x.valid_period && a.valid_period
          CROSS JOIN LATERAL (SELECT a.valid_period * x.valid_period AS p,
                                     x.usage IN ('vip', 'anycast') AS shared) q
          WHERE a.address_id = ANY($1) AND q.shared <> a.shared",
    )
    .bind(addresses)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let mut refused: Vec<(Uuid, Uuid, Uuid)> = Vec::new();
    for (id, from, to, address, interface, shared) in portions {
        if refused.iter().any(|(a, ..)| *a == address) {
            continue;
        }
        if !shared {
            let other: Option<Uuid> = sqlx::query_scalar(
                "SELECT interface_id FROM machines.attachment
                  WHERE address_id = $1 AND id <> $2
                    AND valid_period && tstzrange($3::timestamptz, $4::timestamptz)
                  LIMIT 1",
            )
            .bind(address)
            .bind(id)
            .bind(from)
            .bind(to)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
            if let Some(other) = other {
                refused.push((address, interface, other));
                continue;
            }
        }
        sqlx::query("SELECT public.kartenn_portion($1::regclass, $2, $3, $4, $5::jsonb, $6)")
            .bind(ATTACHMENT_TABLE)
            .bind(id)
            .bind(from)
            .bind(to)
            .bind(serde_json::json!({ "shared": shared }))
            .bind(changeset)
            .execute(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
    }
    Ok(refused)
}

/// The VLANs the interfaces among `ids` carry where they do not live over the version
/// that carries them (REQ-009-12): `(interface, vlan)`.
pub(crate) async fn dead_vlans(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT i.id, v.vlan_id
           FROM machines.interface i
          CROSS JOIN LATERAL unnest(array_remove(ARRAY[i.access_vlan_id, i.native_vlan_id], NULL)
                                    || i.tagged_vlan_ids) AS v(vlan_id)
          WHERE i.id = ANY($1)
            AND NOT EXISTS (
                SELECT 1 FROM vlan.vlan x
                 WHERE x.id = v.vlan_id
                 GROUP BY x.id
                HAVING range_agg(x.valid_period) @> i.valid_period)",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The interfaces among `ids` whose machine does not live over one of their versions:
/// `(interface, machine)`.
pub(crate) async fn orphan_interfaces(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT i.id, i.machine_id
           FROM machines.interface i
          WHERE i.id = ANY($1)
            AND NOT EXISTS (
                SELECT 1 FROM machines.machine m
                 WHERE m.id = i.machine_id
                 GROUP BY m.id
                HAVING range_agg(m.valid_period) @> i.valid_period)",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The objects whose resolved value of one of `keys` the changeset changes, read from the
/// detail of the differential of the properties recorded in the transaction (REQ-003-19):
/// a warning may have changed there, and only there.
pub(crate) async fn diff_objects(
    conn: &mut PgConnection,
    keys: &[&str],
) -> Result<Vec<Uuid>, StoreError> {
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
        "SELECT DISTINCT object_id FROM pg_temp.diff_detail WHERE property_key = ANY($1)",
    )
    .bind(keys)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The interfaces with an open anomaly of `rule` that share a MAC with one of `ids`, before
/// or after the changeset (the versions it replaced end now in the history): evaluated
/// again, so that the partner of an interface whose MAC moved away closes its anomaly.
pub(crate) async fn mac_partners(
    conn: &mut PgConnection,
    ids: &[Uuid],
    rule: &str,
) -> Result<Vec<Uuid>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT DISTINCT o.object_id
           FROM public.anomaly o
           JOIN machines.interface p ON p.id = o.object_id
          WHERE o.rule_id = $2 AND o.closed_at IS NULL
            AND p.mac IN (SELECT mac FROM machines.interface
                           WHERE id = ANY($1) AND mac IS NOT NULL
                          UNION
                          SELECT mac FROM machines.interface_history
                           WHERE id = ANY($1) AND mac IS NOT NULL AND sys_end = now())",
    )
    .bind(ids)
    .bind(rule)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The interfaces among `ids` whose universal MAC (the local bit of the first octet at 0,
/// RFC 9542 §2.1) another interface carries where their periods overlap, now or later:
/// `(interface, other, mac)`.
pub(crate) async fn mac_duplicates(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid, String)>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as(
        "SELECT DISTINCT a.id, b.id, a.mac::text
           FROM machines.interface a
           JOIN machines.interface b
             ON b.mac = a.mac AND b.id <> a.id AND b.valid_period && a.valid_period
          WHERE a.id = ANY($1)
            AND (a.mac & '02:00:00:00:00:00'::macaddr) = '00:00:00:00:00:00'::macaddr
            AND (upper_inf(a.valid_period * b.valid_period)
                 OR upper(a.valid_period * b.valid_period) > now())
          ORDER BY 1, 2",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The interfaces that hold one of `addresses`, now or later.
pub(crate) async fn interfaces_of_addresses(
    conn: &mut PgConnection,
    addresses: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT DISTINCT interface_id FROM machines.attachment
          WHERE address_id = ANY($1)
            AND (upper_inf(valid_period) OR upper(valid_period) > now())",
    )
    .bind(addresses)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The interfaces that hold, now or later, an address of one of the networks `prefixes`.
pub(crate) async fn interfaces_in_networks(
    conn: &mut PgConnection,
    prefixes: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    if prefixes.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT DISTINCT a.interface_id
           FROM public.prefix p
           JOIN public.address x
             ON x.domain_id = p.domain_id AND x.address <<= p.prefix
            AND x.valid_period && p.valid_period
           JOIN machines.attachment a
             ON a.address_id = x.id AND a.valid_period && x.valid_period
          WHERE p.id = ANY($1) AND p.nature = 'network'
            AND (upper_inf(a.valid_period) OR upper(a.valid_period) > now())",
    )
    .bind(prefixes)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The attachments of the interfaces among `ids`, in mode `access` or `trunk`, whose
/// address lies in a network carrying a VLAN (`vlan.vlan`) the interface does not carry:
/// `(interface, address, vlan)`. Each attachment is read at its start, or now when it
/// started before.
pub(crate) async fn vlan_mismatches(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid, Uuid)>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as(
        "WITH att AS (
             SELECT interface_id, address_id, greatest(lower(valid_period), now()) AS t
               FROM machines.attachment
              WHERE interface_id = ANY($1)
                AND (upper_inf(valid_period) OR upper(valid_period) > now()))
         SELECT DISTINCT att.interface_id, att.address_id, pa.v_uuid
           FROM att
           JOIN machines.interface i
             ON i.id = att.interface_id AND i.valid_period @> att.t AND i.vlan_mode <> 'none'
           JOIN public.address x ON x.id = att.address_id AND x.valid_period @> att.t
           JOIN public.prefix p
             ON p.domain_id = x.domain_id AND p.nature = 'network' AND x.address <<= p.prefix
            AND p.valid_period @> att.t
           JOIN public.property_assignment pa
             ON pa.object_id = p.id AND pa.kind = 'value' AND pa.valid_period @> att.t
           JOIN public.property_declaration d ON d.id = pa.property_id AND d.key = 'vlan.vlan'
          WHERE pa.v_uuid IS DISTINCT FROM i.access_vlan_id
            AND pa.v_uuid IS DISTINCT FROM i.native_vlan_id
            AND NOT (pa.v_uuid = ANY(i.tagged_vlan_ids))
          ORDER BY 1, 2",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The machines hosted by one of `hosts`, now or later.
pub(crate) async fn guests(
    conn: &mut PgConnection,
    hosts: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    if hosts.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT DISTINCT id FROM machines.machine
          WHERE host_machine_id = ANY($1)
            AND (upper_inf(valid_period) OR upper(valid_period) > now())",
    )
    .bind(hosts)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The machines among `ids` hosted by a machine of another resolved site, both sites known,
/// read at the start of their common life or now when it started before: `(guest, host)`.
pub(crate) async fn site_mismatches(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as(
        "SELECT DISTINCT g.id, h.id
           FROM machines.machine g
           JOIN machines.machine h
             ON h.id = g.host_machine_id AND h.valid_period && g.valid_period
          CROSS JOIN LATERAL (
              SELECT greatest(lower(g.valid_period * h.valid_period), now()) AS t,
                     g.valid_period * h.valid_period AS common) c
          CROSS JOIN LATERAL (
              SELECT public.kartenn_site(g.id, c.t) AS guest,
                     public.kartenn_site(h.id, c.t) AS host) s
          WHERE g.id = ANY($1) AND c.common @> c.t
            AND s.guest IS NOT NULL AND s.host IS NOT NULL AND s.guest <> s.host
          ORDER BY 1, 2",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The living objects that cite `target` by the reference `(from, to)` of the module, from
/// `at` on (REQ-009-14). The attachments of an interface (`to` an interface) are given by
/// their addresses.
pub(crate) async fn referrers(
    conn: &mut PgConnection,
    from: &str,
    to: &str,
    target: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Vec<Uuid>, StoreError> {
    use crate::module::{ADDRESS, CLUSTER, INTERFACE, MACHINE, VLAN};
    let sql = match (from, to) {
        (INTERFACE, MACHINE) => {
            "SELECT DISTINCT id FROM machines.interface WHERE machine_id = $1 AND {live} ORDER BY id"
        }
        (INTERFACE, INTERFACE) => {
            "SELECT DISTINCT address_id FROM machines.attachment
              WHERE interface_id = $1 AND {live} ORDER BY address_id"
        }
        (INTERFACE, ADDRESS) => {
            "SELECT DISTINCT interface_id FROM machines.attachment
              WHERE address_id = $1 AND {live} ORDER BY interface_id"
        }
        (INTERFACE, VLAN) => {
            "SELECT DISTINCT id FROM machines.interface
              WHERE (access_vlan_id = $1 OR native_vlan_id = $1 OR $1 = ANY(tagged_vlan_ids))
                AND {live} ORDER BY id"
        }
        (MACHINE, MACHINE) => {
            "SELECT DISTINCT id FROM machines.machine
              WHERE host_machine_id = $1 AND {live} ORDER BY id"
        }
        (MACHINE, CLUSTER) => {
            "SELECT DISTINCT id FROM machines.machine
              WHERE (host_cluster_id = $1 OR member_cluster_id = $1) AND {live} ORDER BY id"
        }
        _ => return Ok(Vec::new()),
    };
    let sql = sql.replace(
        "{live}",
        "valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)",
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(target)
        .bind(at)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))
}

/// Applies to `table` over each of the given portions its patch: `(from, to, patch)`.
async fn portions(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    found: Vec<(OffsetDateTime, Option<OffsetDateTime>, Value)>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    for (from, to, patch) in found {
        sqlx::query("SELECT public.kartenn_portion($1::regclass, $2, $3, $4, $5::jsonb, $6)")
            .bind(table)
            .bind(id)
            .bind(from)
            .bind(to)
            .bind(patch)
            .bind(changeset)
            .execute(&mut *conn)
            .await
            .map_err(|e| WriteError::Store(store(&e)))?;
    }
    Ok(())
}

/// Takes the VLAN `vlan` off the interface `id` over `period`, version by version: an
/// access port without its VLAN, or a trunk without a tagged VLAN, has no 802.1Q mode left,
/// unless the trunk keeps a native VLAN, which it then carries untagged (REQ-009-14).
pub(crate) async fn unlink_vlan(
    conn: &mut PgConnection,
    id: Uuid,
    vlan: Uuid,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    let found: Vec<(OffsetDateTime, Option<OffsetDateTime>, Value)> = sqlx::query_as(
        "SELECT greatest(lower(i.valid_period), coalesce($2::timestamptz, now())),
                CASE WHEN $3::timestamptz IS NULL THEN upper(i.valid_period)
                     WHEN upper_inf(i.valid_period) THEN $3
                     ELSE least(upper(i.valid_period), $3) END,
                jsonb_build_object('vlan_mode', m.mode, 'access_vlan_id', m.access,
                                   'native_vlan_id', m.native, 'tagged_vlan_ids', to_jsonb(x.t))
           FROM machines.interface i
          CROSS JOIN LATERAL (SELECT nullif(i.access_vlan_id, $4) AS a,
                                     nullif(i.native_vlan_id, $4) AS n,
                                     array_remove(i.tagged_vlan_ids, $4) AS t) x
          CROSS JOIN LATERAL (
              SELECT i.vlan_mode = 'trunk' AND cardinality(x.t) = 0 AS emptied) e
          CROSS JOIN LATERAL (
              SELECT CASE WHEN i.vlan_mode = 'access' AND x.a IS NULL THEN 'none'
                          WHEN e.emptied AND x.n IS NULL THEN 'none'
                          WHEN e.emptied THEN 'access'
                          ELSE i.vlan_mode END AS mode,
                     CASE WHEN e.emptied THEN x.n ELSE x.a END AS access,
                     CASE WHEN e.emptied THEN NULL ELSE x.n END AS native) m
          WHERE i.id = $1 AND i.valid_period && tstzrange(coalesce($2::timestamptz, now()), $3)
            AND (i.access_vlan_id = $4 OR i.native_vlan_id = $4 OR $4 = ANY(i.tagged_vlan_ids))
          ORDER BY 1",
    )
    .bind(id)
    .bind(from)
    .bind(to)
    .bind(vlan)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| WriteError::Store(store(&e)))?;
    portions(conn, INTERFACE_TABLE, id, found, changeset).await
}

/// Clears the host machine, the host cluster or the cluster `target` of the machine `id`
/// over `period`, version by version (REQ-009-14).
pub(crate) async fn unlink_host(
    conn: &mut PgConnection,
    id: Uuid,
    target: Uuid,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    let found: Vec<(OffsetDateTime, Option<OffsetDateTime>, Value)> = sqlx::query_as(
        "SELECT greatest(lower(valid_period), coalesce($2::timestamptz, now())),
                CASE WHEN $3::timestamptz IS NULL THEN upper(valid_period)
                     WHEN upper_inf(valid_period) THEN $3
                     ELSE least(upper(valid_period), $3) END,
                CASE WHEN host_machine_id = $4
                     THEN jsonb_build_object('host_machine_id', NULL) ELSE '{}' END
                || CASE WHEN host_cluster_id = $4
                        THEN jsonb_build_object('host_cluster_id', NULL) ELSE '{}' END
                || CASE WHEN member_cluster_id = $4
                        THEN jsonb_build_object('member_cluster_id', NULL) ELSE '{}' END
           FROM machines.machine
          WHERE id = $1 AND valid_period && tstzrange(coalesce($2::timestamptz, now()), $3)
            AND $4 IN (host_machine_id, host_cluster_id, member_cluster_id)
          ORDER BY 1",
    )
    .bind(id)
    .bind(from)
    .bind(to)
    .bind(target)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| WriteError::Store(store(&e)))?;
    portions(conn, MACHINE_TABLE, id, found, changeset).await
}

/// The attachments the current transaction makes or ends, as rows of the section
/// `machines.attachments` (REQ-009-23): per interface and address, each portion of time
/// attached before and no longer, or attached now and not before. Before is read from the
/// versions asserted before the transaction, after from the current ones.
pub(crate) async fn attachment_changes(
    conn: &mut PgConnection,
) -> Result<Vec<kartenn_service::module::DiffRow>, StoreError> {
    type Change = (
        Uuid,
        Uuid,
        Uuid,
        Option<bool>,
        Option<bool>,
        i64,
        Option<i64>,
    );
    let found: Vec<Change> = sqlx::query_as(
        "WITH touched AS (
             SELECT id FROM machines.attachment WHERE sys_start = now()
             UNION
             SELECT id FROM machines.attachment_history WHERE sys_end = now()),
         before AS (
             SELECT interface_id, address_id, primary_for_family, valid_period
               FROM machines.attachment_history
              WHERE id IN (SELECT id FROM touched) AND sys_end = now() AND sys_start < now()
             UNION ALL
             SELECT interface_id, address_id, primary_for_family, valid_period
               FROM machines.attachment
              WHERE id IN (SELECT id FROM touched) AND sys_start < now()),
         after AS (
             SELECT interface_id, address_id, primary_for_family, valid_period
               FROM machines.attachment WHERE id IN (SELECT id FROM touched)),
         b AS (SELECT interface_id, address_id, bool_or(primary_for_family) AS p,
                      range_agg(valid_period) AS r
                 FROM before GROUP BY 1, 2),
         f AS (SELECT interface_id, address_id, bool_or(primary_for_family) AS p,
                      range_agg(valid_period) AS r
                 FROM after GROUP BY 1, 2),
         pairs AS (
             SELECT coalesce(b.interface_id, f.interface_id) AS interface_id,
                    coalesce(b.address_id, f.address_id) AS address_id,
                    b.p AS bp, f.p AS fp,
                    coalesce(b.r, '{}'::tstzmultirange) AS br,
                    coalesce(f.r, '{}'::tstzmultirange) AS fr
               FROM b FULL JOIN f USING (interface_id, address_id)),
         portions AS (
             SELECT interface_id, address_id, bp, NULL::boolean AS fp, x AS period
               FROM pairs, unnest(br - fr) AS x
             UNION ALL
             SELECT interface_id, address_id, NULL, fp, x
               FROM pairs, unnest(fr - br) AS x)
         SELECT p.interface_id, i.machine_id, p.address_id, p.bp, p.fp,
                (extract(epoch FROM lower(p.period)) * 1000000)::bigint,
                (extract(epoch FROM upper(p.period)) * 1000000)::bigint
           FROM portions p
           JOIN LATERAL (SELECT machine_id FROM machines.interface
                          WHERE id = p.interface_id LIMIT 1) i ON true
          ORDER BY i.machine_id, p.interface_id, p.address_id, lower(p.period)",
    )
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(found
        .into_iter()
        .map(|(interface, machine, address, before, after, start, end)| {
            let side = |primary: Option<bool>| {
                primary.map(|primary| {
                    serde_json::json!({
                        "machine": machine, "address": address, "primary": primary,
                    })
                })
            };
            kartenn_service::module::DiffRow {
                object: interface,
                kind: crate::module::INTERFACE.to_owned(),
                before: side(before),
                after: side(after),
                period: Period::new(Instant(start), end.map(Instant)).ok(),
            }
        })
        .collect())
}
