//! RBAC in PostgreSQL (spec 005, REQ-005-18 to 20, 23; ADR 0021): the visibility of a
//! subject for a permission, computed from its bindings, and the decision of
//! [`Authorizer::authorize_many`] by the predicate `kartenn_visible` (migration 0029),
//! which also filters the lists.

use kartenn_core::addr::Prefix;
use kartenn_service::StoreError;
use kartenn_service::authz::{Authorizer, Subject};
use kartenn_service::module::ModuleRegistry;
use std::future::Future;
use std::sync::Arc;

use kartenn_service::WriteTx;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgConnection, PgPool};

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Where a subject holds a permission (ADR 0021): the union of the scopes of its bindings,
/// bounded by the confinement of its issuer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Visibility {
    /// Everywhere.
    pub global: bool,
    /// Entities, with their descendants: the objects they own.
    pub entities: Vec<Uuid>,
    /// Sites, with their descendants.
    pub sites: Vec<Uuid>,
    /// Domains: their addressing objects.
    pub domains: Vec<Uuid>,
    /// Prefix subtrees: (domain, prefix in CIDR text).
    pub prefixes: Vec<(Uuid, String)>,
    /// The entities (with descendants) the issuer of the subject may reach; `None`: no
    /// bound (a global attachment, a service or break-glass account).
    pub confinement: Option<Vec<Uuid>>,
}

impl Visibility {
    /// Grants nothing.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Whether it grants nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.global
            && self.entities.is_empty()
            && self.sites.is_empty()
            && self.domains.is_empty()
            && self.prefixes.is_empty()
    }

    /// Without the scopes that others already cover, same meaning (bench 005-16): with
    /// `global`, nothing else; a prefix subtree of a granted domain, or inside another
    /// granted subtree of its domain, goes; duplicates go.
    pub fn normalize(&mut self) {
        if self.global {
            self.entities.clear();
            self.sites.clear();
            self.domains.clear();
            self.prefixes.clear();
            return;
        }
        for list in [&mut self.entities, &mut self.sites, &mut self.domains] {
            list.sort_unstable();
            list.dedup();
        }
        let parse = |text: &str| Prefix::parse_any(&kartenn_core::family::ALL, text).ok();
        let mut kept: Vec<(Uuid, String, Option<Prefix>)> = Vec::new();
        let mut candidates: Vec<(Uuid, String, Option<Prefix>)> =
            std::mem::take(&mut self.prefixes)
                .into_iter()
                .filter(|(domain, _)| self.domains.binary_search(domain).is_err())
                .map(|(domain, text)| {
                    let parsed = parse(&text);
                    (domain, text, parsed)
                })
                .collect();
        // The widest first: a subtree is dropped when a kept one contains it.
        candidates
            .sort_by_key(|(domain, text, p)| (*domain, p.map(Prefix::prefix_len), text.clone()));
        for (domain, text, prefix) in candidates {
            let covered = kept.iter().any(|(d, t, p)| {
                *d == domain
                    && (*t == text
                        || matches!((p, prefix), (Some(outer), Some(inner)) if outer.contains(inner)))
            });
            if !covered {
                kept.push((domain, text, prefix));
            }
        }
        self.prefixes = kept.into_iter().map(|(d, t, _)| (d, t)).collect();
    }

    /// The arguments of `kartenn_visible` after the object, its domain, its value and the
    /// date, in order.
    #[must_use]
    pub fn arguments(&self) -> VisibilityArguments {
        let (prefix_domains, prefixes) = self.prefixes.iter().cloned().unzip();
        VisibilityArguments {
            global: self.global,
            entities: self.entities.clone(),
            sites: self.sites.clone(),
            domains: self.domains.clone(),
            prefix_domains,
            prefixes,
            confinement: self.confinement.clone(),
        }
    }
}

