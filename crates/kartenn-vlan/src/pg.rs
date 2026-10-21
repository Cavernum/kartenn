//! The storage of the module in PostgreSQL (feature `pg`): the tables of the schema `vlan`
//! (migration `0001_vlan.sql`), on the connection of the changeset or of a read.

use kartenn_core::period::{Instant, Period};
use kartenn_core::rules::Violation;
use kartenn_service::port::{StoreError, WriteError};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

/// The table of the groups, as the generic writes of the core name it.
pub(crate) const GROUP_TABLE: &str = "vlan.vlan_group";
/// The table of the VLANs, as the generic writes of the core name it.
pub(crate) const VLAN_TABLE: &str = "vlan.vlan";

/// The objects of the module living from `at` on that reference `target`, of type `to`
/// (REQ-008-10): the VLANs of a group, the groups anchored on an entity, a site or a
/// domain.
pub(crate) async fn referrers(
    conn: &mut PgConnection,
    to: &str,
    target: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Vec<Uuid>, StoreError> {
    let sql = if to == crate::module::GROUP {
        "SELECT DISTINCT id FROM vlan.vlan
          WHERE group_id = $1
            AND valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)
            AND $3::text IS NOT NULL
          ORDER BY id"
    } else {
        "SELECT DISTINCT id FROM vlan.vlan_group
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

/// The anchor of the group `id` at `at`: its kind and its object.
pub(crate) async fn anchor(
    conn: &mut PgConnection,
    id: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Option<(String, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT anchor_kind, anchor_id FROM vlan.vlan_group
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
                &kartenn_core::rules::SLUG_DUPLICATE,
                "the slug of a group of VLAN is taken over the period",
            )
            .with_subjects([id]),
        ),
        Some("vlan_vid_unique") => WriteError::Rule(
            Violation::new(
                &crate::rules::VID_DUPLICATE,
                "the VID of the VLAN is taken in its group over the period",
            )
            .with_subjects([id]),
        ),
        Some("vlan_name_unique") => WriteError::Rule(
            Violation::new(
                &crate::rules::NAME_DUPLICATE,
                "the name of the VLAN is taken in its group over the period",
            )
            .with_subjects([id]),
        ),
        Some("vlan_group_check") => WriteError::Rule(
            Violation::new(
                &crate::rules::VID_OUT_OF_RANGE,
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
    bounds: (Option<i64>, Option<i64>),
    period: Option<Period>,
) -> Result<(), WriteError> {
    let (from, to) = self::bounds(period);
    if let Some(slug) = slug {
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM vlan.vlan_group
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
                    &kartenn_core::rules::SLUG_DUPLICATE,
                    format!("the slug {slug} of a group of VLAN is taken over the period"),
                )
                .with_subjects([id]),
            ));
        }
    }
    if bounds != (None, None) {
        let inverted: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM vlan.vlan_group
                             WHERE id = $1
                               AND valid_period && tstzrange(coalesce($2::timestamptz, now()), $3)
                               AND coalesce($4::integer, vid_min) > coalesce($5::integer, vid_max))",
        )
        .bind(id)
        .bind(from)
        .bind(to)
        .bind(bounds.0.and_then(|v| i32::try_from(v).ok()))
        .bind(bounds.1.and_then(|v| i32::try_from(v).ok()))
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
        if inverted {
            return Err(WriteError::Rule(
                Violation::new(
                    &crate::rules::VID_OUT_OF_RANGE,
                    "the range of a group goes from vid_min to vid_max: vid_min is above",
                )
                .with_subjects([id]),
            ));
        }
    }
    Ok(())
}

/// A new group over `period`.
#[allow(clippy::too_many_arguments)] // the columns of a group
pub(crate) async fn insert_group(
    conn: &mut PgConnection,
    id: Uuid,
    slug: &str,
    anchor: (&str, Uuid),
    range: (i64, i64),
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    admissible(conn, id, Some(slug), (None, None), period).await?;
    let (from, to) = bounds(period);
    sqlx::query(
        "INSERT INTO vlan.vlan_group (id, slug, anchor_kind, anchor_id, vid_min, vid_max,
                                      valid_period, changeset_id)
         VALUES ($1, $2, $3, $4, $5, $6, tstzrange(coalesce($7::timestamptz, now()), $8), $9)",
    )
    .bind(id)
    .bind(slug)
    .bind(anchor.0)
    .bind(anchor.1)
    .bind(i16::try_from(range.0).unwrap_or(i16::MAX))
    .bind(i16::try_from(range.1).unwrap_or(i16::MAX))
    .bind(from)
    .bind(to)
    .bind(changeset)
    .execute(conn)
    .await
    .map(drop)
    .map_err(|e| written(&e, id))
}

