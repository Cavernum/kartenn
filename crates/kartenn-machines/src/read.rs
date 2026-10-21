//! The port of reading of the module (feature `pg`; REQ-009-16, 22, 25; design « Port de
//! lecture »): the addresses of a machine or of an interface with the role of each
//! interface, the interfaces and machines of an address, the machines of a network, of a
//! host or of a cluster, and the values of the machines of a batch of addresses. Each
//! function reads at (`valid_at`, `as_of`) on a connection: that of a changeset, through
//! [`kartenn_service::module::ModuleTx`], sees the state the changeset leaves; a past
//! `as_of` reads the versions then asserted, from the tables and their history. Rights are
//! the caller's to apply (the routes filter by them, REQ-009-16).

use kartenn_core::period::Instant;
use kartenn_service::port::StoreError;
use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::pg::{store, time};

/// An address attached to an interface (REQ-009-22).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct AddressOf {
    /// The interface.
    pub interface: Uuid,
    /// Its name on its machine.
    pub interface_name: String,
    /// Its resolved role (`machines.interface_role`), by key.
    pub role: Option<String>,
    /// The address of the core.
    pub address: Uuid,
    /// Its value, without prefix length.
    pub value: String,
    /// Whether it is the primary address of its family on the interface.
    pub primary: bool,
}

/// An interface that holds an address (REQ-009-22).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct AttachmentOf {
    /// The interface.
    pub interface: Uuid,
    /// Its machine.
    pub machine: Uuid,
    /// The resolved role of the interface, by key.
    pub role: Option<String>,
}

/// The resolved value of a property for a machine, with the object it comes from
/// (REQ-003-14).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedValue {
    /// The key of the property.
    pub key: String,
    /// The value as JSON (closed-list keys); `None`: no value.
    pub value: Option<Value>,
    /// The object carrying the applicable assignment; `None` when nothing applies.
    pub origin: Option<Uuid>,
}

/// A machine that holds an address, through an interface, with the values asked
/// (REQ-009-25).
#[derive(Debug, Clone, PartialEq)]
pub struct MachineValues {
    /// The machine.
    pub machine: Uuid,
    /// The interface that holds the address.
    pub interface: Uuid,
    /// The values of the keys asked, in their order; a key not declared then is absent.
    pub values: Vec<ResolvedValue>,
}

/// The versions of `table` valid at `$1`, as asserted at `$2` (now when null): the current
/// rows, or the history for a past `$2` (as the dated reads of the core,
/// `kartenn_store_pg::temporal::snapshot`). `table` and `cols` are constants of this module.
fn snapshot(table: &str, cols: &str) -> String {
    format!(
        "(SELECT {cols} FROM {table}
           WHERE valid_period @> $1::timestamptz
             AND ($2::timestamptz IS NULL OR sys_start <= $2::timestamptz)
          UNION ALL
          SELECT {cols} FROM {table}_history
           WHERE $2::timestamptz IS NOT NULL AND valid_period @> $1::timestamptz
             AND sys_start <= $2::timestamptz AND sys_end > $2::timestamptz)"
    )
}

/// The role of the interface `i.id`, as a lateral join (`role.key`).
fn role_sql() -> String {
    let assignment = snapshot(
        "public.property_assignment",
        "object_id, property_id, kind, v_uuid",
    );
    let declaration = snapshot("public.property_declaration", "id, key");
    let value = snapshot("public.property_enum_value", "id, key");
    format!(
        "LEFT JOIN LATERAL (
             SELECT e.key FROM {assignment} pa
               JOIN {declaration} d ON d.id = pa.property_id AND d.key = 'machines.interface_role'
               JOIN {value} e ON e.id = pa.v_uuid
              WHERE pa.object_id = i.id AND pa.kind = 'value'
              LIMIT 1) role ON true"
    )
}