/// The arrays a query binds to call `kartenn_visible` (`$prefixes` as `text[]`, cast to
/// `cidr[]` in SQL).
#[derive(Debug, Clone)]
pub struct VisibilityArguments {
    /// See [`Visibility::global`].
    pub global: bool,
    /// See [`Visibility::entities`].
    pub entities: Vec<Uuid>,
    /// See [`Visibility::sites`].
    pub sites: Vec<Uuid>,
    /// See [`Visibility::domains`].
    pub domains: Vec<Uuid>,
    /// Domains of the prefix subtrees.
    pub prefix_domains: Vec<Uuid>,
    /// Prefixes of the subtrees, CIDR text.
    pub prefixes: Vec<String>,
    /// See [`Visibility::confinement`].
    pub confinement: Option<Vec<Uuid>>,
}

impl Visibility {
    /// As a JSON object, for `kartenn_scopes` (migration 0033).
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        let (prefix_domains, prefixes): (Vec<Uuid>, Vec<String>) =
            self.prefixes.iter().cloned().unzip();
        serde_json::json!({
            "global": self.global,
            "entities": self.entities,
            "sites": self.sites,
            "domains": self.domains,
            "prefix_domains": prefix_domains,
            "prefixes": prefixes,
            "confinement": self.confinement,
        })
    }
}

/// The condition that the object `id` of `kind`, in `domain` at `value`, is readable at
/// `at` under the scope materialised as `s` by `kartenn_scopes` (migration 0033): the
/// arrays are built once per query, not per row. A scalar subquery, which the planner
/// keeps per candidate row: an `EXISTS` would be flattened into a semi-join that tests
/// every object of the base (bench 005-16).
#[must_use]
pub fn readable(id: &str, kind: &str, domain: &str, value: &str, at: &str) -> String {
    format!(
        "coalesce((SELECT bool_or(kartenn_visible({id}, {domain}, {value}, {at}, s.global,
                                                 s.entities, s.sites, s.domains, s.prefix_domains,
                                                 s.prefixes, s.confinement))
                   FROM s WHERE s.kind = {kind}), false)"
    )
}

/// The kinds of objects of the core, with their read permission.
pub const READ_PERMISSIONS: [(&str, &str); 6] = [
    ("entity", "core.entity.read"),
    ("site", "core.site.read"),
    ("domain", "core.domain.read"),
    ("prefix", "core.prefix.read"),
    ("range", "core.range.read"),
    ("address", "core.address.read"),
];

/// What a subject may read, kind by kind (REQ-005-24): the visibility of the read
/// permission of each kind; `None` reads everything (kartenn itself).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadScope(Option<std::collections::BTreeMap<&'static str, Visibility>>);

impl ReadScope {
    /// Everything: for kartenn's own work.
    #[must_use]
    pub const fn all() -> Self {
        Self(None)
    }

    /// The scope as the JSON argument of `kartenn_scopes`; `None` for everything.
    #[must_use]
    pub fn to_json(&self) -> Option<serde_json::Value> {
        self.0.as_ref().map(|kinds| {
            serde_json::Value::Object(
                kinds
                    .iter()
                    .filter(|(_, v)| !v.is_empty())
                    .map(|(k, v)| ((*k).to_owned(), v.to_json()))
                    .collect(),
            )
        })
    }
}

/// The authorizer of the RBAC of spec 005.
#[derive(Debug, Clone)]
pub struct PgAuthorizer {
    pool: PgPool,
    /// The modules of the build: the types of their objects are read under
    /// `<type>.read` (spec 008, REQ-008-6).
    modules: Arc<ModuleRegistry>,
}

/// Whether the property of a property permission is sensitive (003/P17): the patterns
/// `prop.*.<action>` spare it.
async fn sensitive(
    conn: &mut PgConnection,
    permission: &str,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    let Some(key) = permission
        .strip_prefix("prop.")
        .and_then(|rest| rest.rsplit_once('.'))
        .map(|(key, _)| key)
    else {
        return Ok(false);
    };
    let found: Option<bool> = sqlx::query_scalar(
        "SELECT sensitive FROM property_declaration WHERE key = $1 AND valid_period @> $2",
    )
    .bind(key)
    .bind(at)
    .fetch_optional(&mut *conn)
    .await
    .map_err(store_err)?;
    Ok(found.unwrap_or(false))
}

