//! Explained resolution of a property (spec 003, REQ-003-14, 15; task 003-14).
//!
//! At any `(valid_at, as_of)`: the resolved value, the object carrying the applicable
//! assignment (the materialised origin), the chain of objects walked from the object up
//! to it in the tree of that instant (addressing, entities or sites), the provenance of
//! the assignment and, when it overrides one, the inherited value it replaces.

use std::collections::HashMap;

use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{AssertSqlSafe, PgPool};

use crate::diff::value_json;
use crate::temporal::{TemporalError, snapshot};

/// The resolution of one property for one object, explained (REQ-003-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    /// Property key.
    pub property: String,
    /// Resolved value as JSON (closed-list keys); `None` when absent or "no value".
    pub value: Option<String>,
    /// Object carrying the applicable assignment; `None` when nothing applies.
    pub origin: Option<Uuid>,
    /// Objects walked from the object to its origin, both included (the object alone
    /// when nothing applies).
    pub chain: Vec<Uuid>,
    /// Provenance of the applicable assignment: `input` or `source:<id>@<version>`.
    pub provenance: Option<String>,
    /// The inherited value the applicable assignment replaces: its origin and value.
    pub replaces: Option<(Uuid, Option<String>)>,
}

fn sql_err(e: &TemporalError) -> sqlx::Error {
    sqlx::Error::Protocol(e.to_string())
}

/// Ancestors of `$3` at `($1, $2)`, nearest first, then its domain: the prefixes of its
/// domain that contain it (strictly for a prefix; from its network up for a range or an
/// address). Empty for a domain.
fn ancestors_sql() -> Result<String, TemporalError> {
    let prefix = snapshot("prefix", &["id", "domain_id", "prefix"])?;
    let range = snapshot("ip_range", &["id", "domain_id", "range"])?;
    let address = snapshot("address", &["id", "domain_id", "address"])?;
    Ok(format!(
        "WITH o AS (
             SELECT id, domain_id, prefix::inet AS v FROM {prefix} x WHERE id = $3
             UNION ALL SELECT id, domain_id, lower(range) FROM {range} x WHERE id = $3
             UNION ALL SELECT id, domain_id, address FROM {address} x WHERE id = $3)
         SELECT id FROM (
             SELECT p.id, masklen(p.prefix) AS depth FROM {prefix} p, o
             WHERE p.domain_id = o.domain_id
               AND p.prefix >>= o.v AND p.id <> o.id
             UNION ALL SELECT o.domain_id, -1 FROM o) a
         ORDER BY depth DESC"
    ))
}

/// Ancestors of `$3` at `($1, $2)` in an entity or site `table`, parent first (003-18).
fn anchor_ancestors_sql(table: &str) -> Result<String, TemporalError> {
    let anchor = snapshot(table, &["id", "parent_id"])?;
    Ok(format!(
        "WITH RECURSIVE c(id, depth) AS (
             SELECT parent_id, 1 FROM {anchor} x WHERE id = $3 AND parent_id IS NOT NULL
             UNION ALL
             SELECT x.parent_id, c.depth + 1 FROM c JOIN {anchor} x ON x.id = c.id
             WHERE x.parent_id IS NOT NULL AND c.depth < 1000)
         SELECT id FROM c ORDER BY depth"
    ))
}

