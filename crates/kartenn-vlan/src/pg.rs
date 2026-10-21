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
