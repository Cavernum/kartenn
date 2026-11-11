//! What a business module needs to serve its own routes under `/v1` with the conventions of
//! the API (spec 008, design « Lectures et routes »; ADR 0026, clause 9): reads dated,
//! paged, under the rights of the caller and journaled; writes by changeset, in JSON Merge
//! Patch under `If-Match`, answered as the routes of the core; the pieces of the contract
//! built by hand.

use std::collections::HashMap;

use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use kartenn_core::rules::{CHANGESET_CONFLICT, Rule};
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::module::ObjectType;
use kartenn_service::operations::Ref;
use kartenn_service::reads::objects_read;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, RequestError};
use kartenn_store_pg::rbac::ReadScope;
use kartenn_store_pg::read;
use serde_json::{Map, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::changesets::{Answer, Location, Pointers, idempotency_key, period, submit_answered};
pub use crate::cursor::{Page, PageParams, next_href, respond_page};
pub use crate::dated::{Dates, DatesParams, ETag, Moment, VERSION, respond, respond_object};
pub use crate::filter::{ListQuery, collection};
pub use crate::objects::{Api, Target};
use crate::objects::{audited, gate_target};
pub use crate::openapi::{
    Op, commanded, documented, listing_of, op, presented, queryable, schemas_of,
};
pub use crate::problem::{BatchViolation, Problem, ProblemKind};
pub use crate::resources::MERGE_PATCH;
use crate::resources::{WHOLE, entity_tag, invalid, members, reserved};

/// The prefix `q` of a suggestion in the filters of `page` (spec 018, REQ-018-23), as a
/// `LIKE` pattern on a lower-cased text, and the page sized for suggestions when there is
/// one; `q` stays among the filters, which the cursor keeps.
///
/// # Errors
///
/// 400 `bad-request` for a malformed `q` (see [`crate::filter::prefix`]).
pub fn suggestion(page: Page) -> Result<(Page, Option<String>), Problem> {
    let mut filters = page.filters.clone();
    match crate::filter::prefix(&mut filters)? {
        Some(prefix) => Ok((
            page.suggesting(),
            Some(kartenn_store_pg::list::like_prefix(&prefix)),
        )),
        None => Ok((page, None)),
    }
}

/// 400 `bad-request` with `detail`.
pub fn bad(detail: impl Into<String>) -> Problem {
    crate::objects::bad(detail)
}

/// A database error, as a problem (503 when unavailable).
pub fn store(error: &sqlx::Error) -> Problem {
    crate::objects::store(error)
}

/// The time of the database, before a read.
///
/// # Errors
///
/// The database unavailable.
pub async fn now(api: &Api) -> Result<OffsetDateTime, Problem> {
    read::now(&api.pool).await.map_err(|e| store(&e))
}

/// What the caller may read at `at`, kind by kind and type by type (REQ-005-24): bound as
/// `ReadScope::to_json` to `kartenn_scopes`, it feeds
/// [`kartenn_store_pg::rbac::readable`].
///
/// # Errors
///
/// The authorizer unavailable.
pub async fn scope(
    api: &Api,
    principal: &Principal,
    at: OffsetDateTime,
) -> Result<ReadScope, Problem> {
    Ok(api
        .authorizer()
        .read_scope(&principal.subject(), at)
        .await?)
}

/// 404 for an object of `ty` the caller may not read, the refusal journaled (ADR 0021).
///
/// # Errors
///
/// 404 `not-found`; the authorizer or the journal unavailable.
pub async fn gate(
    api: &Api,
    context: &RequestContext,
    ty: &'static ObjectType,
    id: Uuid,
) -> Result<(), Problem> {
    gate_target(api, context, Target::Module(ty), id).await
}

/// Journals the read of `ids`, objects of the type `kind`, under `criteria` (REQ-012-18).
///
/// # Errors
///
/// The journal unavailable: the read is not answered.
pub async fn record_read(
    api: &Api,
    context: &RequestContext,
    kind: &str,
    ids: &[Uuid],
    criteria: Value,
) -> Result<(), Problem> {
    let event = objects_read(context, kind, ids, criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(())
}

/// The description of the answer 200 of the read of an object, its `ETag` (REQ-006-14,
/// 006/H19).
pub const OBJECT_READ: &str = "The object. Its strong ETag is opaque: its version, then a \
    digest of everything else the body shows (summaries of linked objects, inherited values, \
    embedded collections), so that it changes whenever the body does.";

/// The description of the `If-Match` of a write of an object (REQ-006-14, 006/H19).
pub const IF_MATCH_OBJECT: &str = "The ETag of the object read. Only its version is compared: \
    a change of a linked object since the read does not refuse the write; the bare version \
    (\"<version>\") is accepted too.";

/// The version of the object `id` at `moment`, the own part of its strong `ETag`
/// (REQ-006-6, 14). Read in the [`Snapshot`] of the value served, it is the version of
/// that value (task 021-11d); [`read_one`] reads both.
///
/// # Errors
///
/// The database unavailable.
pub async fn version(
    executor: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    moment: &Moment,
) -> Result<Option<Uuid>, Problem> {
    read::version(executor, id, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))
}

/// The row of the object `id` that `sql` reads at `moment` (`$1` its `valid_at`, `$2` its
/// `as_of`, `$3` the identifier, as [`crate::dated`] relations take them) and its version,
/// both read in one [`Snapshot`]: an `If-Match` taken from the answer names the value it
/// shows, never a later one (task 021-11d). The snapshot ends before the caller's audit
/// event.
///
/// # Errors
///
/// 404 `not-found` when the object does not exist at `moment`; the database unavailable.
pub async fn read_one<R>(
    api: &Api,
    sql: String,
    id: Uuid,
    moment: &Moment,
) -> Result<(R, Uuid), Problem>
where
    R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    let mut snapshot = snapshot(api).await?;
    let row: Option<R> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(id)
        .fetch_optional(&mut *snapshot)
        .await
        .map_err(|e| store(&e))?;
    let found = match row {
        Some(row) => version(&mut *snapshot, id, moment)
            .await?
            .map(|version| (row, version)),
        None => None,
    };
    release(snapshot);
    found.ok_or_else(|| Problem::new(ProblemKind::NotFound))
}