/// `ids` with their descendants in `table` (`entity` or `site`) at `at`.
pub(crate) async fn descendants(
    conn: &mut PgConnection,
    table: &str,
    ids: &[Uuid],
    at: OffsetDateTime,
) -> Result<Vec<Uuid>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "WITH RECURSIVE tree(id) AS (
             SELECT t.id FROM {table} t WHERE t.id = ANY($1) AND t.valid_period @> $2
             UNION
             SELECT c.id FROM {table} c JOIN tree ON c.parent_id = tree.id
             WHERE c.valid_period @> $2)
         SELECT id FROM tree"
    )))
    .bind(ids)
    .bind(at)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)
}

/// Where `subject` holds `permission` at `at`, as `conn` sees the data.
///
/// # Errors
///
/// Database errors.
pub async fn visibility_on(
    conn: &mut PgConnection,
    subject: &Subject,
    permission: &str,
    at: OffsetDateTime,
) -> Result<Visibility, StoreError> {
    Ok(visibilities_on(conn, subject, &[permission], at)
        .await?
        .pop()
        .unwrap_or_default())
}

/// A scope of the bindings of a subject: kind, identifier, the permissions granting it,
/// and the domain and value of a prefix subtree.
type ScopeRow = (
    String,
    Option<Uuid>,
    Vec<String>,
    Option<Uuid>,
    Option<String>,
);

/// Whether a binding row of `granted` grants `permission`: `*`, the permission itself, or
/// `prop.*.<action>` for a property that is not sensitive (`pattern`).
fn grants(granted: &str, permission: &str, pattern: bool) -> bool {
    granted == "*"
        || granted == permission
        || (pattern
            && granted.strip_prefix("prop.*.")
                == Some(permission.rsplit('.').next().unwrap_or_default()))
}

