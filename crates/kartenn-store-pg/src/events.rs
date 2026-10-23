//! The reads of the shared reader of the event stream (spec 022, ADR 0034): a volatile
//! view of the outbox, read above a floor kept in memory, without writing anything
//! (`outbox_delivery` and `pass_floor` belong to the consumers of ADR 0018).
//!
//! The floor rises as the floor of a pass does (migration 0027, [`crate::floor`]): every
//! number up to the greatest one seen at a pass is final once the oldest running
//! transaction is past the `pg_snapshot_xmax` noted at that pass; the first number not
//! read since, or that greatest one, is the new floor. A long transaction delays the
//! rise, never a delivery (E6, REQ-022-7).

use std::collections::{BTreeSet, HashMap};

use kartenn_service::StoreError;
use sqlx::PgConnection;
use sqlx::types::Uuid;

use crate::outbox::Event;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// The floor of a reader, in memory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Floor {
    /// Every number up to it was read, or will never exist.
    pub floor: i64,
    /// The greatest number at the pass that noted [`Self::pending_xmax`].
    pending_max: Option<i64>,
    /// `pg_snapshot_xmax` at that pass.
    pending_xmax: Option<i64>,
}

impl Floor {
    /// A floor at `floor`, nothing pending.
    #[must_use]
    pub fn at(floor: i64) -> Self {
        Self {
            floor,
            pending_max: None,
            pending_xmax: None,
        }
    }
}

/// Raises `floor` when it safely can, `seen` holding the numbers read above it (those at
/// or below the new floor are dropped from it).
///
/// # Errors
///
/// Database errors.
pub async fn raise(
    conn: &mut PgConnection,
    floor: &mut Floor,
    seen: &mut BTreeSet<i64>,
) -> Result<(), StoreError> {
    let (xmin, xmax, max): (i64, i64, Option<i64>) = sqlx::query_as(
        "SELECT pg_snapshot_xmin(pg_current_snapshot())::text::bigint,
                pg_snapshot_xmax(pg_current_snapshot())::text::bigint,
                (SELECT max(seq) FROM outbox)",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(store_err)?;
    let settled = floor.pending_xmax.is_some_and(|x| xmin >= x);
    if let (true, Some(pending)) = (settled, floor.pending_max) {
        let above: Vec<i64> = seen.iter().copied().collect();
        // The first number not read up to `pending`: final, it will come at the next
        // read; the new floor is just below it.
        let first: Option<i64> = sqlx::query_scalar(
            "SELECT min(seq) FROM outbox WHERE seq > $1 AND seq <= $2 AND seq <> ALL($3)",
        )
        .bind(floor.floor)
        .bind(pending)
        .bind(&above)
        .fetch_one(&mut *conn)
        .await
        .map_err(store_err)?;
        floor.floor = floor.floor.max(first.map_or(pending, |f| f - 1));
        let kept = seen.split_off(&(floor.floor + 1));
        *seen = kept;
    }
    if settled || floor.pending_max.is_none() {
        floor.pending_max = max;
        floor.pending_xmax = max.map(|_| xmax);
    }
    Ok(())
}

type Row = (i64, Option<Uuid>, Vec<Uuid>, Vec<String>, serde_json::Value);

fn event((seq, changeset, domains, order_keys, payload): Row) -> Event {
    Event {
        seq,
        changeset,
        domains,
        order_keys,
        payload,
    }
}

/// The committed events above `floor` not in `seen`, by number, at most `limit`.
///
/// # Errors
///
/// Database errors.
pub async fn read(
    conn: &mut PgConnection,
    floor: i64,
    seen: &BTreeSet<i64>,
    limit: i64,
) -> Result<Vec<Event>, StoreError> {
    let above: Vec<i64> = seen.iter().copied().collect();
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT seq, changeset_id, domain_ids, order_keys, payload FROM outbox
         WHERE seq > $1 AND seq <> ALL($2) ORDER BY seq LIMIT $3",
    )
    .bind(floor)
    .bind(&above)
    .bind(limit)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    Ok(rows.into_iter().map(event).collect())
}

/// The events between `low` (excluded) and `high` (included) not in `seen`: the sweep of
/// the window of the ring, for a number drawn before its transaction had an identifier.
///
/// # Errors
///
/// Database errors.
pub async fn sweep(
    conn: &mut PgConnection,
    low: i64,
    high: i64,
    seen: &BTreeSet<i64>,
) -> Result<Vec<Event>, StoreError> {
    let known: Vec<i64> = seen.range(low + 1..=high).copied().collect();
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT seq, changeset_id, domain_ids, order_keys, payload FROM outbox
         WHERE seq > $1 AND seq <= $2 AND seq <> ALL($3) ORDER BY seq",
    )
    .bind(low)
    .bind(high)
    .bind(&known)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    Ok(rows.into_iter().map(event).collect())
}

