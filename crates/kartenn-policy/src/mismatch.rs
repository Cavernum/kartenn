//! The deviations of the inference by origin node (REQ-010-48, I9, Q13): the background
//! task `policy.inference_check` re-evaluates, once the revision of the rules of inference
//! moves, the origin nodes of `policy.environment` in the domains those rules cite. For
//! each origin it counts the heirs whose single expected environment differs from the
//! declared one, and raises `policy.inference.mismatch` on the origin when some do; the
//! origins it went over are evaluated anew, so that a corrected deviation closes
//! (ADR 0026, amendment 4). A pass goes by batches of heirs, one per run, and never touches
//! the referential at each changeset.

use std::collections::BTreeSet;

use kartenn_core::rules::Violation;
use kartenn_service::expected::{ExpectedContext, ExpectedValueSource};
use kartenn_service::module::{Findings, ModuleTx};
use kartenn_service::operations::RawInput;
use kartenn_service::port::{ObjectView, StoreError};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::infer::InferenceSource;
use crate::module::ENVIRONMENT;
use crate::pg::store;
use crate::rules::INFERENCE_MISMATCH;

/// The heirs evaluated by one run of the task, at most.
pub const BATCH: i64 = 1_000;

/// The progress of the task (`policy.policy_inference_check`).
#[derive(Debug, Clone, sqlx::FromRow)]
struct Progress {
    checked: i64,
    since: OffsetDateTime,
    pass: Option<i64>,
    pass_since: Option<OffsetDateTime>,
    scope: Option<Vec<Uuid>>,
    origin: Option<Uuid>,
    after_origin: Option<Uuid>,
    after_heir: Option<Uuid>,
    deviating: i64,
}

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut::<PgConnection>()
        .ok_or_else(|| StoreError("the policy needs a PostgreSQL transaction".into()))
}

/// One run of the task: starts a pass when the rules of inference changed since the last
/// one, then goes over at most [`BATCH`] heirs of the origins of the pass.
///
/// # Errors
/// A failure of the database.
pub async fn run(tx: &mut dyn ModuleTx) -> Result<Findings, StoreError> {
    run_with(tx, BATCH).await
}

