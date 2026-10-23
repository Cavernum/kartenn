//! What an applied changeset wrote, to undo it (spec 004, REQ-004-34, 35; task 004-20).

use kartenn_service::operations::{BindingScope, BindingSubject, PeriodSpec, Ref};
use kartenn_service::port::{
    RbacBefore, RbacRow, RbacState, RbacTouched, StoreError, Touched, UndoFacts,
};
use kartenn_service::{ObjectKind, Operation};
use sqlx::postgres::PgRow;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgPool, Row as _};

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Versioned objects a changeset may write, by kind.
const OBJECTS: [(ObjectKind, &str); 6] = [
    (ObjectKind::Domain, "addressing_domain"),
    (ObjectKind::Prefix, "prefix"),
    (ObjectKind::Range, "ip_range"),
    (ObjectKind::Address, "address"),
    (ObjectKind::Entity, "entity"),
    (ObjectKind::Site, "site"),
];

/// Per key of `table` written by `$1` (applied at `$2`): created (nothing asserted
/// before) and the first later changeset that wrote it over `[$3, ∞)`.
fn touched_sql(table: &str, key: &str, extra: &str) -> String {
    format!(
        "WITH k AS (SELECT DISTINCT {key}{extra} FROM {table} WHERE changeset_id = $1
                    UNION SELECT DISTINCT {key}{extra} FROM {table}_history WHERE changeset_id = $1),
              v AS (SELECT {key}, changeset_id, sys_start, NULL::timestamptz AS sys_end, valid_period
                    FROM {table} WHERE ({key}) IN (SELECT {key} FROM k)
                    UNION ALL
                    SELECT {key}, changeset_id, sys_start, sys_end, valid_period
                    FROM {table}_history WHERE ({key}) IN (SELECT {key} FROM k))
         SELECT k.*,
                NOT EXISTS (SELECT 1 FROM v WHERE ({key}) = (k.{key_k}) AND v.sys_start < $2
                                              AND (v.sys_end IS NULL OR v.sys_end >= $2)),
                (SELECT v.changeset_id FROM v WHERE ({key}) = (k.{key_k}) AND v.sys_start > $2
                   AND v.changeset_id <> $1 AND v.valid_period && tstzrange($3, NULL)
                 ORDER BY v.sys_start LIMIT 1)
         FROM k",
        key_k = key.replace(", ", ", k."),
    )
}