/// The addresses of the interfaces matching `filter` (on `i`, with `$3`).
async fn addresses_where(
    conn: &mut PgConnection,
    filter: &str,
    id: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<AddressOf>, StoreError> {
    let interface = snapshot("machines.interface", "id, machine_id, name");
    let attachment = snapshot(
        "machines.attachment",
        "interface_id, address_id, primary_for_family",
    );
    let address = snapshot("public.address", "id, address");
    let role = role_sql();
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT i.id AS interface, i.name AS interface_name, role.key AS role,
                x.id AS address, host(x.address) AS value, a.primary_for_family AS primary
           FROM {interface} i
           JOIN {attachment} a ON a.interface_id = i.id
           JOIN {address} x ON x.id = a.address_id
           {role}
          WHERE {filter} = $3
          ORDER BY i.name, x.address"
    )))
    .bind(time(valid_at))
    .bind(as_of.map(time))
    .bind(id)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The addresses of the machine `machine` at (`valid_at`, `as_of`), by interface, each with
/// the role of its interface (REQ-009-16, 22).
///
/// # Errors
///
/// Database errors.
pub async fn addresses_of_machine(
    conn: &mut PgConnection,
    machine: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<AddressOf>, StoreError> {
    addresses_where(conn, "i.machine_id", machine, valid_at, as_of).await
}

/// The addresses of the interface `interface` at (`valid_at`, `as_of`), with its role.
///
/// # Errors
///
/// Database errors.
pub async fn addresses_of_interface(
    conn: &mut PgConnection,
    interface: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<AddressOf>, StoreError> {
    addresses_where(conn, "i.id", interface, valid_at, as_of).await
}

/// The interfaces and machines that hold `address` at (`valid_at`, `as_of`), with the role
/// of each interface (REQ-009-22).
///
/// # Errors
///
/// Database errors.
pub async fn attachments_of_address(
    conn: &mut PgConnection,
    address: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<AttachmentOf>, StoreError> {
    let interface = snapshot("machines.interface", "id, machine_id");
    let attachment = snapshot("machines.attachment", "interface_id, address_id");
    let role = role_sql();
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT i.id AS interface, i.machine_id AS machine, role.key AS role
           FROM {attachment} a
           JOIN {interface} i ON i.id = a.interface_id
           {role}
          WHERE a.address_id = $3
          ORDER BY i.machine_id, i.id"
    )))
    .bind(time(valid_at))
    .bind(as_of.map(time))
    .bind(address)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The machines that hold an address of the network `prefix` at (`valid_at`, `as_of`), by
/// identifier, at most `limit` after `after` (REQ-009-16).
///
/// # Errors
///
/// Database errors.
pub async fn machines_in_network(
    conn: &mut PgConnection,
    prefix: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Uuid>, StoreError> {
    let network = snapshot("public.prefix", "id, domain_id, prefix");
    let address = snapshot("public.address", "id, domain_id, address");
    let attachment = snapshot("machines.attachment", "interface_id, address_id");
    let interface = snapshot("machines.interface", "id, machine_id");
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT DISTINCT i.machine_id
           FROM {network} p
           JOIN {address} x ON x.domain_id = p.domain_id AND x.address <<= p.prefix
           JOIN {attachment} a ON a.address_id = x.id
           JOIN {interface} i ON i.id = a.interface_id
          WHERE p.id = $3 AND ($4::uuid IS NULL OR i.machine_id > $4)
          ORDER BY i.machine_id
          LIMIT $5"
    )))
    .bind(time(valid_at))
    .bind(as_of.map(time))
    .bind(prefix)
    .bind(after)
    .bind(limit)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The machines hosted by the machine or the cluster `host` at (`valid_at`, `as_of`).
