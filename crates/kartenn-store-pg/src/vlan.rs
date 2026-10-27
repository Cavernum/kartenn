//! The VLANs of the core in PostgreSQL (spec 026, decision C2): the tables `vlan_group` and
//! `vlan` of the core (migration `0072_vlan_core.sql`), and the handler that completes,
//! writes and checks the operations of the VLANs in the write pipeline
//! (`kartenn_service::vlan` plans them). Every write on a VLAN takes the key of its groups
//! (REQ-008-27) before the uniqueness of its VID and name is checked.

use kartenn_core::period::{Instant, Period};
use kartenn_core::rules::{
    SLUG_DUPLICATE, VLAN_DOMAIN_MISMATCH, VLAN_GROUP_ANCHOR, VLAN_NAME_DUPLICATE,
    VLAN_PREFIX_NOT_NETWORK, VLAN_SITE_MISMATCH, VLAN_VID_DUPLICATE, VLAN_VID_OUT_OF_RANGE,
    Violation,
};
use kartenn_service::ObjectKind;
use kartenn_service::module::{
    BoxFuture, CheckContext, Findings, ModuleHandler, ModulePlan, ModuleTx, Reference,
};
use kartenn_service::operations::{ModuleOperation, Ref};
use kartenn_service::port::{StoreError, WriteError};
use kartenn_service::vlan::{self, Completed, VlanCompleted, id_in};
use serde_json::json;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

/// The handler of the operations of the VLANs, given to the pipeline by the transactions
/// of the store (`WriteTx::core_handler`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Handler;

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut()
        .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))
}

fn store(e: &sqlx::Error) -> StoreError {
    StoreError(e.to_string())
}

/// An instant as PostgreSQL reads it.
fn time(i: Instant) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).expect("an instant")
}

/// The bounds of `period` to bind; a `None` start is the instant of the transaction.
fn bounds(period: Option<Period>) -> (Option<OffsetDateTime>, Option<OffsetDateTime>) {
    period.map_or((None, None), |p| (Some(time(p.start())), p.end().map(time)))
}

/// The objects living from `at` on that reference `target`, of kind `to` (REQ-008-10):
/// the VLANs of a group, the groups anchored on an entity, a site or a domain.
async fn referrers(
    conn: &mut PgConnection,
    to: &str,
    target: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Vec<Uuid>, StoreError> {
    let sql = if to == vlan::GROUP {
        "SELECT DISTINCT id FROM vlan
          WHERE group_id = $1
            AND valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)
            AND $3::text IS NOT NULL
          ORDER BY id"
    } else {
        "SELECT DISTINCT id FROM vlan_group
          WHERE anchor_id = $1 AND anchor_kind = $3
            AND valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)
          ORDER BY id"
    };
    sqlx::query_scalar(sql)
        .bind(target)
        .bind(at)
        .bind(to)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))
}