/// The events of the last `seconds`, at most `limit` (the latest), by number: the ring
/// reloaded at the start (REQ-022-7); and the floor to start from: just below the first
/// of them, or the greatest number of the outbox without any.
///
/// # Errors
///
/// Database errors.
pub async fn recent(
    conn: &mut PgConnection,
    seconds: i64,
    limit: i64,
) -> Result<(Vec<Event>, Floor), StoreError> {
    // The latest `limit` by the primary key, then those of the window: the outbox has no
    // index on the time of its events.
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT seq, changeset_id, domain_ids, order_keys, payload FROM (
             SELECT seq, changeset_id, domain_ids, order_keys, payload FROM outbox
             ORDER BY seq DESC LIMIT $2) latest
         WHERE (payload ->> 'time')::timestamptz > now() - $1::bigint * interval '1 second'
         ORDER BY seq",
    )
    .bind(seconds)
    .bind(limit)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    let start = match rows.first() {
        Some((seq, ..)) => seq - 1,
        None => sqlx::query_scalar::<_, Option<i64>>("SELECT max(seq) FROM outbox")
            .fetch_one(&mut *conn)
            .await
            .map_err(store_err)?
            .unwrap_or(0),
    };
    Ok((rows.into_iter().map(event).collect(), Floor::at(start)))
}

/// The kind of each object among `ids` in `carrier_object` (`prefix`, `vlan.vlan`…),
/// whatever its validity: one query for a pass (design « Format »).
///
/// # Errors
///
/// Database errors.
pub async fn kinds(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, String>, StoreError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT DISTINCT ON (c.id) c.id, c.kind
         FROM unnest($1::uuid[]) AS o(id) JOIN carrier_object c ON c.id = o.id",
    )
    .bind(ids)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    Ok(rows.into_iter().collect())
}

/// Which subjects read which objects now (REQ-022-16; design « Visibilité par lot »): one
/// query for every subject and object of a batch, the predicates of the RBAC (ADR 0021)
/// joined subjects × objects. `scopes` holds, per subject (its index), the JSON of its
/// read scope (`ReadScope::to_json`) without the kinds it reads everywhere, which the
/// caller decides without the base. An object no longer valid is judged on its last
/// version, one to come on its first.
///
/// # Errors
///
/// Database errors.
pub async fn visible(
    conn: &mut PgConnection,
    scopes: &[(i32, serde_json::Value)],
    objects: &[Uuid],
) -> Result<Vec<(i32, Uuid)>, StoreError> {
    if scopes.is_empty() || objects.is_empty() {
        return Ok(Vec::new());
    }
    let (subjects, jsons): (Vec<i32>, Vec<serde_json::Value>) = scopes.iter().cloned().unzip();
    sqlx::query_as(
        "WITH s AS MATERIALIZED (
             SELECT t.n, k.* FROM unnest($1::int[], $2::jsonb[]) AS t(n, scope),
                  LATERAL kartenn_scopes(t.scope) k),
         o AS MATERIALIZED (
             SELECT DISTINCT ON (c.id) c.id, c.kind, c.domain_id, c.value,
                    CASE WHEN c.valid_period @> now() THEN now()
                         WHEN upper(c.valid_period) <= now()
                             THEN upper(c.valid_period) - interval '1 microsecond'
                         ELSE lower(c.valid_period) END AS at
             FROM unnest($3::uuid[]) AS x(id) JOIN carrier_object c ON c.id = x.id
             ORDER BY c.id, c.valid_period @> now() DESC,
                      upper(c.valid_period) DESC NULLS FIRST)
         SELECT s.n, o.id FROM o JOIN s ON s.kind = o.kind
         WHERE kartenn_visible(o.id, o.domain_id, o.value, o.at, s.global, s.entities,
                               s.sites, s.domains, s.prefix_domains, s.prefixes,
                               s.confinement)",
    )
    .bind(&subjects)
    .bind(&jsons)
    .bind(objects)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)
}

/// The state of each identity among `ids` (REQ-005-3): disabled, and its sessions
/// revoked after (Unix seconds); an identity absent is not returned.
///
/// # Errors
///
/// Database errors.
pub async fn states(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, bool, Option<i64>)>, StoreError> {
    sqlx::query_as(
        "SELECT id, disabled_at IS NOT NULL, floor(extract(epoch FROM revoked_after))::bigint
         FROM person WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)
}

/// Whether one of `ids` (the objects of applied changesets) is an object of the
/// administration of the rights: a binding, an identity (REQ-005-25).
///
/// # Errors
///
/// Database errors.
pub async fn administrative(conn: &mut PgConnection, ids: &[Uuid]) -> Result<bool, StoreError> {
    if ids.is_empty() {
        return Ok(false);
    }
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM binding WHERE id = ANY($1))
             OR EXISTS (SELECT 1 FROM person WHERE id = ANY($1))",
    )
    .bind(ids)
    .fetch_one(&mut *conn)
    .await
    .map_err(store_err)
}