/// [`visibility_on`] for several permissions at once, in their order: the bindings, the
/// subtrees of entities and sites, the prefixes and the confinement read once for all
/// (a read scope asks for six; bench 005-16).
///
/// # Errors
///
/// Database errors.
pub async fn visibilities_on(
    conn: &mut PgConnection,
    subject: &Subject,
    permissions: &[&str],
    at: OffsetDateTime,
) -> Result<Vec<Visibility>, StoreError> {
    let mut patterns = Vec::with_capacity(permissions.len());
    for permission in permissions {
        patterns.push(permission.starts_with("prop.") && !sensitive(conn, permission, at).await?);
    }
    let wanted: Vec<&str> = permissions.to_vec();
    // One row per binding, with the permissions of its role that matter here and, for a
    // prefix subtree, its domain and value (none once the prefix is closed: it grants
    // nothing then). No sort: duplicates go in `Visibility::normalize`.
    let rows: Vec<ScopeRow> = sqlx::query_as(
        "WITH granting AS (
             SELECT rp.role_id, array_agg(rp.permission) AS permissions
             FROM role r JOIN role_permission rp ON rp.role_id = r.id AND rp.valid_period @> $2
             WHERE r.valid_period @> $2
               AND (rp.permission = '*' OR rp.permission = ANY($1)
                    OR ($6 AND rp.permission LIKE 'prop.*.%'))
             GROUP BY rp.role_id)
         SELECT b.scope_kind, b.scope_id, g.permissions, p.domain_id, p.prefix::text
         FROM binding b
         JOIN granting g ON g.role_id = b.role_id
         LEFT JOIN prefix p ON b.scope_kind = 'prefix' AND p.id = b.scope_id
                           AND p.valid_period @> $2
         WHERE b.valid_period @> $2
           AND ((b.subject_kind = 'identity' AND b.subject_id = $3)
                OR (b.subject_kind = 'group' AND b.issuer = $4 AND b.group_name = ANY($5))
                OR (b.subject_kind = 'issuer' AND b.issuer = $4))",
    )
    .bind(&wanted)
    .bind(at)
    .bind(subject.id)
    .bind(subject.issuer.as_deref())
    .bind(&subject.groups)
    .bind(patterns.iter().any(|p| *p))
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    // The scopes of each permission; a global-only one acts on the global scope alone
    // (K7, K11).
    let mut out: Vec<Visibility> = vec![Visibility::none(); permissions.len()];
    let mut trees: Vec<Vec<(&str, Uuid)>> = vec![Vec::new(); permissions.len()];
    for (k, permission) in permissions.iter().enumerate() {
        let global_only =
            kartenn_core::authz::permission(permission).is_some_and(|p| p.global_only);
        for (kind, id, granted, domain, prefix) in &rows {
            if !granted.iter().any(|g| grants(g, permission, patterns[k])) {
                continue;
            }
            match (kind.as_str(), id) {
                ("global", _) => out[k].global = true,
                (_, _) if global_only => {}
                ("domain", Some(id)) => out[k].domains.push(*id),
                ("prefix", Some(_)) => {
                    if let (Some(domain), Some(prefix)) = (domain, prefix) {
                        out[k].prefixes.push((*domain, prefix.clone()));
                    }
                }
                (kind @ ("entity" | "site"), Some(id)) => trees[k].push((kind, *id)),
                _ => {}
            }
        }
    }
    let of = |kind: &str| -> Vec<Uuid> {
        let mut ids: Vec<Uuid> = trees
            .iter()
            .flatten()
            .filter(|(k, _)| *k == kind)
            .map(|(_, id)| *id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };
    let entities = descendants_by_root(conn, "entity", &of("entity"), at).await?;
    let sites = descendants_by_root(conn, "site", &of("site"), at).await?;
    let confinement = confinement(conn, subject, at).await?;
    for (v, trees) in out.iter_mut().zip(trees) {
        for (kind, id) in trees {
            let found = if kind == "entity" { &entities } else { &sites };
            let list = if kind == "entity" {
                &mut v.entities
            } else {
                &mut v.sites
            };
            list.extend(found.get(&id).into_iter().flatten());
        }
        v.confinement.clone_from(&confinement);
        v.normalize();
    }
    Ok(out)
}

/// `roots` with their descendants in `table` (`entity` or `site`) at `at`, by root.
async fn descendants_by_root(
    conn: &mut PgConnection,
    table: &str,
    roots: &[Uuid],
    at: OffsetDateTime,
) -> Result<std::collections::HashMap<Uuid, Vec<Uuid>>, StoreError> {
    let mut out: std::collections::HashMap<Uuid, Vec<Uuid>> = std::collections::HashMap::new();
    if roots.is_empty() {
        return Ok(out);
    }
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH RECURSIVE tree(root, id) AS (
             SELECT t.id, t.id FROM {table} t WHERE t.id = ANY($1) AND t.valid_period @> $2
             UNION
             SELECT tree.root, c.id FROM {table} c JOIN tree ON c.parent_id = tree.id
             WHERE c.valid_period @> $2)
         SELECT root, id FROM tree"
    )))
    .bind(roots)
    .bind(at)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    for (root, id) in rows {
        out.entry(root).or_default().push(id);
    }
    Ok(out)
}