/// A read-only snapshot of the database ([`read::consistent`]): a page and the versions of
/// its objects read in it are of the same instant, so that a `version` names the value
/// served and an `If-Match` taken from the page cannot pass on a change it did not show
/// (tasks 021-11a, 11c).
pub type Snapshot = sqlx::Transaction<'static, sqlx::Postgres>;

/// A new [`Snapshot`], begun before the reads of a page: the round trip of its `BEGIN`.
///
/// # Errors
///
/// The database unavailable.
pub async fn snapshot(api: &Api) -> Result<Snapshot, Problem> {
    read::consistent(&api.pool).await.map_err(|e| store(&e))
}

/// Ends `snapshot` once its reads are done, before the audit event of the page. It wrote
/// nothing: it is dropped, its `ROLLBACK` queued; its connection goes back to the pool in a
/// task of its own, whose ping sends that `ROLLBACK` while the request goes on (its audit
/// event, its answer), where a `COMMIT` would make the request wait one more round trip.
pub fn release(snapshot: Snapshot) {
    drop(snapshot);
}

/// The own versions of the objects `ids` at `moment` (REQ-006-14, 006/H19), those their
/// reads by identifier state, in one read for a page; read in the [`Snapshot`] of the
/// page, they are those of the values it serves. An object absent at `moment` has none.
///
/// # Errors
///
/// The database unavailable.
pub async fn versions(
    executor: impl sqlx::PgExecutor<'_>,
    moment: &Moment,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, Uuid>, Problem> {
    read::versions(executor, ids, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))
}

/// `items`, the objects of a page, each with its own version among `versions` (read by
/// [`versions`]) as `version`. An item without an `id`, or without a version, is left as
/// it is.
///
/// # Panics
///
/// Never: a value made of serialisable JSON members serialises.
#[must_use]
pub fn versioned<T: serde::Serialize, S: std::hash::BuildHasher>(
    items: &[T],
    versions: &HashMap<Uuid, Uuid, S>,
) -> Vec<Value> {
    let id = |item: &Value| item.get("id").and_then(Value::as_str)?.parse::<Uuid>().ok();
    items
        .iter()
        .map(|item| serde_json::to_value(item).expect("JSON values serialise"))
        .map(|item| match id(&item).and_then(|id| versions.get(&id)) {
            Some(version) => crate::dated::with_version(&item, *version),
            None => item,
        })
        .collect()
}

/// The kind of the pipeline of `ty`.
fn kind_of(ty: &'static ObjectType) -> ObjectKind {
    ObjectKind::from_name(ty.kind).expect("a type of the registry or of the core")
}