/// [`run`] with a batch of `batch` heirs.
///
/// # Errors
/// A failure of the database.
pub async fn run_with(tx: &mut dyn ModuleTx, batch: i64) -> Result<Findings, StoreError> {
    let conn = connection(tx)?;
    let mut p: Progress = sqlx::query_as(
        "SELECT checked, since, pass, pass_since, scope, origin, after_origin, after_heir,
                deviating
           FROM policy.policy_inference_check FOR UPDATE",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let mut findings = Findings::default();
    if p.pass.is_none() {
        let revision: i64 = sqlx::query_scalar("SELECT inference FROM policy.policy_revision")
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
        if revision == p.checked {
            return Ok(findings);
        }
        let started: OffsetDateTime = sqlx::query_scalar("SELECT now()")
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
        p.pass = Some(revision);
        p.pass_since = Some(started);
        p.scope = scope(conn, p.since).await?;
        p.origin = None;
        p.after_origin = None;
        p.after_heir = None;
        p.deviating = 0;
    }
    let Some(environment) = environment_id(conn).await? else {
        return finish(conn, p, findings).await;
    };
    let mut budget = batch.max(1);
    loop {
        let conn = connection(tx)?;
        let origin = if let Some(origin) = p.origin {
            origin
        } else if let Some(origin) =
            next_origin(conn, environment, p.after_origin, p.scope.as_deref()).await?
        {
            p.origin = Some(origin);
            p.after_heir = None;
            p.deviating = 0;
            origin
        } else {
            p.pass_done();
            return finish(conn, p, findings).await;
        };
        let requested = budget;
        let heirs = heirs(conn, environment, origin, p.after_heir, requested).await?;
        p.deviating += deviating(tx, origin, environment, &heirs).await?;
        let read = i64::try_from(heirs.len()).unwrap_or(i64::MAX);
        budget -= read;
        if read < requested {
            // Every heir of the origin was read: its count is complete.
            findings.reevaluated.push((origin, INFERENCE_MISMATCH.id));
            if p.deviating > 0 {
                findings.violations.push(
                    Violation::new(
                        &INFERENCE_MISMATCH,
                        format!(
                            "{} héritier(s) de la valeur déclarée ici portent un environnement \
                             que l'inférence n'attend pas",
                            p.deviating
                        ),
                    )
                    .with_subjects([origin]),
                );
            }
            p.after_origin = Some(origin);
            p.origin = None;
            p.after_heir = None;
            p.deviating = 0;
        } else {
            p.after_heir = heirs.last().copied();
        }
        if budget <= 0 {
            let conn = connection(tx)?;
            return finish(conn, p, findings).await;
        }
    }
}

/// The heirs among `heirs` whose single expected environment is not the one declared on
/// `origin`; an heir with no or several expected sets is not a deviation (REQ-010-45).
async fn deviating(
    tx: &mut dyn ModuleTx,
    origin: Uuid,
    environment: Uuid,
    heirs: &[Uuid],
) -> Result<i64, StoreError> {
    let conn = connection(tx)?;
    let Some(declared) = declared(conn, origin, environment).await? else {
        return Ok(0);
    };
    let views = views(conn, heirs).await?;
    let expected = InferenceSource
        .expected_many(ENVIRONMENT, &views, ExpectedContext::default(), tx)
        .await?;
    let mut count = 0;
    for single in expected.iter().filter_map(|e| e.single()) {
        let values: BTreeSet<String> = single
            .values
            .iter()
            .filter_map(|v| match v {
                RawInput::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        if values != declared {
            count += 1;
        }
    }
    Ok(count)
}

impl Progress {
    /// The pass is over: its revision is checked.
    fn pass_done(&mut self) {
        if let (Some(pass), Some(since)) = (self.pass, self.pass_since) {
            self.checked = pass;
            self.since = since;
        }
        self.pass = None;
        self.pass_since = None;
        self.scope = None;
        self.origin = None;
        self.after_origin = None;
        self.after_heir = None;
        self.deviating = 0;
    }
}

/// Stores the progress, and returns the findings of the run.
async fn finish(
    conn: &mut PgConnection,
    p: Progress,
    findings: Findings,
) -> Result<Findings, StoreError> {
    sqlx::query(
        "UPDATE policy.policy_inference_check
            SET checked = $1, since = $2, pass = $3, pass_since = $4, scope = $5, origin = $6,
                after_origin = $7, after_heir = $8, deviating = $9",
    )
    .bind(p.checked)
    .bind(p.since)
    .bind(p.pass)
    .bind(p.pass_since)
    .bind(p.scope)
    .bind(p.origin)
    .bind(p.after_origin)
    .bind(p.after_heir)
    .bind(p.deviating)
    .execute(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(findings)
}

/// The domains cited by the rules of inference written or ended since `since`; `None`
/// (every domain) when one of them cites none (REQ-010-46).
async fn scope(
    conn: &mut PgConnection,
    since: OffsetDateTime,
) -> Result<Option<Vec<Uuid>>, StoreError> {
    let cited: Vec<Option<Uuid>> = sqlx::query_scalar(
        "SELECT domain_id FROM policy.policy_inference_rule WHERE sys_start > $1
         UNION
         SELECT domain_id FROM policy.policy_inference_rule_history WHERE sys_end > $1",
    )
    .bind(since)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    if cited.iter().any(Option::is_none) {
        return Ok(None);
    }
    Ok(Some(cited.into_iter().flatten().collect()))
}

/// The declaration of `policy.environment` living now.
async fn environment_id(conn: &mut PgConnection) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT id FROM property_declaration WHERE key = $1 AND valid_period @> now()",
    )
    .bind(ENVIRONMENT)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// The origin node after `after` in the scope, by id: a node that carries the declared
/// value its heirs resolve.
async fn next_origin(
    conn: &mut PgConnection,
    environment: Uuid,
    after: Option<Uuid>,
    scope: Option<&[Uuid]>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT o.origin_object_id FROM property_origin o
          WHERE o.property_id = $1 AND o.valid_period @> now()
            AND o.origin_object_id IS NOT NULL
            AND ($2::uuid IS NULL OR o.origin_object_id > $2)
            AND ($3::uuid[] IS NULL OR coalesce(
                    (SELECT d.id FROM addressing_domain d
                      WHERE d.id = o.origin_object_id AND d.valid_period @> now()),
                    (SELECT p.domain_id FROM prefix p
                      WHERE p.id = o.origin_object_id AND p.valid_period @> now()),
                    (SELECT a.domain_id FROM address a
                      WHERE a.id = o.origin_object_id AND a.valid_period @> now()))
                 = ANY($3))
          ORDER BY o.origin_object_id LIMIT 1",
    )
    .bind(environment)
    .bind(after)
    .bind(scope)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// The environments declared on `origin` now, `None` when none.
async fn declared(
    conn: &mut PgConnection,
    origin: Uuid,
    environment: Uuid,
) -> Result<Option<BTreeSet<String>>, StoreError> {
    // A closed list stores the identifiers of its values (REQ-003-3).
    let values: Option<Vec<String>> = sqlx::query_scalar(
        "SELECT array_agg(e.key) FROM property_assignment a
           JOIN property_enum_value e ON e.id = a.v_uuid OR e.id = ANY(a.v_uuids)
          WHERE a.object_id = $1 AND a.property_id = $2 AND a.kind = 'value'
            AND a.valid_period @> now()",
    )
    .bind(origin)
    .bind(environment)
    .fetch_one(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(values.map(|v| v.into_iter().collect()))
}

/// The heirs of `origin` after `after`, by id, `limit` at most (the origin among them).
async fn heirs(
    conn: &mut PgConnection,
    environment: Uuid,
    origin: Uuid,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT object_id FROM property_origin
          WHERE property_id = $1 AND origin_object_id = $2 AND valid_period @> now()
            AND ($3::uuid IS NULL OR object_id > $3)
          ORDER BY object_id LIMIT $4",
    )
    .bind(environment)
    .bind(origin)
    .bind(after)
    .bind(limit)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The views of the nodes of the addressing tree among `ids`, as the inference reads them;
/// other carriers (a machine) have none.
async fn views(conn: &mut PgConnection, ids: &[Uuid]) -> Result<Vec<ObjectView>, StoreError> {
    let rows: Vec<(Uuid, String, Uuid, Option<String>)> = sqlx::query_as(
        "SELECT id, 'domain', id, NULL::text FROM addressing_domain
          WHERE id = ANY($1) AND valid_period @> now()
         UNION ALL
         SELECT id, 'prefix', domain_id, prefix::text FROM prefix
          WHERE id = ANY($1) AND valid_period @> now()
         UNION ALL
         SELECT id, 'address', domain_id, host(address) FROM address
          WHERE id = ANY($1) AND valid_period @> now()",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(rows
        .into_iter()
        .map(|(id, kind, domain, value)| ObjectView {
            id,
            kind,
            domain,
            value,
            properties: std::collections::BTreeMap::new(),
        })
        .collect())
}