/// The bound of the issuer of `subject` (K4, K13): `None` without issuer or with a global
/// attachment; the attached entities with their descendants otherwise (empty without
/// attachment: no right).
async fn confinement(
    conn: &mut PgConnection,
    subject: &Subject,
    at: OffsetDateTime,
) -> Result<Option<Vec<Uuid>>, StoreError> {
    let Some(issuer) = &subject.issuer else {
        return Ok(None);
    };
    let attached: Vec<Option<Uuid>> = sqlx::query_scalar(
        "SELECT entity_id FROM issuer_attachment WHERE issuer = $1 AND valid_period @> $2",
    )
    .bind(issuer)
    .bind(at)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    if attached.iter().any(Option::is_none) {
        return Ok(None);
    }
    let entities: Vec<Uuid> = attached.into_iter().flatten().collect();
    Ok(Some(descendants(conn, "entity", &entities, at).await?))
}

/// The objects among `objects` outside `visibility` at `at`, as `conn` sees them.
///
/// # Errors
///
/// Database errors.
pub async fn outside_on(
    conn: &mut PgConnection,
    visibility: &Visibility,
    objects: &[Uuid],
    at: OffsetDateTime,
) -> Result<Vec<Uuid>, StoreError> {
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    if visibility.is_empty() {
        return Ok(objects.to_vec());
    }
    let a = visibility.arguments();
    sqlx::query_scalar(
        "SELECT o.id FROM unnest($1::uuid[]) AS o(id)
         LEFT JOIN carrier_object c ON c.id = o.id AND c.valid_period @> $2
         WHERE NOT coalesce(kartenn_visible(o.id, c.domain_id, c.value, $2, $3, $4, $5,
                                            $6, $7, $8::text[]::cidr[], $9), false)",
    )
    .bind(objects)
    .bind(at)
    .bind(a.global)
    .bind(&a.entities)
    .bind(&a.sites)
    .bind(&a.domains)
    .bind(&a.prefix_domains)
    .bind(&a.prefixes)
    .bind(a.confinement.as_deref())
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)
}

/// [`Authorizer::authorize_many`] on `conn`, now.
///
/// # Errors
///
/// Database errors.
pub async fn outside_now(
    conn: &mut PgConnection,
    subject: &Subject,
    permission: &str,
    objects: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    let now = OffsetDateTime::now_utc();
    let visibility = visibility_on(conn, subject, permission, now).await?;
    outside_on(conn, &visibility, objects, now).await
}

impl PgAuthorizer {
    /// On `pool`.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            modules: Arc::default(),
        }
    }

    /// With the modules of the build, whose types of objects the read scope covers.
    #[must_use]
    pub fn with_modules(mut self, modules: Arc<ModuleRegistry>) -> Self {
        self.modules = modules;
        self
    }

    /// Where `subject` holds `permission` at `at`.
    ///
    /// # Errors
    ///
    /// Database errors.
    pub async fn visibility(
        &self,
        subject: &Subject,
        permission: &str,
        at: OffsetDateTime,
    ) -> Result<Visibility, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_err)?;
        visibility_on(&mut conn, subject, permission, at).await
    }

    /// What `subject` may read at `at`, kind by kind.
    ///
    /// # Errors
    ///
    /// Database errors.
    pub async fn read_scope(
        &self,
        subject: &Subject,
        at: OffsetDateTime,
    ) -> Result<ReadScope, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_err)?;
        let reads: Vec<(&'static str, String)> = READ_PERMISSIONS
            .iter()
            .map(|(kind, p)| (*kind, (*p).to_owned()))
            .chain(
                self.modules
                    .object_types()
                    .map(|t| (t.kind, format!("{}.read", t.kind))),
            )
            .collect();
        let permissions: Vec<&str> = reads.iter().map(|(_, p)| p.as_str()).collect();
        let visibilities = visibilities_on(&mut conn, subject, &permissions, at).await?;
        let kinds = reads
            .iter()
            .map(|(kind, _)| *kind)
            .zip(visibilities)
            .collect();
        Ok(ReadScope(Some(kinds)))
    }

    /// The objects among `objects` outside `visibility` at `at`.
    ///
    /// # Errors
    ///
    /// Database errors.
    pub async fn outside(
        &self,
        visibility: &Visibility,
        objects: &[Uuid],
        at: OffsetDateTime,
    ) -> Result<Vec<Uuid>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_err)?;
        outside_on(&mut conn, visibility, objects, at).await
    }
}

