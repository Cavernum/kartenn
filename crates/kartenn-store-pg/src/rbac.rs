//! RBAC in PostgreSQL (spec 005, REQ-005-18 to 20, 23; ADR 0021): the visibility of a
//! subject for a permission, computed from its bindings, and the decision of
//! [`Authorizer::authorize_many`] by the predicate `kartenn_visible` (migration 0029),
//! which also filters the lists.

use kartenn_service::StoreError;
use kartenn_service::authz::{Authorizer, Subject};
use std::future::Future;

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
    /// As a JSON object, for `kartenn_readable` (migration 0030).
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

    /// The scope as the JSON argument of `kartenn_readable`; `None` for everything.
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
    let action = permission.rsplit('.').next().unwrap_or_default();
    let pattern_applies =
        permission.starts_with("prop.") && !sensitive(conn, permission, at).await?;
    let rows: Vec<(String, Option<Uuid>)> = sqlx::query_as(
        "SELECT DISTINCT b.scope_kind, b.scope_id
         FROM binding b
         JOIN role r ON r.id = b.role_id AND r.valid_period @> $2
         JOIN role_permission rp ON rp.role_id = r.id AND rp.valid_period @> $2
         WHERE b.valid_period @> $2
           AND (rp.permission IN ('*', $1)
                OR ($6 AND rp.permission = 'prop.*.' || $7))
           AND ((b.subject_kind = 'identity' AND b.subject_id = $3)
                OR (b.subject_kind = 'group' AND b.issuer = $4 AND b.group_name = ANY($5))
                OR (b.subject_kind = 'issuer' AND b.issuer = $4))",
    )
    .bind(permission)
    .bind(at)
    .bind(subject.id)
    .bind(subject.issuer.as_deref())
    .bind(&subject.groups)
    .bind(pattern_applies)
    .bind(action)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    // A global-only permission acts on the global scope alone (K7, K11).
    let global_only = kartenn_core::authz::permission(permission).is_some_and(|p| p.global_only);
    let mut v = Visibility::none();
    let (mut entities, mut sites, mut prefix_ids) = (Vec::new(), Vec::new(), Vec::new());
    for (kind, id) in rows {
        match (kind.as_str(), id) {
            ("global", _) => v.global = true,
            (_, _) if global_only => {}
            ("entity", Some(id)) => entities.push(id),
            ("site", Some(id)) => sites.push(id),
            ("domain", Some(id)) => v.domains.push(id),
            ("prefix", Some(id)) => prefix_ids.push(id),
            _ => {}
        }
    }
    v.entities = descendants(conn, "entity", &entities, at).await?;
    v.sites = descendants(conn, "site", &sites, at).await?;
    if !prefix_ids.is_empty() {
        v.prefixes = sqlx::query_as(
            "SELECT domain_id, prefix::text FROM prefix WHERE id = ANY($1) AND valid_period @> $2",
        )
        .bind(&prefix_ids)
        .bind(at)
        .fetch_all(&mut *conn)
        .await
        .map_err(store_err)?;
    }
    v.confinement = confinement(conn, subject, at).await?;
    Ok(v)
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
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
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
        let mut kinds = std::collections::BTreeMap::new();
        for (kind, permission) in READ_PERMISSIONS {
            kinds.insert(
                kind,
                visibility_on(&mut conn, subject, permission, at).await?,
            );
        }
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