/// Explains the resolution of `property` for `object` at `valid_at`, as asserted at
/// `as_of` (now when `None`). `None` when the property is not declared then.
///
/// # Errors
///
/// Database errors.
pub async fn explain(
    pool: &PgPool,
    object: Uuid,
    property: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<Explanation>> {
    let declaration = snapshot(
        "property_declaration",
        &["id", "key", "value_type", "inherit_tree"],
    )
    .map_err(|e| sql_err(&e))?;
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(AssertSqlSafe(format!(
        "SELECT key, value_type, inherit_tree FROM {declaration} d WHERE id = $3"
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(property)
    .fetch_optional(pool)
    .await?;
    let Some((key, value_type, inherit)) = row else {
        return Ok(None);
    };
    let ancestors_of = match inherit.as_deref() {
        Some("entity") => Some(anchor_ancestors_sql("entity")),
        Some("site") => Some(anchor_ancestors_sql("site")),
        Some(_) => Some(ancestors_sql()),
        None => None,
    };
    let ancestors: Vec<Uuid> = if let Some(sql) = ancestors_of {
        sqlx::query_scalar(AssertSqlSafe(sql.map_err(|e| sql_err(&e))?))
            .bind(valid_at)
            .bind(as_of)
            .bind(object)
            .fetch_all(pool)
            .await?
    } else {
        Vec::new()
    };
    let origin: Option<Uuid> = if inherit.is_some() {
        let origins = snapshot(
            "property_origin",
            &["object_id", "property_id", "origin_object_id"],
        )
        .map_err(|e| sql_err(&e))?;
        sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT origin_object_id FROM {origins} o WHERE object_id = $3 AND property_id = $4"
        )))
        .bind(valid_at)
        .bind(as_of)
        .bind(object)
        .bind(property)
        .fetch_optional(pool)
        .await?
    } else {
        Some(object)
    };
    let closed_list = value_type == "enum";
    let assignment =
        |candidate: Uuid| assignment_at(pool, candidate, property, closed_list, valid_at, as_of);
    let candidates: Vec<Uuid> = std::iter::once(object).chain(ancestors).collect();
    let Some(origin) = origin else {
        return Ok(Some(Explanation {
            property: key,
            value: None,
            origin: None,
            chain: vec![object],
            provenance: None,
            replaces: None,
        }));
    };
    let Some((value, provenance)) = assignment(origin).await? else {
        // Non-heritable without an assignment of its own.
        return Ok(Some(Explanation {
            property: key,
            value: None,
            origin: None,
            chain: vec![object],
            provenance: None,
            replaces: None,
        }));
    };
    let depth = candidates.iter().position(|c| *c == origin).unwrap_or(0);
    let mut replaces = None;
    for candidate in candidates.iter().skip(depth + 1) {
        if let Some((value, _)) = assignment(*candidate).await? {
            replaces = Some((*candidate, value));
            break;
        }
    }
    Ok(Some(Explanation {
        property: key,
        value,
        origin: Some(origin),
        chain: candidates[..=depth].to_vec(),
        provenance: Some(provenance),
        replaces,
    }))
}

/// The assignment of a property on an object at `(valid_at, as_of)`: its value as JSON
/// (`None` for "no value") and its provenance.
async fn assignment_at(
    pool: &PgPool,
    object: Uuid,
    property: Uuid,
    closed_list: bool,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<(Option<String>, String)>> {
    let assignments = snapshot(
        "property_assignment",
        &[
            "object_id",
            "property_id",
            "kind",
            "v_uuid",
            "v_uuids",
            "v_text",
            "v_texts",
            "v_int",
            "v_ints",
            "v_bool",
            "v_date",
            "v_dates",
            "v_json",
            "v_jsons",
            "provenance",
        ],
    )
    .map_err(|e| sql_err(&e))?;
    let value = value_json(if closed_list { "true" } else { "false" });
    sqlx::query_as(AssertSqlSafe(format!(
        "SELECT ({value})::text, a.provenance FROM {assignments} a
         WHERE a.object_id = $3 AND a.property_id = $4"
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(object)
    .bind(property)
    .fetch_optional(pool)
    .await
}

/// The resolution of one property for one object among many (REQ-016-1): as
/// [`Explanation`], without the value replaced, with the kind of the object and whether
/// the property is sensitive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// The object.
    pub object: Uuid,
    /// Its kind (`domain`…, or a qualified type of a module).
    pub kind: String,
    /// Property key.
    pub property: String,
    /// Whether the property is declared sensitive (REQ-003-27).
    pub sensitive: bool,
    /// Resolved value as JSON (closed-list keys); `None` when absent or "no value".
    pub value: Option<String>,
    /// Object carrying the applicable assignment; `None` when nothing applies.
    pub origin: Option<Uuid>,
    /// Objects walked from the object to its origin, both included.
    pub chain: Vec<Uuid>,
    /// Provenance of the applicable assignment.
    pub provenance: Option<String>,
}

/// One row of the grouped read: object, kind, key, tree, sensitive, origin, value,
/// provenance.
type ResolutionRow = (
    Uuid,
    String,
    String,
    Option<String>,
    bool,
    Option<Uuid>,
    Option<String>,
    Option<String>,
);

/// The kind of the object `o.object` at `($1, $2)`, for a lateral join: one probe per
/// table until found. The keys of the tables are `GiST`: one equality probe per object
/// costs far less than an `= ANY` over hundreds of them (bench at 10⁶, REQ-016-1).
fn kind_sql() -> Result<String, TemporalError> {
    let kind = |table: &str, kind: &str| {
        snapshot(table, &["id"])
            .map(|s| format!("SELECT '{kind}'::text AS kind FROM {s} x WHERE x.id = o.object"))
    };
    let modules = snapshot("module_object", &["id", "kind"])?;
    let union = [
        kind("address", "address")?,
        kind("prefix", "prefix")?,
        kind("ip_range", "range")?,
        kind("addressing_domain", "domain")?,
        kind("entity", "entity")?,
        kind("site", "site")?,
        format!("SELECT x.kind FROM {modules} x WHERE x.id = o.object"),
    ]
    .join(" UNION ALL ");
    Ok(format!("SELECT kind FROM ({union}) z LIMIT 1"))
}

/// Ancestors of each of the objects `$3` at `($1, $2)` along the addressing tree, nearest
/// first, then the domain (as [`ancestors_sql`] for one object): rows of (object,
/// ancestor). The containing prefixes are read object by object, through the index of
/// their domain (at most one per mask length: the limit keeps the subquery a lateral
/// probe instead of a join over every prefix of the domain).
fn ancestors_many_sql() -> Result<String, TemporalError> {
    let prefix = snapshot("prefix", &["id", "domain_id", "prefix"])?;
    let range = snapshot("ip_range", &["id", "domain_id", "range"])?;
    let address = snapshot("address", &["id", "domain_id", "address"])?;
    Ok(format!(
        "WITH o AS MATERIALIZED (
             SELECT u.id, l.domain_id, l.v FROM unnest($3::uuid[]) u(id)
             CROSS JOIN LATERAL (
                 SELECT domain_id, address AS v FROM {address} x WHERE x.id = u.id
                 UNION ALL SELECT domain_id, prefix::inet FROM {prefix} x WHERE x.id = u.id
                 UNION ALL SELECT domain_id, lower(range) FROM {range} x WHERE x.id = u.id
                 LIMIT 1) l)
         SELECT object, id FROM (
             SELECT o.id AS object, p.id, p.depth FROM o
             CROSS JOIN LATERAL (
                 SELECT p.id, masklen(p.prefix) AS depth FROM {prefix} p
                 WHERE p.domain_id = o.domain_id AND p.prefix >>= o.v AND p.id <> o.id
                 LIMIT 129) p
             UNION ALL SELECT o.id, o.domain_id, -1 FROM o) a
         ORDER BY object, depth DESC"
    ))
}

/// Ancestors of each of the objects `$3` at `($1, $2)` in an entity or site `table`,
/// parent first: rows of (object, ancestor).
fn anchor_ancestors_many_sql(table: &str) -> Result<String, TemporalError> {
    let anchor = snapshot(table, &["id", "parent_id"])?;
    Ok(format!(
        "WITH RECURSIVE c(object, id, depth) AS (
             SELECT u.id, l.parent_id, 1 FROM unnest($3::uuid[]) u(id)
             CROSS JOIN LATERAL (SELECT parent_id FROM {anchor} x
                                 WHERE x.id = u.id AND x.parent_id IS NOT NULL) l
             UNION ALL
             SELECT c.object, x.parent_id, c.depth + 1 FROM c JOIN {anchor} x ON x.id = c.id
             WHERE x.parent_id IS NOT NULL AND c.depth < 1000)
         SELECT object, id FROM c ORDER BY object, depth"
    ))
}

/// The tree along which the chain of a resolution is read, as [`explain`] reads it: the
/// entity or site tree, else the addressing tree.
fn chain_tree(inherit: Option<&str>) -> &'static str {
    match inherit {
        Some("entity") => "entity",
        Some("site") => "site",
        _ => "addressing",
    }
}

/// The ancestors of `objects` along `tree`, by object, nearest first.
async fn ancestors_along(
    pool: &PgPool,
    tree: &'static str,
    objects: &[Uuid],
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<HashMap<Uuid, Vec<Uuid>>> {
    let sql = match tree {
        "addressing" => ancestors_many_sql(),
        anchor => anchor_ancestors_many_sql(anchor),
    }
    .map_err(|e| sql_err(&e))?;
    let pairs: Vec<(Uuid, Uuid)> = sqlx::query_as(AssertSqlSafe(sql))
        .persistent(false)
        .bind(valid_at)
        .bind(as_of)
        .bind(objects)
        .fetch_all(pool)
        .await?;
    let mut ancestors: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for (object, ancestor) in pairs {
        ancestors.entry(object).or_default().push(ancestor);
    }
    Ok(ancestors)
}

/// The values and origins of the properties `$4` of the objects `$3` at `($1, $2)`: rows
/// of [`ResolutionRow`] (see [`resolved_many`]).
fn resolution_sql() -> Result<String, TemporalError> {
    let snap = snapshot;
    let declarations = snap(
        "property_declaration",
        &["id", "key", "value_type", "inherit_tree", "sensitive"],
    )?;
    let origins = snap(
        "property_origin",
        &["object_id", "property_id", "origin_object_id"],
    )?;
    let assignments = snap(
        "property_assignment",
        &[
            "object_id",
            "property_id",
            "kind",
            "v_uuid",
            "v_uuids",
            "v_text",
            "v_texts",
            "v_int",
            "v_ints",
            "v_bool",
            "v_date",
            "v_dates",
            "v_json",
            "v_jsons",
            "provenance",
        ],
    )?;
    let kind = kind_sql()?;
    let value = value_json("s.value_type = 'enum'");
    // Each part narrowed to the objects and keys asked, then joined; planned with its
    // values at each call (a generic plan of the dated snapshots scans whole tables). The
    // objects are read by equality, one probe each (see `kind_sql`); their origins and the
    // assignments through the btree indexes by object and property.
    Ok(format!(
        "WITH d AS MATERIALIZED (
                  SELECT id, key, value_type, inherit_tree, sensitive
                  FROM {declarations} x WHERE key = ANY($4)),
              o AS MATERIALIZED (
                  SELECT u.id AS object, u.n FROM unnest($3::uuid[]) WITH ORDINALITY u(id, n)),
              k AS MATERIALIZED (
                  SELECT o.object AS id, l.kind FROM o CROSS JOIN LATERAL ({kind}) l),
              p AS MATERIALIZED (
                  SELECT x.object_id, x.property_id, x.origin_object_id FROM {origins} x
                  WHERE x.object_id = ANY($3)
                    AND x.property_id = ANY(ARRAY(SELECT id FROM d
                                                  WHERE inherit_tree IS NOT NULL))),
              s AS MATERIALIZED (
                  SELECT o.object, o.n, d.id AS property, d.key, d.value_type, d.inherit_tree,
                         d.sensitive,
                         CASE WHEN d.inherit_tree IS NULL THEN o.object
                              ELSE p.origin_object_id END AS origin
                  FROM o JOIN k ON k.id = o.object CROSS JOIN d
                  LEFT JOIN p ON p.object_id = o.object AND p.property_id = d.id),
              a AS MATERIALIZED (
                  SELECT * FROM {assignments} x
                  WHERE object_id = ANY(ARRAY(SELECT DISTINCT origin FROM s WHERE origin IS NOT NULL))
                    AND property_id = ANY(ARRAY(SELECT id FROM d)))
         SELECT s.object, k.kind, s.key, s.inherit_tree, s.sensitive,
                CASE WHEN a.object_id IS NULL THEN NULL ELSE s.origin END,
                ({value})::text, a.provenance
         FROM s JOIN k ON k.id = s.object
         LEFT JOIN a ON a.object_id = s.origin AND a.property_id = s.property
         ORDER BY s.n, array_position($4::text[], s.key)"
    ))
}

/// Resolves the properties `keys` for the `objects` at `valid_at`, as asserted at `as_of`
/// (now when `None`), explained as [`explain`] does one by one (REQ-016-1): one row per
/// existing object and declared key, in the order of `objects` then `keys`. Objects that
/// do not exist then and keys not declared then are left out; nothing is filtered by
/// rights (the service does, REQ-005-24).
///
/// # Errors
///
/// Database errors.
pub async fn resolved_many(
    pool: &PgPool,
    objects: &[Uuid],
    keys: &[String],
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Vec<Resolution>> {
    let declarations =
        snapshot("property_declaration", &["key", "inherit_tree"]).map_err(|e| sql_err(&e))?;
    let trees: Vec<Option<String>> = sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT DISTINCT inherit_tree FROM {declarations} x WHERE key = ANY($3)"
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(keys)
    .fetch_all(pool)
    .await?;
    // A few index probes per object and key: the objects are read in parts, side by side
    // on the pool, so that the latency is that of a part (bench, REQ-016-1).
    let size = objects.len().div_ceil(PARTS).max(PART_MIN);
    let parts: Vec<&[Uuid]> = objects.chunks(size).collect();
    let part = |i: usize| {
        let objects = parts.get(i).copied().unwrap_or_default();
        resolved_part(pool, objects, keys, &trees, valid_at, as_of)
    };
    let (a, b, c, d) = tokio::try_join!(part(0), part(1), part(2), part(3))?;
    Ok([a, b, c, d].concat())
}

/// The number of parts of a grouped resolution read side by side, and the least number of
/// objects in a part.
const PARTS: usize = 4;
const PART_MIN: usize = 64;

/// [`resolved_many`] for one part of the objects, given the trees of the `keys`.
async fn resolved_part(
    pool: &PgPool,
    objects: &[Uuid],
    keys: &[String],
    trees: &[Option<String>],
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Vec<Resolution>> {
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    let sql = resolution_sql().map_err(|e| sql_err(&e))?;
    let rows = sqlx::query_as::<_, ResolutionRow>(AssertSqlSafe(sql))
        .persistent(false)
        .bind(valid_at)
        .bind(as_of)
        .bind(objects)
        .bind(keys)
        .fetch_all(pool);
    // The chains along the trees of the keys, read beside the values: the latency is that
    // of the slowest query, not their sum.
    let along = |tree: &'static str| {
        let wanted = trees
            .iter()
            .any(|t| t.is_some() && chain_tree(t.as_deref()) == tree);
        async move {
            if wanted {
                ancestors_along(pool, tree, objects, valid_at, as_of).await
            } else {
                Ok(HashMap::new())
            }
        }
    };
    let (rows, entity, site, addressing) =
        tokio::try_join!(rows, along("entity"), along("site"), along("addressing"),)?;
    Ok(rows
        .into_iter()
        .map(
            |(object, kind, property, tree, sensitive, origin, value, provenance)| {
                let along = match chain_tree(tree.as_deref()) {
                    "entity" => &entity,
                    "site" => &site,
                    _ => &addressing,
                };
                let candidates: Vec<Uuid> = std::iter::once(object)
                    .chain(along.get(&object).into_iter().flatten().copied())
                    .collect();
                let depth = origin
                    .and_then(|o| candidates.iter().position(|c| *c == o))
                    .unwrap_or(0);
                Resolution {
                    object,
                    kind,
                    property,
                    sensitive,
                    value: origin.and(value),
                    origin,
                    chain: candidates[..=depth].to_vec(),
                    provenance: origin.and(provenance),
                }
            },
        )
        .collect())
}