/// The records of the objects served by an [`ObjectType`]: those of the modules
/// (`module_object`), and the VLANs of the core in their own tables (spec 026).
pub(crate) const TYPED_OBJECTS: &str = "(SELECT id, kind, valid_period, changeset_id
      FROM module_object
     UNION ALL SELECT id, 'vlan_group', valid_period, changeset_id FROM vlan_group
     UNION ALL SELECT id, 'vlan', valid_period, changeset_id FROM vlan)";

/// The members of a write of `ty` but its reserved ones, each one of `allowed` (400
/// otherwise), and its reserved members.
fn written(
    headers: &HeaderMap,
    raw: &[u8],
    media: &str,
    allowed: &[&str],
) -> Result<(crate::resources::Reserved, Map<String, Value>), Problem> {
    let mut members = members(headers, raw, media)?;
    let reserved = reserved(&mut members)?;
    if let Some(unknown) = members.keys().find(|n| !allowed.contains(&n.as_str())) {
        return Err(bad(format!("unknown member {unknown}")));
    }
    Ok((reserved, members))
}

/// The operation `<module>.<name>` of the members `fields`.
fn operation(name: &str, mut fields: Map<String, Value>) -> Result<Operation, Problem> {
    fields.insert("op".into(), Value::String(name.to_owned()));
    serde_json::from_value(Value::Object(fields)).map_err(|e| bad(format!("{name}: {e}")))
}

/// The object `id` of `ty`, read now under the rights of the caller: 404 if it is absent,
/// invisible or of another type.
async fn existing(
    api: &Api,
    context: &RequestContext,
    ty: &'static ObjectType,
    id: Uuid,
) -> Result<(), Problem> {
    gate(api, context, ty, id).await?;
    let found: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXISTS (SELECT 1 FROM {TYPED_OBJECTS} o
                         WHERE id = $1 AND kind = $2 AND valid_period @> now())"
    )))
    .bind(id)
    .bind(ty.kind)
    .fetch_one(&api.pool)
    .await
    .map_err(|e| store(&e))?;
    if found {
        Ok(())
    } else {
        Err(Problem::new(ProblemKind::NotFound))
    }
}

/// The version `If-Match` names, the own part of the composite tag of an object or a bare
/// version (REQ-006-14, 006/H19): absent or `*` → 428, several or malformed → 400, weak
/// or of another form → 412.
///
/// # Errors
///
/// As above.
pub fn if_match(headers: &HeaderMap) -> Result<Uuid, Problem> {
    crate::resources::if_match(headers)
}

/// The opaque part of the strong entity tag `If-Match` names, for a resource whose tag is
/// not a version (the matrix of a flow type, 010/S35): absent or `*` → 428 with `required`
/// as detail, several or malformed → 400, weak → 412.
///
/// # Errors
///
/// As above.
pub fn if_match_tag(headers: &HeaderMap, required: &str) -> Result<String, Problem> {
    entity_tag(headers, required)
}

/// `POST /v1/<collection>` of a module: the members of the body, each one of `allowed`,
/// make the operation `name` (`machines.create_machine`); 201 and `Location` to the object of
/// `ty` it creates.
///
/// # Errors
///
/// 400, 415 for the body; the problems of a changeset (REQ-006-21).
#[allow(clippy::too_many_arguments)] // the request, then what the route writes
pub async fn create(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    headers: &HeaderMap,
    raw: &Bytes,
    ty: &'static ObjectType,
    name: &str,
    allowed: &[&str],
) -> Result<Response, Problem> {
    let idempotency_key = idempotency_key(headers)?;
    let (reserved, members) = written(headers, raw, "application/json", allowed)?;
    let request = ChangesetRequest {
        author: principal.identity,
        motive: reserved.motive,
        period: period(
            api,
            reserved.valid_from.as_deref(),
            reserved.valid_to.as_deref(),
        )
        .await?,
        operations: vec![operation(name, members)?],
        simulate: false,
        idempotency_key,
        approval_self: context.approval_self,
    };
    let pointers = [WHOLE.to_owned()];
    let answer = Answer {
        pointers: Pointers::Members(&pointers),
        applied: StatusCode::CREATED,
        location: Some(Location::Module(ty)),
        precondition: None,
        shown: None,
    };
    submit_answered(api, context, &request, &answer).await
}

