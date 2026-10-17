//! RBAC in PostgreSQL (spec 005, REQ-005-18 to 20, 23; ADR 0021): the visibility of a
//! subject for a permission, computed from its bindings, and the decision of
//! [`Authorizer::authorize_many`] by the predicate `kartenn_visible` (migration 0029),
//! which also filters the lists.

use kartenn_service::StoreError;
use kartenn_service::authz::{Authorizer, Subject};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

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

/// The authorizer of the RBAC of spec 005.
#[derive(Debug, Clone)]
pub struct PgAuthorizer {
    pool: PgPool,
}

/// Whether the property of a property permission is sensitive (003/P17): the patterns
/// `prop.*.<action>` spare it.
async fn sensitive(
    pool: &PgPool,
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
    .fetch_optional(pool)
    .await
    .map_err(store_err)?;
    Ok(found.unwrap_or(false))
}

/// `ids` with their descendants in `table` (`entity` or `site`) at `at`.
async fn descendants(
    pool: &PgPool,
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
    .fetch_all(pool)
    .await
    .map_err(store_err)
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
        let action = permission.rsplit('.').next().unwrap_or_default();
        let pattern_applies =
            permission.starts_with("prop.") && !sensitive(&self.pool, permission, at).await?;
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
        .fetch_all(&self.pool)
        .await
        .map_err(store_err)?;
        // A global-only permission acts on the global scope alone (K7, K11).
        let global_only =
            kartenn_core::authz::permission(permission).is_some_and(|p| p.global_only);
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
        v.entities = descendants(&self.pool, "entity", &entities, at).await?;
        v.sites = descendants(&self.pool, "site", &sites, at).await?;
        if !prefix_ids.is_empty() {
            v.prefixes = sqlx::query_as(
                "SELECT domain_id, prefix::text FROM prefix WHERE id = ANY($1) AND valid_period @> $2",
            )
            .bind(&prefix_ids)
            .bind(at)
            .fetch_all(&self.pool)
            .await
            .map_err(store_err)?;
        }
        v.confinement = self.confinement(subject, at).await?;
        Ok(v)
    }

    /// The bound of the issuer of `subject` (K4, K13): `None` without issuer or with a
    /// global attachment; the attached entities with their descendants otherwise (empty
    /// without attachment: no right).
    async fn confinement(
        &self,
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
        .fetch_all(&self.pool)
        .await
        .map_err(store_err)?;
        if attached.iter().any(Option::is_none) {
            return Ok(None);
        }
        let entities: Vec<Uuid> = attached.into_iter().flatten().collect();
        Ok(Some(
            descendants(&self.pool, "entity", &entities, at).await?,
        ))
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
        .fetch_all(&self.pool)
        .await
        .map_err(store_err)
    }
}

impl Authorizer for PgAuthorizer {
    async fn authorize_many(
        &self,
        subject: &Subject,
        permission: &str,
        objects: &[Uuid],
    ) -> Result<Vec<Uuid>, StoreError> {
        let now = OffsetDateTime::now_utc();
        let visibility = self.visibility(subject, permission, now).await?;
        self.outside(&visibility, objects, now).await
    }
}