/// Applies `patch` (columns of the group and their values) to `id` over `period` ("FOR
/// PORTION OF", migration 0018 of the core).
pub(crate) async fn patch_group(
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
    let (from, to) = bounds(period);
    let versions: i32 = sqlx::query_scalar(
        "SELECT public.kartenn_portion('vlan.vlan_group'::regclass, $1,
                                       coalesce($2::timestamptz, now()), $3, $4::jsonb, $5)",
    )
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
pub(crate) async fn excluded_vlans(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, i16, String)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT g.id, v.vid, v.name
           FROM vlan.vlan_group g
           JOIN vlan.vlan v ON v.group_id = g.id AND v.valid_period && g.valid_period
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
pub(crate) async fn dead_anchors(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT g.id, g.anchor_kind, g.anchor_id
           FROM vlan.vlan_group g
          WHERE g.id = ANY($1)
            AND NOT EXISTS (
                SELECT 1 FROM (
                    SELECT id, valid_period FROM public.entity WHERE g.anchor_kind = 'entity'
                    UNION ALL
                    SELECT id, valid_period FROM public.site WHERE g.anchor_kind = 'site'
                    UNION ALL
                    SELECT id, valid_period FROM public.addressing_domain
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
pub(crate) async fn vlan_group(
    conn: &mut PgConnection,
    id: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT group_id FROM vlan.vlan
          WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(id)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// The domain of the group `id` at `at`, when it is anchored on one.
pub(crate) async fn group_domain(
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
           EXISTS (SELECT 1 FROM me JOIN vlan.vlan o
                     ON o.id <> $1 AND o.group_id = me.g AND o.vid = me.vid
                    AND o.valid_period && me.p),
           EXISTS (SELECT 1 FROM me JOIN vlan.vlan o
                     ON o.id <> $1 AND o.group_id = me.g AND o.name = me.name
                    AND o.valid_period && me.p),
           EXISTS (SELECT 1 FROM me
                    WHERE NOT coalesce((SELECT range_agg(g.valid_period) @> me.p
                                               AND bool_and(me.vid BETWEEN g.vid_min AND g.vid_max)
                                          FROM vlan.vlan_group g
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
                   FROM vlan.vlan v
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
            &crate::rules::VID_OUT_OF_RANGE,
            "the VID of the VLAN is outside the range of its group, or the group does not \
             live over the period",
        );
    }
    if vid_taken {
        return refused(
            &crate::rules::VID_DUPLICATE,
            "the VID of the VLAN is taken in its group over the period",
        );
    }
    if name_taken {
        return refused(
            &crate::rules::NAME_DUPLICATE,
            "the name of the VLAN is taken in its group over the period",
        );
    }
    Ok(())
}

/// A new VLAN over `period`, from its planned fields (`group_id`, `vid`, `name`,
/// `status`).
pub(crate) async fn insert_vlan(
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
        "INSERT INTO vlan.vlan (id, group_id, vid, name, status, valid_period, changeset_id)
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
pub(crate) async fn patch_vlan(
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
    let (from, to) = bounds(period);
    let versions: i32 = sqlx::query_scalar(
        "SELECT public.kartenn_portion('vlan.vlan'::regclass, $1,
                                       coalesce($2::timestamptz, now()), $3, $4::jsonb, $5)",
    )
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

/// The prefixes among `ids` that carry `vlan.vlan` while not a network, on some part of
/// the period of the assignment (REQ-008-17).
pub(crate) async fn vlan_off_network(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT DISTINCT a.object_id
           FROM public.property_assignment a
           JOIN public.property_declaration d ON d.id = a.property_id AND d.key = 'vlan.vlan'
           JOIN public.prefix p ON p.id = a.object_id AND p.valid_period && a.valid_period
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
pub(crate) async fn networks_of(
    conn: &mut PgConnection,
    vlans: &[Uuid],
    groups: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    if vlans.is_empty() && groups.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT DISTINCT a.object_id
           FROM public.property_assignment a
           JOIN public.property_declaration d ON d.id = a.property_id AND d.key = 'vlan.vlan'
          WHERE a.kind = 'value'
            AND (upper(a.valid_period) IS NULL OR upper(a.valid_period) > now())
            AND (a.v_uuid = ANY($1)
                 OR a.v_uuid IN (SELECT id FROM vlan.vlan WHERE group_id = ANY($2)))",
    )
    .bind(vlans)
    .bind(groups)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The objects that carry an open anomaly of `rule`: evaluated again at each changeset, so
/// that an anomaly closes once its cause is gone, whatever the changeset touched.
pub(crate) async fn open_anomalies(
    conn: &mut PgConnection,
    rule: &str,
) -> Result<Vec<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT object_id FROM public.anomaly WHERE rule_id = $1 AND closed_at IS NULL",
    )
    .bind(rule)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The networks among `ids` whose VLAN is in a group anchored on a site outside the site
/// of the network (`site`) or on another domain (`domain`), V9: each assignment of
/// `vlan.vlan` is read at its start, or now when it started before.
pub(crate) async fn mismatches(
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
               FROM public.property_assignment a
               JOIN public.property_declaration d
                 ON d.id = a.property_id AND d.key = 'vlan.vlan'
              WHERE a.object_id = ANY($1) AND a.kind = 'value'
                AND (upper(a.valid_period) IS NULL OR upper(a.valid_period) > now())
         ), anchored AS (
             SELECT l.network, l.t, g.anchor_kind, g.anchor_id
               FROM links l
               JOIN vlan.vlan v ON v.id = l.vlan AND v.valid_period @> l.t
               JOIN vlan.vlan_group g ON g.id = v.group_id AND g.valid_period @> l.t
         )
         SELECT x.network, 'site'
           FROM anchored x
          CROSS JOIN LATERAL (SELECT public.kartenn_site(x.network, x.t) AS site) s
          WHERE x.anchor_kind = 'site' AND s.site IS NOT NULL
            AND NOT EXISTS (
                WITH RECURSIVE up(id) AS (
                    SELECT x.anchor_id
                    UNION
                    SELECT st.parent_id FROM public.site st JOIN up ON st.id = up.id
                     WHERE st.valid_period @> x.t AND st.parent_id IS NOT NULL)
                SELECT 1 FROM up WHERE up.id = s.site)
         UNION
         SELECT x.network, 'domain'
           FROM anchored x
           JOIN public.prefix p ON p.id = x.network AND p.valid_period @> x.t
          WHERE x.anchor_kind = 'domain' AND p.domain_id <> x.anchor_id
         ORDER BY 1, 2",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}