impl Authorizer for PgAuthorizer {
    async fn authorize_many(
        &self,
        subject: &Subject,
        permission: &str,
        objects: &[Uuid],
    ) -> Result<Vec<Uuid>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_err)?;
        outside_now(&mut conn, subject, permission, objects).await
    }

    fn authorize_in<T: WriteTx + Send>(
        &self,
        tx: &mut T,
        subject: &Subject,
        permission: &str,
        objects: &[Uuid],
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send {
        tx.rbac_outside(subject, permission, objects)
    }
}

/// Whether one of `objects` lies in the scope `kind` `scope` or below it, now, as `conn`
/// sees them (approval policies by scope, REQ-005-27).
///
/// # Errors
///
/// Database errors.
pub async fn scope_touched_on(
    conn: &mut PgConnection,
    kind: &str,
    scope: Uuid,
    objects: &[Uuid],
) -> Result<bool, StoreError> {
    let now = OffsetDateTime::now_utc();
    let mut v = Visibility::none();
    match kind {
        "entity" => v.entities = descendants(conn, "entity", &[scope], now).await?,
        "site" => v.sites = descendants(conn, "site", &[scope], now).await?,
        "prefix" => {
            v.prefixes = sqlx::query_as(
                "SELECT domain_id, prefix::text FROM prefix WHERE id = $1 AND valid_period @> $2",
            )
            .bind(scope)
            .bind(now)
            .fetch_all(&mut *conn)
            .await
            .map_err(store_err)?;
        }
        _ => return Ok(false),
    }
    Ok(outside_on(conn, &v, objects, now).await?.len() < objects.len())
}

/// The keys of the roles `subject` holds now, on any scope.
///
/// # Errors
///
/// Database errors.
pub async fn roles_of_on(
    conn: &mut PgConnection,
    subject: &Subject,
) -> Result<Vec<String>, StoreError> {
    sqlx::query_scalar(
        "SELECT DISTINCT r.key FROM binding b
         JOIN role r ON r.id = b.role_id AND r.valid_period @> now()
         WHERE b.valid_period @> now()
           AND ((b.subject_kind = 'identity' AND b.subject_id = $1)
                OR (b.subject_kind = 'group' AND b.issuer = $2 AND b.group_name = ANY($3))
                OR (b.subject_kind = 'issuer' AND b.issuer = $2))
         ORDER BY r.key",
    )
    .bind(subject.id)
    .bind(subject.issuer.as_deref())
    .bind(&subject.groups)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_visibility_drops_what_others_cover() {
        let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let mut v = Visibility {
            domains: vec![a, a],
            prefixes: vec![
                (a, "10.0.0.0/8".into()),
                (b, "10.1.0.0/16".into()),
                (b, "10.0.0.0/8".into()),
                (b, "10.1.2.0/24".into()),
                (b, "10.0.0.0/8".into()),
                (b, "192.0.2.0/24".into()),
                (b, "2001:db8::/32".into()),
            ],
            ..Visibility::none()
        };
        v.normalize();
        assert_eq!(v.domains, [a], "duplicates go");
        assert_eq!(
            v.prefixes,
            [
                (b, "10.0.0.0/8".to_owned()),
                (b, "192.0.2.0/24".to_owned()),
                (b, "2001:db8::/32".to_owned()),
            ],
            "the subtrees of a granted domain, or inside a granted subtree, go"
        );
        let mut global = Visibility {
            global: true,
            domains: vec![a],
            confinement: Some(vec![b]),
            ..Visibility::none()
        };
        global.normalize();
        assert!(global.domains.is_empty());
        assert_eq!(global.confinement, Some(vec![b]), "the bound stays");
    }
}