/// `PATCH /v1/<collection>/{id}` of a module, in JSON Merge Patch under `If-Match`: the
/// guard of the version, then the operation `name` of the members given, each one of
/// `allowed` and none `null`.
///
/// # Errors
///
/// 400, 415, 422 for the body; 428, 412 for `If-Match`; 404; the problems of a changeset.
#[allow(clippy::too_many_arguments)] // the request, then what the route writes
pub async fn modify(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    id: Uuid,
    headers: &HeaderMap,
    raw: &Bytes,
    ty: &'static ObjectType,
    name: &str,
    allowed: &[&str],
) -> Result<Response, Problem> {
    modify_nullable(
        api,
        principal,
        context,
        id,
        headers,
        raw,
        ty,
        (name, allowed, &[]),
    )
    .await
}

/// [`modify`], where the members of `nullable` may be `null`, which removes them (RFC 7396):
/// `write` is the operation, the members it allows, and those it lets remove.
///
/// # Errors
///
/// As [`modify`].
#[allow(clippy::too_many_arguments)] // the request, then what the route writes
pub async fn modify_nullable(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    id: Uuid,
    headers: &HeaderMap,
    raw: &Bytes,
    ty: &'static ObjectType,
    write: (&str, &[&str], &[&str]),
) -> Result<Response, Problem> {
    let (name, allowed, nullable) = write;
    let (reserved, mut members) = written(headers, raw, MERGE_PATCH, allowed)?;
    if let Some((member, _)) = members
        .iter()
        .find(|(n, v)| v.is_null() && !nullable.contains(&n.as_str()))
    {
        return Err(invalid(
            &format!("/{member}"),
            format!("{member} cannot be removed"),
        ));
    }
    let expected = if_match(headers)?;
    existing(api, context, ty, id).await?;
    members.insert("id".into(), serde_json::json!({ "id": id }));
    let request = ChangesetRequest {
        author: principal.identity,
        motive: reserved.motive,
        period: period(
            api,
            reserved.valid_from.as_deref(),
            reserved.valid_to.as_deref(),
        )
        .await?,
        operations: vec![
            Operation::Expect {
                kind: kind_of(ty),
                id: Ref::Id(id),
                expected,
            },
            operation(name, members)?,
        ],
        simulate: false,
        idempotency_key: None,
        approval_self: context.approval_self,
    };
    let pointers = [WHOLE.to_owned(), WHOLE.to_owned()];
    let answer = Answer {
        pointers: Pointers::Members(&pointers),
        applied: StatusCode::OK,
        location: None,
        precondition: Some(&CHANGESET_CONFLICT),
        shown: None,
    };
    submit_answered(api, context, &request, &answer).await
}

/// An operation `name` on the object `id` of `ty` under its `If-Match` (an attachment of an
/// interface, REQ-009-9): the members of a JSON body, each one of `allowed`, completed by
/// `fixed` (the object itself, a member of the path); an empty body has none. The guard
/// of the version, then the operation; `applied` on success.
///
/// # Errors
///
/// 400, 415 for the body; 428, 412 for `If-Match`; 404; the problems of a changeset.
#[allow(clippy::too_many_arguments)] // the request, then what the route writes
pub async fn act(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    id: Uuid,
    headers: &HeaderMap,
    raw: &Bytes,
    ty: &'static ObjectType,
    write: (&str, &[&str], Map<String, Value>),
    applied: StatusCode,
) -> Result<Response, Problem> {
    let (name, allowed, fixed) = write;
    let (reserved, mut members) = if raw.is_empty() {
        (
            crate::resources::Reserved {
                motive: String::new(),
                valid_from: None,
                valid_to: None,
            },
            Map::new(),
        )
    } else {
        written(headers, raw, "application/json", allowed)?
    };
    let expected = if_match(headers)?;
    existing(api, context, ty, id).await?;
    members.extend(fixed);
    let request = ChangesetRequest {
        author: principal.identity,
        motive: reserved.motive,
        period: period(
            api,
            reserved.valid_from.as_deref(),
            reserved.valid_to.as_deref(),
        )
        .await?,
        operations: vec![
            Operation::Expect {
                kind: kind_of(ty),
                id: Ref::Id(id),
                expected,
            },
            operation(name, members)?,
        ],
        simulate: false,
        idempotency_key: None,
        approval_self: context.approval_self,
    };
    let pointers = [WHOLE.to_owned(), WHOLE.to_owned()];
    let answer = Answer {
        pointers: Pointers::Members(&pointers),
        applied,
        location: None,
        precondition: Some(&CHANGESET_CONFLICT),
        shown: None,
    };
    submit_answered(api, context, &request, &answer).await
}