/// Per key of the RBAC table `table` written by `$1` (applied at `$2`): its version in
/// force before, over `[$3, ∞)`, with its columns `cols` as `b_<col>`, whether one is in
/// force after, and the first later changeset that wrote it there (spec 005, K16). A row
/// the changeset deleted (a period that started later) ends in the history at `$2`, the
/// instant of its transaction. `more` adds keys.
fn rbac_sql(table: &str, key: &[&str], cols: &[&str], more: &str) -> String {
    let list = |a: &str| {
        key.iter()
            .map(|c| format!("{a}.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let same = |a: &str| format!("({}) IS NOT DISTINCT FROM ({})", list(a), list("k"));
    let keys = key.join(", ");
    let all = key
        .iter()
        .chain(cols)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    let before = cols
        .iter()
        .map(|c| format!("b.{c} AS b_{c}, "))
        .collect::<Vec<_>>()
        .concat();
    format!(
        "WITH k AS (SELECT DISTINCT {keys} FROM {table} WHERE changeset_id = $1
                    UNION SELECT DISTINCT {keys} FROM {table}_history
                          WHERE changeset_id = $1 OR sys_end = $2
                    {more}),
              v AS (SELECT {all}, valid_period, sys_start, NULL::timestamptz AS sys_end, changeset_id
                    FROM {table}
                    UNION ALL
                    SELECT {all}, valid_period, sys_start, sys_end, changeset_id FROM {table}_history)
         SELECT k.*, {before}
                b.valid_period IS NOT NULL AS b_exists,
                lower(b.valid_period) AS b_start, upper(b.valid_period) AS b_end,
                EXISTS (SELECT 1 FROM {table} c WHERE {after}
                               AND c.valid_period && tstzrange($3, NULL)) AS after,
                (SELECT l.changeset_id FROM v l WHERE {later} AND l.sys_start > $2
                   AND l.changeset_id <> $1 AND l.valid_period && tstzrange($3, NULL)
                 ORDER BY l.sys_start LIMIT 1) AS later
         FROM k LEFT JOIN LATERAL (
             SELECT * FROM v WHERE {was} AND v.sys_start < $2
                AND (v.sys_end IS NULL OR v.sys_end >= $2) AND v.valid_period && tstzrange($3, NULL)
             ORDER BY lower(v.valid_period) LIMIT 1) b ON true",
        after = same("c"),
        later = same("l"),
        was = same("v"),
    )
}

/// The rows of [`rbac_sql`].
async fn rbac_rows(
    pool: &PgPool,
    sql: String,
    (id, at, from): (Uuid, OffsetDateTime, OffsetDateTime),
) -> Result<Vec<PgRow>, StoreError> {
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(at)
        .bind(from)
        .fetch_all(pool)
        .await
        .map_err(store_err)
}

fn micros(t: OffsetDateTime) -> i64 {
    i64::try_from(t.unix_timestamp_nanos() / 1_000).unwrap_or(i64::MIN)
}

/// The period before of a row of [`rbac_sql`], when it had one.
fn period_before(row: &PgRow) -> Result<Option<PeriodSpec>, StoreError> {
    if !row.try_get::<bool, _>("b_exists").map_err(store_err)? {
        return Ok(None);
    }
    let start: OffsetDateTime = row.try_get("b_start").map_err(store_err)?;
    let end: Option<OffsetDateTime> = row.try_get("b_end").map_err(store_err)?;
    Ok(Some(PeriodSpec {
        start: micros(start),
        end: end.map(micros),
    }))
}

/// The key of the role `id`, from any of its versions.
async fn role_key(pool: &PgPool, id: Uuid) -> Result<String, StoreError> {
    sqlx::query_scalar(
        "SELECT key FROM role WHERE id = $1 UNION ALL SELECT key FROM role_history WHERE id = $1
         LIMIT 1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(store_err)
}

/// The scope of a binding from its columns.
fn scope(kind: &str, id: Option<Uuid>) -> Result<BindingScope, StoreError> {
    let id = || {
        id.map(Ref::Id)
            .ok_or_else(|| store_err(format!("a {kind} scope without object")))
    };
    Ok(match kind {
        "global" => BindingScope::Global,
        "entity" => BindingScope::Entity { id: id()? },
        "site" => BindingScope::Site { id: id()? },
        "domain" => BindingScope::Domain { id: id()? },
        "prefix" => BindingScope::Prefix { id: id()? },
        other => return Err(store_err(format!("unknown scope kind {other}"))),
    })
}

/// The subject of a binding from its columns.
fn subject(row: &PgRow) -> Result<BindingSubject, StoreError> {
    let kind: String = row.try_get("b_subject_kind").map_err(store_err)?;
    let id: Option<Uuid> = row.try_get("b_subject_id").map_err(store_err)?;
    let issuer: Option<String> = row.try_get("b_issuer").map_err(store_err)?;
    let group: Option<String> = row.try_get("b_group_name").map_err(store_err)?;
    match (kind.as_str(), id, issuer, group) {
        ("identity", Some(id), _, _) => Ok(BindingSubject::Identity { id }),
        ("group", _, Some(issuer), Some(group)) => Ok(BindingSubject::Group { issuer, group }),
        ("issuer", _, Some(issuer), _) => Ok(BindingSubject::Issuer { issuer }),
        _ => Err(store_err(format!("a malformed {kind} binding subject"))),
    }
}

/// The roles the changeset `args.0` (applied at `args.1`) wrote, by their row or one of
/// their entries, as they were before over `[args.2, ∞)`.
async fn roles_touched(
    pool: &PgPool,
    args: (Uuid, OffsetDateTime, OffsetDateTime),
) -> Result<Vec<RbacTouched>, StoreError> {
    let (id, at, from) = args;
    let mut touched = Vec::new();
    let sql = rbac_sql(
        "role",
        &["id"],
        &["key", "labels"],
        "UNION SELECT DISTINCT role_id FROM role_permission WHERE changeset_id = $1
         UNION SELECT DISTINCT role_id FROM role_permission_history
               WHERE changeset_id = $1 OR sys_end = $2",
    );
    for row in rbac_rows(pool, sql, args).await? {
        let role: Uuid = row.try_get("id").map_err(store_err)?;
        let period = period_before(&row)?;
        let before = match period {
            Some(period) => {
                let at_start = OffsetDateTime::from_unix_timestamp_nanos(
                    i128::from(period.start.max(micros(from))) * 1_000,
                )
                .map_err(store_err)?;
                let entries = sqlx::query_scalar(
                    "SELECT permission FROM role_permission
                     WHERE role_id = $1 AND sys_start < $2 AND valid_period @> $3::timestamptz
                     UNION
                     SELECT permission FROM role_permission_history
                     WHERE role_id = $1 AND sys_start < $2 AND sys_end >= $2
                       AND valid_period @> $3::timestamptz
                     ORDER BY 1",
                )
                .bind(role)
                .bind(at)
                .bind(at_start)
                .fetch_all(pool)
                .await
                .map_err(store_err)?;
                Some(RbacBefore {
                    period,
                    state: RbacState::Role {
                        labels: row
                            .try_get::<sqlx::types::Json<_>, _>("b_labels")
                            .map_err(store_err)?
                            .0,
                        entries,
                    },
                })
            }
            None => None,
        };
        // A later write of an entry; for a role it created, a binding of another
        // changeset, which closing the role would end.
        let later: Option<Uuid> = sqlx::query_scalar(
            "SELECT changeset_id FROM (
                 SELECT changeset_id, sys_start, valid_period FROM role_permission WHERE role_id = $1
                 UNION ALL
                 SELECT changeset_id, sys_start, valid_period FROM role_permission_history
                 WHERE role_id = $1
                 UNION ALL
                 SELECT changeset_id, sys_start, valid_period FROM binding
                 WHERE role_id = $1 AND $5
                 UNION ALL
                 SELECT changeset_id, sys_start, valid_period FROM binding_history
                 WHERE role_id = $1 AND $5) w
             WHERE sys_start > $2 AND changeset_id <> $3 AND valid_period && tstzrange($4, NULL)
             ORDER BY sys_start LIMIT 1",
        )
        .bind(role)
        .bind(at)
        .bind(id)
        .bind(from)
        .bind(before.is_none())
        .fetch_optional(pool)
        .await
        .map_err(store_err)?;
        touched.push(RbacTouched {
            row: RbacRow::Role {
                key: role_key(pool, role).await?,
            },
            before,
            after: row.try_get("after").map_err(store_err)?,
            later: row
                .try_get::<Option<Uuid>, _>("later")
                .map_err(store_err)?
                .or(later),
        });
    }
    Ok(touched)
}

/// The roles, bindings and issuer attachments the changeset `id` (applied at `at`) wrote,
/// as they were before over `[from, ∞)` (spec 005, REQ-005-25, K16).
async fn rbac_touched(
    pool: &PgPool,
    id: Uuid,
    at: OffsetDateTime,
    from: OffsetDateTime,
) -> Result<Vec<RbacTouched>, StoreError> {
    let args = (id, at, from);
    let mut touched = roles_touched(pool, args).await?;
    let sql = rbac_sql(
        "binding",
        &["id"],
        &[
            "subject_kind",
            "subject_id",
            "issuer",
            "group_name",
            "role_id",
            "scope_kind",
            "scope_id",
        ],
        "",
    );
    for row in rbac_rows(pool, sql, args).await? {
        let before = match period_before(&row)? {
            Some(period) => {
                let kind: String = row.try_get("b_scope_kind").map_err(store_err)?;
                Some(RbacBefore {
                    period,
                    state: RbacState::Binding {
                        subject: subject(&row)?,
                        role: role_key(pool, row.try_get("b_role_id").map_err(store_err)?).await?,
                        scope: scope(&kind, row.try_get("b_scope_id").map_err(store_err)?)?,
                    },
                })
            }
            None => None,
        };
        touched.push(RbacTouched {
            row: RbacRow::Binding {
                id: row.try_get("id").map_err(store_err)?,
            },
            before,
            after: row.try_get("after").map_err(store_err)?,
            later: row.try_get("later").map_err(store_err)?,
        });
    }
    let sql = rbac_sql("issuer_attachment", &["issuer", "entity_id"], &[], "");
    for row in rbac_rows(pool, sql, args).await? {
        touched.push(RbacTouched {
            row: RbacRow::Attachment {
                issuer: row.try_get("issuer").map_err(store_err)?,
                entity: row.try_get("entity_id").map_err(store_err)?,
            },
            before: period_before(&row)?.map(|period| RbacBefore {
                period,
                state: RbacState::Attachment,
            }),
            after: row.try_get("after").map_err(store_err)?,
            later: row.try_get("later").map_err(store_err)?,
        });
    }
    Ok(touched)
}

fn kind_of(name: &str) -> Result<ObjectKind, StoreError> {
    OBJECTS
        .iter()
        .find(|(k, _)| k.name() == name)
        .map(|(k, _)| *k)
        // An object of a module (spec 008, REQ-008-3).
        .or_else(|| ObjectKind::from_name(name).filter(|k| matches!(k, ObjectKind::Module(_))))
        .ok_or_else(|| store_err(format!("unknown object kind {name}")))
}

/// The objects of the modules the changeset `args.0` (applied at `args.1`) wrote, by their
/// record in the core (spec 008, REQ-008-3): a module writes its own rows beside, and
/// gives them back at the restore.
async fn module_objects(
    pool: &PgPool,
    (id, at, from): (Uuid, OffsetDateTime, OffsetDateTime),
) -> Result<Vec<Touched>, StoreError> {
    let rows: Vec<(Uuid, String, bool, Option<Uuid>)> = sqlx::query_as(sqlx::AssertSqlSafe(
        touched_sql("module_object", "id", ", kind"),
    ))
    .bind(id)
    .bind(at)
    .bind(from)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    rows.into_iter()
        .map(|(object, kind, created, later)| {
            Ok(Touched {
                kind: kind_of(&kind)?,
                id: object,
                property: None,
                created,
                later,
            })
        })
        .collect()
}

/// What the applied changeset `id` wrote; `None` if it is not applied.
pub(crate) async fn facts(pool: &PgPool, id: Uuid) -> Result<Option<UndoFacts>, StoreError> {
    let row: Option<(Option<OffsetDateTime>, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT applied_at, lower(default_period) FROM changeset WHERE id = $1 AND state = 'applied'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(store_err)?;
    let Some((Some(at), Some(default_start))) = row else {
        return Ok(None);
    };
    // The earliest start: the changeset's period, or an operation's own (REQ-004-2).
    let payloads: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT payload FROM changeset_operation WHERE changeset_id = $1")
            .bind(id)
            .fetch_all(pool)
            .await
            .map_err(store_err)?;
    let start = payloads
        .into_iter()
        .filter_map(|p| serde_json::from_value::<Operation>(p).ok())
        .filter_map(|op| op.own_period())
        .map(|p| p.start)
        .chain(std::iter::once(micros(default_start)))
        .min()
        .unwrap_or_else(|| micros(default_start));
    let from =
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(start) * 1_000).map_err(store_err)?;
    let mut touched = Vec::new();
    for (kind, table) in OBJECTS {
        let rows: Vec<(Uuid, bool, Option<Uuid>)> =
            sqlx::query_as(sqlx::AssertSqlSafe(touched_sql(table, "id", "")))
                .bind(id)
                .bind(at)
                .bind(from)
                .fetch_all(pool)
                .await
                .map_err(store_err)?;
        touched.extend(rows.into_iter().map(|(object, created, later)| Touched {
            kind,
            id: object,
            property: None,
            created,
            later,
        }));
    }
    touched.extend(module_objects(pool, (id, at, from)).await?);
    let rows: Vec<(Uuid, Uuid, String, bool, Option<Uuid>)> =
        sqlx::query_as(sqlx::AssertSqlSafe(touched_sql(
            "property_assignment",
            "object_id, property_id",
            ", object_type",
        )))
        .bind(id)
        .bind(at)
        .bind(from)
        .fetch_all(pool)
        .await
        .map_err(store_err)?;
    for (object, property, kind, created, later) in rows {
        touched.push(Touched {
            kind: kind_of(&kind)?,
            id: object,
            property: Some(property),
            created,
            later,
        });
    }
    let unsupported: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT 'property', id FROM property_declaration WHERE changeset_id = $1
         UNION SELECT 'property', id FROM property_declaration_history WHERE changeset_id = $1
         UNION SELECT 'enum_value', id FROM property_enum_value WHERE changeset_id = $1
         UNION SELECT 'enum_value', id FROM property_enum_value_history WHERE changeset_id = $1
         -- Service accounts, their secrets and identity states (005) keep no history:
         -- no inverse operation; listed by operation (K16).
         UNION SELECT payload ->> 'op', changeset_id FROM changeset_operation
               WHERE changeset_id = $1
                 AND payload ->> 'op' IN ('create_service_account', 'add_service_secret',
                                          'revoke_service_secret', 'set_identity_state')
         ORDER BY 1, 2",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    Ok(Some(UndoFacts {
        start,
        touched,
        unsupported,
        rbac: rbac_touched(pool, id, at, from).await?,
    }))
}