/// The identifier of the property `key` declared at `at`.
async fn property(
    conn: &mut PgConnection,
    key: &str,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT id FROM property_declaration
          WHERE key = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(key)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// The resolved owner of `object` at `at` (ADR 0014).
async fn owner(
    conn: &mut PgConnection,
    object: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar("SELECT kartenn_owner($1, coalesce($2::timestamptz, now()))")
        .bind(object)
        .bind(at)
        .fetch_one(conn)
        .await
        .map_err(|e| store(&e))
}

/// The anchor of the group `id` at `at`: its kind and its object.
async fn anchor(
    conn: &mut PgConnection,
    id: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Option<(String, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT anchor_kind, anchor_id FROM vlan_group
          WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(id)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// A database error as a violation of the rule its constraint backs, else a failure.
fn written(e: &sqlx::Error, id: Uuid) -> WriteError {
    let constraint = e
        .as_database_error()
        .and_then(|db| db.constraint())
        .map(str::to_owned);
    match constraint.as_deref() {
        Some("vlan_group_slug_unique") => WriteError::Rule(
            Violation::new(
                &SLUG_DUPLICATE,
                "the slug of a group of VLAN is taken over the period",
            )
            .with_subjects([id]),
        ),
        Some("vlan_vid_unique") => WriteError::Rule(
            Violation::new(
                &VLAN_VID_DUPLICATE,
                "the VID of the VLAN is taken in its group over the period",
            )
            .with_subjects([id]),
        ),
        Some("vlan_name_unique") => WriteError::Rule(
            Violation::new(
                &VLAN_NAME_DUPLICATE,
                "the name of the VLAN is taken in its group over the period",
            )
            .with_subjects([id]),
        ),
        Some("vlan_group_check") => WriteError::Rule(
            Violation::new(
                &VLAN_VID_OUT_OF_RANGE,
                "the range of a group goes from vid_min to vid_max, within 1 to 4094",
            )
            .with_subjects([id]),
        ),
        _ => WriteError::Store(store(e)),
    }
}

/// Refuses, before writing, what the constraints of the table would refuse: a failed
/// statement would abort the transaction of the changeset, whose other violations must
/// still be found. `slug` taken by another group over the period; bounds that, merged
/// with the versions of `id`, would leave `vid_min` above `vid_max`.
async fn admissible(
    conn: &mut PgConnection,
    id: Uuid,
    slug: Option<&str>,
    limits: (Option<i64>, Option<i64>),
    period: Option<Period>,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    if let Some(slug) = slug {
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM vlan_group
                             WHERE slug = $1 AND id <> $2
                               AND valid_period && tstzrange(coalesce($3::timestamptz, now()), $4))",
        )
        .bind(slug)
        .bind(id)
        .bind(from)
        .bind(to)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
        if taken {
            return Err(WriteError::Rule(
                Violation::new(
                    &SLUG_DUPLICATE,
                    format!("the slug {slug} of a group of VLAN is taken over the period"),
                )
                .with_subjects([id]),
            ));
        }
    }
    if limits != (None, None) {
        let inverted: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM vlan_group
                             WHERE id = $1
                               AND valid_period && tstzrange(coalesce($2::timestamptz, now()), $3)
                               AND coalesce($4::integer, vid_min) > coalesce($5::integer, vid_max))",
        )
        .bind(id)
        .bind(from)
        .bind(to)
        .bind(limits.0.and_then(|v| i32::try_from(v).ok()))
        .bind(limits.1.and_then(|v| i32::try_from(v).ok()))
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
        if inverted {
            return Err(WriteError::Rule(
                Violation::new(
                    &VLAN_VID_OUT_OF_RANGE,
                    "the range of a group goes from vid_min to vid_max: vid_min is above",
                )
                .with_subjects([id]),
            ));
        }
    }
    Ok(())
}

/// A new group over `period`.
async fn insert_group(
    conn: &mut PgConnection,
    id: Uuid,
    create: &serde_json::Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let anchor_id: Uuid = create["anchor_id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| StoreError("a planned anchor".into()))?;
    let slug = create["slug"].as_str().unwrap_or_default();
    admissible(conn, id, Some(slug), (None, None), period).await?;
    let (from, to) = bounds(period);
    let limit = |name: &str, default: i64| {
        i16::try_from(create[name].as_i64().unwrap_or(default)).unwrap_or(i16::MAX)
    };
    sqlx::query(
        "INSERT INTO vlan_group (id, slug, anchor_kind, anchor_id, vid_min, vid_max,
                                 valid_period, changeset_id)
         VALUES ($1, $2, $3, $4, $5, $6, tstzrange(coalesce($7::timestamptz, now()), $8), $9)",
    )
    .bind(id)
    .bind(slug)
    .bind(create["anchor_kind"].as_str().unwrap_or_default())
    .bind(anchor_id)
    .bind(limit("vid_min", 1))
    .bind(limit("vid_max", 4094))
    .bind(from)
    .bind(to)
    .bind(changeset)
    .execute(conn)
    .await
    .map(drop)
    .map_err(|e| written(&e, id))
}