/// An operation `name` that names its objects by itself (a cell of a matrix, a value by its
/// key; REQ-010-27): the members of an optional JSON body, each one of `allowed`,
/// completed by `fixed` (members of the path). No version guard: the operation checks
/// the state it writes; 200 once applied, 202 when held for a decision.
///
/// # Errors
///
/// 400, 415 for the body; the problems of a changeset.
pub async fn submit(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    headers: &HeaderMap,
    raw: &Bytes,
    write: (&str, &[&str], Map<String, Value>),
) -> Result<Response, Problem> {
    submitted(api, principal, context, headers, raw, write, None).await
}

/// [`submit`] of an operation that checks by itself the entity tag `If-Match` named (the
/// matrix of a flow type, 010/S35, by [`if_match_tag`]; the version of an object named by
/// its key, task 021-11e, by [`if_match`]), carried in one of the members of `fixed`: a
/// violation of `stale` answers 412 instead of 422.
///
/// # Errors
///
/// 400, 415 for the body; 412 for `stale`; the problems of a changeset.
pub async fn submit_matched(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    headers: &HeaderMap,
    raw: &Bytes,
    write: (&str, &[&str], Map<String, Value>),
    stale: &'static Rule,
) -> Result<Response, Problem> {
    submitted(api, principal, context, headers, raw, write, Some(stale)).await
}

async fn submitted(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    headers: &HeaderMap,
    raw: &Bytes,
    write: (&str, &[&str], Map<String, Value>),
    precondition: Option<&'static Rule>,
) -> Result<Response, Problem> {
    let (name, allowed, fixed) = write;
    let (reserved, mut members) = if raw.is_empty() {
        (
            crate::resources::Reserved {
                motive: String::new(),
                valid_from: None,
                valid_to: None,
            },
            Map::new(),
        )
    } else {
        written(headers, raw, "application/json", allowed)?
    };
    members.extend(fixed);
    let request = ChangesetRequest {
        author: principal.identity,
        motive: reserved.motive,
        period: period(
            api,
            reserved.valid_from.as_deref(),
            reserved.valid_to.as_deref(),
        )
        .await?,
        operations: vec![operation(name, members)?],
        simulate: false,
        idempotency_key: idempotency_key(headers)?,
        approval_self: context.approval_self,
    };
    let pointers = [WHOLE.to_owned()];
    let answer = Answer {
        pointers: Pointers::Members(&pointers),
        applied: StatusCode::OK,
        location: None,
        precondition,
        shown: None,
    };
    submit_answered(api, context, &request, &answer).await
}

/// `DELETE /v1/<collection>/{id}` of a module under `If-Match`: closes the object, from
/// `valid_from` of an optional body `{motive, valid_from, cascade}` (REQ-008-10).
///
/// # Errors
///
/// 400, 415 for the body; 428, 412 for `If-Match`; 404; the problems of a changeset.
pub async fn close(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    id: Uuid,
    headers: &HeaderMap,
    raw: &Bytes,
    ty: &'static ObjectType,
) -> Result<Response, Problem> {
    let (reserved, mut members) = if raw.is_empty() {
        (
            crate::resources::Reserved {
                motive: String::new(),
                valid_from: None,
                valid_to: None,
            },
            Map::new(),
        )
    } else {
        written(headers, raw, "application/json", &["cascade"])?
    };
    if reserved.valid_to.is_some() {
        return Err(bad("a closure has no valid_to"));
    }
    let cascade = members
        .remove("cascade")
        .map(|v| crate::resources::typed::<bool>("cascade", v))
        .transpose()?
        .unwrap_or(false);
    let expected = if_match(headers)?;
    existing(api, context, ty, id).await?;
    let request = ChangesetRequest {
        author: principal.identity,
        motive: reserved.motive,
        period: period(api, reserved.valid_from.as_deref(), None).await?,
        operations: vec![Operation::Close {
            kind: kind_of(ty),
            id: Ref::Id(id),
            expected: Some(expected),
            cascade,
        }],
        simulate: false,
        idempotency_key: None,
        approval_self: context.approval_self,
    };
    let pointers = [WHOLE.to_owned()];
    let answer = Answer {
        pointers: Pointers::Members(&pointers),
        applied: StatusCode::OK,
        location: None,
        precondition: Some(&CHANGESET_CONFLICT),
        shown: None,
    };
    submit_answered(api, context, &request, &answer).await
}