///
/// # Errors
///
/// Database errors.
pub async fn hosted_by(
    conn: &mut PgConnection,
    host: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<Uuid>, StoreError> {
    machines_where(
        conn,
        "host_machine_id = $3 OR host_cluster_id = $3",
        host,
        valid_at,
        as_of,
    )
    .await
}

/// The members of the cluster `cluster` at (`valid_at`, `as_of`).
///
/// # Errors
///
/// Database errors.
pub async fn members_of(
    conn: &mut PgConnection,
    cluster: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<Uuid>, StoreError> {
    machines_where(conn, "member_cluster_id = $3", cluster, valid_at, as_of).await
}

async fn machines_where(
    conn: &mut PgConnection,
    filter: &str,
    id: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<Uuid>, StoreError> {
    let machine = snapshot(
        "machines.machine",
        "id, host_machine_id, host_cluster_id, member_cluster_id",
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT DISTINCT id FROM {machine} m WHERE {filter} ORDER BY id"
    )))
    .bind(time(valid_at))
    .bind(as_of.map(time))
    .bind(id)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// For each of `addresses`, in their order, the machines that hold it at (`valid_at`,
/// `as_of`), with the interface that holds it and the resolved values of the properties
/// `keys` with their origin; an address held by no machine has an empty list (REQ-009-25).
/// One query for the batch; composing with the value of the addressing tree is the
/// reader's (010/S27).
///
/// # Errors
///
/// Database errors.
pub async fn machine_values_of_addresses(
    conn: &mut PgConnection,
    addresses: &[Uuid],
    keys: &[&str],
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<(Uuid, Vec<MachineValues>)>, StoreError> {
    type Row = (
        Uuid,
        Uuid,
        Uuid,
        Option<String>,
        Option<Uuid>,
        Option<Value>,
    );
    let attachment = snapshot("machines.attachment", "interface_id, address_id");
    let interface = snapshot("machines.interface", "id, machine_id");
    let declaration = snapshot(
        "public.property_declaration",
        "id, key, value_type, inherit_tree",
    );
    let origin = snapshot(
        "public.property_origin",
        "object_id, property_id, origin_object_id",
    );
    let assignment = snapshot(
        "public.property_assignment",
        "object_id, property_id, kind, v_uuid, v_uuids, v_text, v_texts, v_int, v_ints, \
         v_bool, v_date, v_dates",
    );
    let value = snapshot("public.property_enum_value", "id, key");
    let keys: Vec<String> = keys.iter().map(|&k| k.to_owned()).collect();
    let found: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH held AS (
             SELECT a.address_id, i.machine_id, i.id AS interface_id
               FROM {attachment} a JOIN {interface} i ON i.id = a.interface_id
              WHERE a.address_id = ANY($3)),
         wanted AS (
             SELECT d.id, d.key, d.value_type = 'enum' AS closed_list, d.inherit_tree,
                    array_position($4::text[], d.key) AS rank
               FROM {declaration} d WHERE d.key = ANY($4))
         SELECT h.address_id, h.machine_id, h.interface_id, w.key, src.origin,
                CASE
                    WHEN a.kind IS NULL OR a.kind = 'cleared' THEN NULL
                    WHEN w.closed_list AND a.v_uuid IS NOT NULL THEN
                        to_jsonb((SELECT e.key FROM {value} e WHERE e.id = a.v_uuid LIMIT 1))
                    WHEN w.closed_list THEN
                        (SELECT jsonb_agg(k ORDER BY k) FROM (
                            SELECT DISTINCT e.key AS k FROM {value} e
                             WHERE e.id = ANY(a.v_uuids)) x)
                    ELSE coalesce(to_jsonb(a.v_uuid), to_jsonb(a.v_uuids), to_jsonb(a.v_text),
                                  to_jsonb(a.v_texts), to_jsonb(a.v_int), to_jsonb(a.v_ints),
                                  to_jsonb(a.v_bool), to_jsonb(a.v_date), to_jsonb(a.v_dates))
                END
           FROM held h
           LEFT JOIN wanted w ON true
           LEFT JOIN LATERAL (
               SELECT CASE WHEN w.inherit_tree IS NULL THEN h.machine_id
                           ELSE (SELECT o.origin_object_id FROM {origin} o
                                  WHERE o.object_id = h.machine_id AND o.property_id = w.id
                                  LIMIT 1) END AS origin) src ON true
           LEFT JOIN {assignment} a
             ON a.object_id = src.origin AND a.property_id = w.id
          ORDER BY h.address_id, h.machine_id, h.interface_id, w.rank"
    )))
    .bind(time(valid_at))
    .bind(as_of.map(time))
    .bind(addresses)
    .bind(&keys)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    let mut out: Vec<(Uuid, Vec<MachineValues>)> =
        addresses.iter().map(|&a| (a, Vec::new())).collect();
    for (address, machine, interface, key, origin, value) in found {
        let Some((_, machines)) = out.iter_mut().find(|(a, _)| *a == address) else {
            continue;
        };
        let held = |m: &MachineValues| m.machine == machine && m.interface == interface;
        if !machines.iter().any(held) {
            machines.push(MachineValues {
                machine,
                interface,
                values: Vec::new(),
            });
        }
        if let Some(key) = key
            && let Some(entry) = machines.iter_mut().find(|m| held(m))
        {
            // An assignment that does not apply leaves the value absent, not its origin.
            let origin = origin.filter(|_| value.is_some());
            entry.values.push(ResolvedValue { key, value, origin });
        }
    }
    Ok(out)
}