/// Applies `patch` (columns of the group and their values) to `id` over `period` ("FOR
/// PORTION OF", migration 0018).
async fn patch_group(
    conn: &mut PgConnection,
    id: Uuid,
    patch: &serde_json::Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    admissible(
        conn,
        id,
        patch.get("slug").and_then(serde_json::Value::as_str),
        (
            patch.get("vid_min").and_then(serde_json::Value::as_i64),
            patch.get("vid_max").and_then(serde_json::Value::as_i64),
        ),
        period,
    )
    .await?;
    portion(conn, "vlan_group", id, patch, period, changeset).await
}

/// `kartenn_portion` of `patch` on the versions of `id` in `table` over `period`.
async fn portion(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    patch: &serde_json::Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    let versions: i32 = sqlx::query_scalar(
        "SELECT kartenn_portion($1::regclass, $2, coalesce($3::timestamptz, now()), $4,
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
    .map_err(|e| written(&e, id))?;
    if versions == 0 {
        return Err(WriteError::NoVersion(id));
    }
    Ok(())
}

/// The VLANs of the groups `ids` outside the range of their group where both live, as
/// `(group, vid, name)`.
async fn excluded_vlans(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, i16, String)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT g.id, v.vid, v.name
           FROM vlan_group g
           JOIN vlan v ON v.group_id = g.id AND v.valid_period && g.valid_period
          WHERE g.id = ANY($1) AND (v.vid < g.vid_min OR v.vid > g.vid_max)
          ORDER BY g.id, v.vid",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The groups among `ids` whose anchor does not live over one of their versions, as
/// `(group, anchor kind, anchor)`.
async fn dead_anchors(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT g.id, g.anchor_kind, g.anchor_id
           FROM vlan_group g
          WHERE g.id = ANY($1)
            AND NOT EXISTS (
                SELECT 1 FROM (
                    SELECT id, valid_period FROM entity WHERE g.anchor_kind = 'entity'
                    UNION ALL
                    SELECT id, valid_period FROM site WHERE g.anchor_kind = 'site'
                    UNION ALL
                    SELECT id, valid_period FROM addressing_domain
                     WHERE g.anchor_kind = 'domain'
                ) a
                WHERE a.id = g.anchor_id
                GROUP BY a.id
                HAVING range_agg(a.valid_period) @> g.valid_period)",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The group of the VLAN `id` at `at`.
async fn vlan_group(
    conn: &mut PgConnection,
    id: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT group_id FROM vlan
          WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(id)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// The domain of the group `id` at `at`, when it is anchored on one.
async fn group_domain(
    conn: &mut PgConnection,
    id: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    Ok(anchor(conn, id, at)
        .await?
        .and_then(|(kind, anchor)| (kind == "domain").then_some(anchor)))
}

/// What a VLAN would be over the period, version by version (`me`: group `g`, `vid`,
/// `name`, period `p`), against the other VLANs and its group: whether its VID or its name
/// is taken in its group, and whether its VID leaves the range of a group that does not
/// cover it or does not live over its period. `$1` is the VLAN.
macro_rules! verdict {
    () => {
        "SELECT
           EXISTS (SELECT 1 FROM me JOIN vlan o
                     ON o.id <> $1 AND o.group_id = me.g AND o.vid = me.vid
                    AND o.valid_period && me.p),
           EXISTS (SELECT 1 FROM me JOIN vlan o
                     ON o.id <> $1 AND o.group_id = me.g AND o.name = me.name
                    AND o.valid_period && me.p),
           EXISTS (SELECT 1 FROM me
                    WHERE NOT coalesce((SELECT range_agg(g.valid_period) @> me.p
                                               AND bool_and(me.vid BETWEEN g.vid_min AND g.vid_max)
                                          FROM vlan_group g
                                         WHERE g.id = me.g AND g.valid_period && me.p), false))"
    };
}

/// A new VLAN, as planned.
const NEW_VLAN: &str = concat!(
    "WITH me AS (SELECT $2::uuid AS g, $3::smallint AS vid, $4::text AS name,
                        tstzrange(coalesce($5::timestamptz, now()), $6) AS p) ",
    verdict!()
);

/// A VLAN once patched: its versions over the period, the fields of the patch replacing
/// theirs.
const PATCHED_VLAN: &str = concat!(
    "WITH me AS (SELECT coalesce($2::uuid, v.group_id) AS g,
                        coalesce($3::smallint, v.vid) AS vid,
                        coalesce($4::text, v.name) AS name,
                        v.valid_period * tstzrange(coalesce($5::timestamptz, now()), $6) AS p
                   FROM vlan v
                  WHERE v.id = $1
                    AND v.valid_period && tstzrange(coalesce($5::timestamptz, now()), $6)) ",
    verdict!()
);

/// Refuses, before writing and under the key of its groups (REQ-008-27), what the rules
/// of the VLAN `id` would refuse (REQ-008-14 to 16): a failed statement would abort the
/// transaction of the changeset. The constraints of the table stay the last defence.
async fn admissible_vlan(
    conn: &mut PgConnection,
    sql: &'static str,
    id: Uuid,
    fields: (Option<Uuid>, Option<i16>, Option<&str>),
    period: Option<Period>,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    let (vid_taken, name_taken, out_of_range): (bool, bool, bool) = sqlx::query_as(sql)
        .bind(id)
        .bind(fields.0)
        .bind(fields.1)
        .bind(fields.2)
        .bind(from)
        .bind(to)
        .fetch_one(conn)
        .await
        .map_err(|e| store(&e))?;
    let refused = |rule, message: &str| {
        Err(WriteError::Rule(
            Violation::new(rule, message).with_subjects([id]),
        ))
    };
    if out_of_range {
        return refused(
            &VLAN_VID_OUT_OF_RANGE,
            "the VID of the VLAN is outside the range of its group, or the group does not \
             live over the period",
        );
    }
    if vid_taken {
        return refused(
            &VLAN_VID_DUPLICATE,
            "the VID of the VLAN is taken in its group over the period",
        );
    }
    if name_taken {
        return refused(
            &VLAN_NAME_DUPLICATE,
            "the name of the VLAN is taken in its group over the period",
        );
    }
    Ok(())
}

/// A new VLAN over `period`, from its planned fields (`group_id`, `vid`, `name`,
/// `status`).
async fn insert_vlan(
    conn: &mut PgConnection,
    id: Uuid,
    create: &serde_json::Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let group: Uuid = create["group_id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| StoreError("a planned group".into()))?;
    let vid = create["vid"]
        .as_i64()
        .and_then(|v| i16::try_from(v).ok())
        .ok_or_else(|| StoreError("a planned VID".into()))?;
    let name = create["name"].as_str().unwrap_or_default();
    admissible_vlan(
        &mut *conn,
        NEW_VLAN,
        id,
        (Some(group), Some(vid), Some(name)),
        period,
    )
    .await?;
    let (from, to) = bounds(period);
    sqlx::query(
        "INSERT INTO vlan (id, group_id, vid, name, status, valid_period, changeset_id)
         VALUES ($1, $2, $3, $4, $5, tstzrange(coalesce($6::timestamptz, now()), $7), $8)",
    )
    .bind(id)
    .bind(group)
    .bind(vid)
    .bind(name)
    .bind(create["status"].as_str().unwrap_or("active"))
    .bind(from)
    .bind(to)
    .bind(changeset)
    .execute(conn)
    .await
    .map(drop)
    .map_err(|e| written(&e, id))
}

/// Applies `patch` (`group_id`, `vid`, `name`, `status`) to the VLAN `id` over `period`.
async fn patch_vlan(
    conn: &mut PgConnection,
    id: Uuid,
    patch: &serde_json::Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let group = patch
        .get("group_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|s| s.parse::<Uuid>().ok());
    let vid = patch
        .get("vid")
        .and_then(serde_json::Value::as_i64)
        .and_then(|v| i16::try_from(v).ok());
    let name = patch.get("name").and_then(serde_json::Value::as_str);
    admissible_vlan(&mut *conn, PATCHED_VLAN, id, (group, vid, name), period).await?;
    portion(conn, "vlan", id, patch, period, changeset).await
}

/// The prefixes among `ids` that carry `core.vlan` while not a network, on some part of
/// the period of the assignment (REQ-008-17).
async fn vlan_off_network(conn: &mut PgConnection, ids: &[Uuid]) -> Result<Vec<Uuid>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT DISTINCT a.object_id
           FROM property_assignment a
           JOIN property_declaration d ON d.id = a.property_id AND d.key = 'core.vlan'
           JOIN prefix p ON p.id = a.object_id AND p.valid_period && a.valid_period
          WHERE a.object_id = ANY($1) AND a.kind = 'value' AND p.nature <> 'network'
          ORDER BY 1",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The prefixes that carry, now or later, one of the VLANs `vlans` or a VLAN of one of the
/// groups `groups`.
async fn networks_of(
    conn: &mut PgConnection,
    vlans: &[Uuid],
    groups: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    if vlans.is_empty() && groups.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT DISTINCT a.object_id
           FROM property_assignment a
           JOIN property_declaration d ON d.id = a.property_id AND d.key = 'core.vlan'
          WHERE a.kind = 'value'
            AND (upper(a.valid_period) IS NULL OR upper(a.valid_period) > now())
            AND (a.v_uuid = ANY($1)
                 OR a.v_uuid IN (SELECT id FROM vlan WHERE group_id = ANY($2)))",
    )
    .bind(vlans)
    .bind(groups)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The networks that carry `core.vlan` and whose resolved `core.site`, `core.owner` or
/// `core.vlan` the changeset changes, on them or on a prefix that contains them (the site
/// of a network is that of its nearest ancestor): read from the detail of the
/// differential of the properties, recorded in the transaction (REQ-003-19), so that V9 is
/// evaluated again where it may have changed, and only there.
async fn diff_networks(conn: &mut PgConnection) -> Result<Vec<Uuid>, StoreError> {
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
        "SELECT DISTINCT n.id
           FROM (SELECT DISTINCT object_id FROM pg_temp.diff_detail
                  WHERE property_key IN ('core.site', 'core.owner', 'core.vlan')) d
           JOIN prefix c ON c.id = d.object_id
           JOIN prefix n ON n.domain_id = c.domain_id AND n.prefix <<= c.prefix
          WHERE EXISTS (SELECT 1 FROM property_assignment a
                          JOIN property_declaration pd
                            ON pd.id = a.property_id AND pd.key = 'core.vlan'
                         WHERE a.object_id = n.id)",
    )
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The networks among `ids` whose VLAN is in a group anchored on a site outside the site
/// of the network (`site`) or on another domain (`domain`), V9: each assignment of
/// `core.vlan` is read at its start, or now when it started before.
async fn mismatches(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String)>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as(
        "WITH links AS (
             SELECT a.object_id AS network, a.v_uuid AS vlan,
                    greatest(lower(a.valid_period), now()) AS t
               FROM property_assignment a
               JOIN property_declaration d
                 ON d.id = a.property_id AND d.key = 'core.vlan'
              WHERE a.object_id = ANY($1) AND a.kind = 'value'
                AND (upper(a.valid_period) IS NULL OR upper(a.valid_period) > now())
         ), anchored AS (
             SELECT l.network, l.t, g.anchor_kind, g.anchor_id
               FROM links l
               JOIN vlan v ON v.id = l.vlan AND v.valid_period @> l.t
               JOIN vlan_group g ON g.id = v.group_id AND g.valid_period @> l.t
         )
         SELECT x.network, 'site'
           FROM anchored x
          CROSS JOIN LATERAL (SELECT kartenn_site(x.network, x.t) AS site) s
          WHERE x.anchor_kind = 'site' AND s.site IS NOT NULL
            AND NOT EXISTS (
                WITH RECURSIVE up(id) AS (
                    SELECT x.anchor_id
                    UNION
                    SELECT st.parent_id FROM site st JOIN up ON st.id = up.id
                     WHERE st.valid_period @> x.t AND st.parent_id IS NOT NULL)
                SELECT 1 FROM up WHERE up.id = s.site)
         UNION
         SELECT x.network, 'domain'
           FROM anchored x
           JOIN prefix p ON p.id = x.network AND p.valid_period @> x.t
          WHERE x.anchor_kind = 'domain' AND p.domain_id <> x.anchor_id
         ORDER BY 1, 2",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

impl Handler {
    /// Completes an operation on a group (`create_vlan_group`, `update_vlan_group`).
    async fn prepare_group(
        conn: &mut PgConnection,
        operation: &mut ModuleOperation,
        at: Option<OffsetDateTime>,
    ) -> Result<(), StoreError> {
        let mut completed = Completed {
            owner_property: property(conn, "core.owner", at).await?,
            site_property: property(conn, "core.site", at).await?,
            anchor_kind: None,
        };
        let body = &mut operation.body;
        if operation.name == "update_vlan_group"
            && let Some(group) = id_in(body, "id")
        {
            completed.anchor_kind = anchor(conn, group, at).await?.map(|(kind, _)| kind);
        }
        // A group anchored on a domain is owned, unless told otherwise, by the resolved
        // owner of the domain (V3).
        let domain = body
            .get("anchor")
            .and_then(|anchor| anchor.get("domain"))
            .cloned()
            .and_then(|r| serde_json::from_value::<Ref>(r).ok());
        if !body.contains_key("owner")
            && let Some(Ref::Id(domain)) = domain
            && let Some(owner) = owner(conn, domain, at).await?
        {
            body.insert("owner".into(), json!({ "id": owner }));
        }
        body.insert(
            "completed".into(),
            serde_json::to_value(&completed).expect("a JSON object"),
        );
        Ok(())
    }

    /// Completes an operation on a VLAN (`create_vlan`, `update_vlan`): its current group
    /// and the domains of the groups it is written in. A group created in the same
    /// changeset is locked by its own operation.
    async fn prepare_vlan(
        conn: &mut PgConnection,
        operation: &mut ModuleOperation,
        at: Option<OffsetDateTime>,
    ) -> Result<(), StoreError> {
        let body = &mut operation.body;
        let mut completed = VlanCompleted::default();
        if operation.name == "update_vlan"
            && let Some(vlan) = id_in(body, "id")
        {
            completed.current_group = vlan_group(conn, vlan, at).await?;
        }
        let groups: Vec<Uuid> = completed
            .current_group
            .into_iter()
            .chain(id_in(body, "group"))
            .collect();
        for group in groups {
            if let Some(domain) = group_domain(conn, group, at).await? {
                completed.domains.push(domain);
            }
        }
        completed.domains.sort_unstable();
        completed.domains.dedup();
        body.insert(
            "completed".into(),
            serde_json::to_value(&completed).expect("a JSON object"),
        );
        Ok(())
    }
}

impl ModuleHandler for Handler {
    fn prepare<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        operation: &'a mut ModuleOperation,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            let (at, _) = bounds(period);
            let conn = connection(tx)?;
            match operation.name.as_str() {
                "create_vlan" | "update_vlan" => Self::prepare_vlan(conn, operation, at).await,
                _ => Self::prepare_group(conn, operation, at).await,
            }
        })
    }

    fn plan(
        &self,
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        vlan::plan(operation, object, resolve)
    }

    fn apply<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        plan: &'a ModulePlan,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(async move {
            let conn = connection(tx)?;
            let id = plan.object;
            let is_vlan = plan.kind == vlan::vlan_kind();
            match (is_vlan, plan.data.get("create")) {
                (true, Some(create)) => insert_vlan(conn, id, create, period, changeset).await,
                (true, None) => patch_vlan(conn, id, &plan.data["update"], period, changeset).await,
                (false, Some(create)) => insert_group(conn, id, create, period, changeset).await,
                (false, None) => {
                    patch_group(conn, id, &plan.data["update"], period, changeset).await
                }
            }
        })
    }

    fn referrers<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        reference: &'a Reference,
        target: Uuid,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<Vec<Uuid>, StoreError>> {
        Box::pin(async move {
            let (at, _) = bounds(period);
            referrers(connection(tx)?, reference.to, target, at).await
        })
    }

    fn check<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Findings, StoreError>> {
        Box::pin(async move {
            let of = |wanted| -> Vec<Uuid> {
                context
                    .touched
                    .iter()
                    .filter(|(kind, _)| *kind == wanted)
                    .map(|(_, id)| *id)
                    .collect()
            };
            let (groups, vlans) = (of(vlan::group_kind()), of(vlan::vlan_kind()));
            let prefixes: Vec<Uuid> = context
                .objects
                .iter()
                .filter(|(kind, _)| *kind == ObjectKind::Prefix)
                .map(|(_, id)| *id)
                .collect();
            let conn = connection(tx)?;
            // Networks whose resolved site, owner or VLAN changed, by the changeset's
            // differential: V9 may have changed there, its anomaly closes once corrected.
            let changed = diff_networks(conn).await?;
            if groups.is_empty() && vlans.is_empty() && prefixes.is_empty() && changed.is_empty() {
                return Ok(Findings::default());
            }
            let mut found = Vec::new();
            // REQ-008-17: `core.vlan` on a network only, assigned or kept through a change
            // of nature.
            for prefix in vlan_off_network(conn, &prefixes).await? {
                found.push(
                    Violation::new(
                        &VLAN_PREFIX_NOT_NETWORK,
                        "core.vlan is assigned to a prefix that is not a network",
                    )
                    .with_subjects([prefix]),
                );
            }
            // REQ-008-18 (V9), warnings: the networks the changeset writes, and those
            // that carry a VLAN it moves or a VLAN of a group it reanchors.
            let mut networks = prefixes;
            networks.extend(networks_of(conn, &vlans, &groups).await?);
            networks.extend(changed);
            networks.sort_unstable();
            networks.dedup();
            for (network, kind) in mismatches(conn, &networks).await? {
                let (rule, message) = if kind == "site" {
                    (
                        &VLAN_SITE_MISMATCH,
                        "the VLAN of the network is in a group anchored on a site outside the \
                         site of the network",
                    )
                } else {
                    (
                        &VLAN_DOMAIN_MISMATCH,
                        "the VLAN of the network is in a group anchored on another domain",
                    )
                };
                found.push(Violation::new(rule, message).with_subjects([network]));
            }
            // REQ-008-13: a range keeps the VLANs of its group, listed.
            let excluded = excluded_vlans(conn, &groups).await?;
            for group in &groups {
                let vlans: Vec<String> = excluded
                    .iter()
                    .filter(|(g, _, _)| g == group)
                    .map(|(_, vid, name)| format!("{vid} ({name})"))
                    .collect();
                if !vlans.is_empty() {
                    found.push(
                        Violation::new(
                            &VLAN_VID_OUT_OF_RANGE,
                            format!("the range of the group excludes VLAN {}", vlans.join(", ")),
                        )
                        .with_subjects([*group]),
                    );
                }
            }
            for (group, kind, anchor) in dead_anchors(conn, &groups).await? {
                found.push(
                    Violation::new(
                        &VLAN_GROUP_ANCHOR,
                        format!("the {kind} {anchor} does not live over the period of the group"),
                    )
                    .with_subjects([group]),
                );
            }
            let reevaluated = networks
                .iter()
                .flat_map(|n| [(*n, VLAN_SITE_MISMATCH.id), (*n, VLAN_DOMAIN_MISMATCH.id)])
                .collect();
            Ok(Findings {
                violations: found,
                reevaluated,
            })
        })
    }
}