/// The edges of the module that start or end at `object` at (`valid_at`, `as_of`), each
/// with the period of the version it comes from (REQ-009-17).
///
/// # Errors
///
/// Database errors.
pub async fn edges_of(
    conn: &mut PgConnection,
    object: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<kartenn_service::module::Edge>, StoreError> {
    type Row = (String, Uuid, Uuid, OffsetDateTime, Option<OffsetDateTime>);
    let attachment = snapshot(
        "machines.attachment",
        "interface_id, address_id, valid_period",
    );
    let interface = snapshot(
        "machines.interface",
        "id, access_vlan_id, native_vlan_id, tagged_vlan_ids, valid_period",
    );
    let machine = snapshot(
        "machines.machine",
        "id, host_machine_id, host_cluster_id, member_cluster_id, valid_period",
    );
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH e (name, origin, target, period) AS (
             SELECT 'machines.has_address', interface_id, address_id, valid_period
               FROM {attachment} a
             UNION ALL
             SELECT 'machines.access', id, access_vlan_id, valid_period
               FROM {interface} i WHERE access_vlan_id IS NOT NULL
             UNION ALL
             SELECT 'machines.native', id, native_vlan_id, valid_period
               FROM {interface} i WHERE native_vlan_id IS NOT NULL
             UNION ALL
             SELECT 'machines.tagged', i.id, t.vlan, i.valid_period
               FROM {interface} i, unnest(i.tagged_vlan_ids) AS t (vlan)
             UNION ALL
             SELECT 'machines.hosted_on', id, host_machine_id, valid_period
               FROM {machine} m WHERE host_machine_id IS NOT NULL
             UNION ALL
             SELECT 'machines.hosted_in', id, host_cluster_id, valid_period
               FROM {machine} m WHERE host_cluster_id IS NOT NULL
             UNION ALL
             SELECT 'machines.member_of', id, member_cluster_id, valid_period
               FROM {machine} m WHERE member_cluster_id IS NOT NULL)
         SELECT name, origin, target, lower(period), upper(period) FROM e
          WHERE origin = $3 OR target = $3
          ORDER BY name, origin, target"
    )))
    .bind(time(valid_at))
    .bind(as_of.map(time))
    .bind(object)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    let named = |name: &str| {
        kartenn_service::module::Module::edges(&crate::module::Machines)
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.name)
    };
    Ok(rows
        .into_iter()
        .filter_map(|(name, from, to, start, end)| {
            let period =
                kartenn_core::period::Period::new(instant(start), end.map(instant)).ok()?;
            Some(kartenn_service::module::Edge {
                name: named(&name)?,
                from,
                to,
                period,
            })
        })
        .collect())
}

/// An instant of PostgreSQL as the core reads it.
#[must_use]
pub fn instant(t: OffsetDateTime) -> Instant {
    Instant(i64::try_from(t.unix_timestamp_nanos() / 1_000).unwrap_or(i64::MAX))
}
