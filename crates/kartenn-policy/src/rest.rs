//! The routes of the module under `/v1` (feature `rest`; spec 010, REQ-010-27; design
//! « API »), mounted by kartenn-server: the objects of the policy as collections, dated,
//! paged and filtered, written by changeset; the matrix view of a flow type; the query of
//! compliance by `QUERY /v1/verdicts` and its simple form `GET /v1/verdicts`.
//!
//! The objects of the policy have no anchor (S15): they are read under
//! `policy.catalogue.read`, written under the administration of their family, through the
//! permissions of the types (ADR 0026, amendment 3). An object the caller may not read
//! answers as an absent one (ADR 0021). A verdict is answered under the rights of
//! [`crate::verdict::query`] (REQ-010-25): any other caller gets the 404 of an absent
//! object.

use std::collections::BTreeMap;

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, Request, State};
use axum::http::{HeaderMap, Method, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json};
use kartenn_rest::ModuleRouter;
use kartenn_rest::kit::{
    self, Api, Dates, DatesParams, ETag, ListQuery, MERGE_PATCH, Moment, Page, PageParams, Problem,
    ProblemKind, documented, listing_of, next_href, op, queryable, respond, respond_page,
    schemas_of,
};
use kartenn_service::audit::RequestContext;
use kartenn_service::authn::Principal;
use kartenn_service::module::ObjectType;
use kartenn_store_pg::rbac::readable;
use kartenn_store_pg::write::PgStore;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa::openapi::path::HttpMethod;
use utoipa::openapi::{RefOr, Schema};
use uuid::Uuid;

use crate::cache::SnapshotCache;
use crate::evaluate::{
    ConditionState, Decisive, Discard, Dossier, Evaluation, Outcome, Side, Synthesis,
    SynthesisVerdict, Verdict,
};
use crate::matrix::{MatrixView, fingerprint, view};
use crate::model::{AxisKey, AxisValue, DossierField, Key, Transport};
use crate::module::{
    AXIS, CONDITION, EXEMPT_ROLE, FLOW_TYPE, INFERENCE_RULE, PATTERN, RULE, SERVICE, SIGNAL, VALUE,
    object_type,
};
use crate::rules::MATRIX_STALE;
use crate::verdict::{
    Access, Answer, Empty, EndpointKind, EndpointRef, Object, Point, Query, QueryError, Source,
    query,
};

/// The snapshots of the policy, shared by the requests of the process (REQ-010-16).
fn cache() -> &'static SnapshotCache {
    crate::cache::shared()
}

// ---------------------------------------------------------------- collections

/// How a filter of a list compares its value to a member of an object.
#[derive(Debug, Clone, Copy)]
enum Compare {
    /// The member equals the value.
    Equal,
    /// The member is a list that holds the value.
    Holds,
}

/// A collection of the module: its type, its table, its order, its filters and its writes.
struct Collection {
    /// The type of its objects.
    kind: &'static str,
    /// Its path under `/v1`.
    name: &'static str,
    /// Its table, in the schema `policy`.
    table: &'static str,
    /// The expression of the order of a list, on the row `t`.
    order: &'static str,
    /// The objects it keeps of the table, on the row `t` (an environment is a value of
    /// `policy.environment`).
    only: Option<&'static str>,
    /// Its filters: parameter, member of the object, comparison, identifier or not.
    filters: &'static [(&'static str, &'static str, Compare, bool)],
    /// Its creation: the operation and the members of the body.
    create: Option<(&'static str, &'static [&'static str])>,
    /// Its modification: the operation and the members of the body.
    update: Option<(&'static str, &'static [&'static str])>,
    /// How an object closes.
    close: Close,
}

/// How an object of a collection closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Close {
    /// It does not, by these routes.
    Never,
    /// By the closure of the core (REQ-008-10), under `If-Match`.
    Object,
    /// By `policy.environment.close`, which names the environment by its key (S15).
    Environment,
}

const KEYED: &str = "t.key";

/// How the clients name and show the collection `name` (023/A5): the filters of its
/// natural key, the members of a table (the columns of its row, [`object_of`]). A value
/// of an axis has no filter of its own key: it is named by its UUID.
fn presentation(name: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match name {
        "policy-axes" => (&["property"], &["property_key", "carrier", "inference"]),
        // The labels of a value are those of its closed-list value (spec 018, Q-018-7).
        "policy-values" => (&[], &["property_key", "value_key", "rank", "transverse"]),
        "environments" => (&["key"], &["value_key", "rank", "transverse"]),
        "services" => (&["key"], &["key", "labels", "flow_type_id"]),
        "policy-rules" => (&["key"], &["key", "effect", "labels"]),
        "admissibility-conditions" => (&["key"], &["key", "kind"]),
        "risk-signals" => (&["key"], &["key", "kind", "weight"]),
        "policy-exempt-roles" => (&["role"], &["role_key"]),
        "inference-rules" => (&["key"], &["key", "axis_key", "priority", "domain_id"]),
        // flow-types, policy-patterns
        _ => (&["key"], &["key", "labels"]),
    }
}

static COLLECTIONS: &[Collection] = &[
    Collection {
        kind: AXIS,
        name: "policy-axes",
        table: "policy_axis",
        order: "t.property_key",
        only: None,
        filters: &[("property", "property_key", Compare::Equal, false)],
        create: Some((
            "policy.axis.designate",
            &["property", "carrier", "inference"],
        )),
        update: Some(("policy.axis.update", &["carrier", "inference"])),
        close: Close::Object,
    },
    Collection {
        kind: VALUE,
        name: "policy-values",
        table: "policy_value",
        order: "t.property_key || '/' || t.value_key",
        only: None,
        filters: &[("axis", "property_key", Compare::Equal, false)],
        create: Some((
            "policy.value.describe",
            &[
                "property",
                "value",
                "labels",
                "description",
                "transverse",
                "rank",
            ],
        )),
        update: None,
        close: Close::Never,
    },
    Collection {
        kind: VALUE,
        name: "environments",
        table: "policy_value",
        order: "t.value_key",
        only: Some("t.property_key = 'policy.environment'"),
        filters: &[("key", "value_key", Compare::Equal, false)],
        create: Some((
            "policy.environment.create",
            &["key", "labels", "description", "transverse", "rank"],
        )),
        update: None,
        close: Close::Environment,
    },
    Collection {
        kind: FLOW_TYPE,
        name: "flow-types",
        table: "policy_flow_type",
        order: KEYED,
        only: None,
        filters: &[("key", "key", Compare::Equal, false)],
        create: Some(("policy.flow_type.create", &["key", "labels", "description"])),
        update: Some(("policy.flow_type.update", &["labels", "description"])),
        close: Close::Object,
    },
    Collection {
        kind: SERVICE,
        name: "services",
        table: "policy_service",
        order: KEYED,
        only: None,
        filters: &[
            ("key", "key", Compare::Equal, false),
            ("flow_type", "flow_type_id", Compare::Equal, true),
        ],
        create: Some((
            "policy.service.create",
            &["key", "labels", "flow_type", "entries"],
        )),
        update: Some(("policy.service.update", &["labels", "flow_type", "entries"])),
        close: Close::Object,
    },
    Collection {
        kind: PATTERN,
        name: "policy-patterns",
        table: "policy_pattern",
        order: KEYED,
        only: None,
        filters: &[("key", "key", Compare::Equal, false)],
        create: Some(("policy.pattern.create", &["key", "labels", "texts"])),
        update: Some(("policy.pattern.update", &["labels", "texts"])),
        close: Close::Object,
    },
    Collection {
        kind: RULE,
        name: "policy-rules",
        table: "policy_rule",
        order: KEYED,
        only: None,
        filters: &[
            ("key", "key", Compare::Equal, false),
            ("effect", "effect", Compare::Equal, false),
            ("pattern", "pattern_id", Compare::Equal, true),
            ("flow_type", "flow_type_ids", Compare::Holds, true),
            ("service", "service_ids", Compare::Holds, true),
        ],
        create: Some((
            "policy.rule.create",
            &[
                "key",
                "labels",
                "effect",
                "source",
                "destination",
                "same",
                "flow_types",
                "services",
                "pattern",
                "required_patterns",
                "texts",
            ],
        )),
        update: Some((
            "policy.rule.update",
            &[
                "labels",
                "effect",
                "source",
                "destination",
                "same",
                "flow_types",
                "services",
                "pattern",
                "required_patterns",
                "texts",
            ],
        )),
        close: Close::Object,
    },
    Collection {
        kind: CONDITION,
        name: "admissibility-conditions",
        table: "policy_condition",
        order: KEYED,
        only: None,
        filters: &[
            ("key", "key", Compare::Equal, false),
            ("kind", "kind", Compare::Equal, false),
        ],
        create: Some((
            "policy.condition.create",
            &[
                "key",
                "labels",
                "kind",
                "params",
                "rules",
                "flow_types",
                "texts",
            ],
        )),
        update: Some((
            "policy.condition.update",
            &["labels", "kind", "params", "rules", "flow_types", "texts"],
        )),
        close: Close::Object,
    },
    Collection {
        kind: SIGNAL,
        name: "risk-signals",
        table: "policy_signal",
        order: KEYED,
        only: None,
        filters: &[
            ("key", "key", Compare::Equal, false),
            ("kind", "kind", Compare::Equal, false),
        ],
        create: Some((
            "policy.signal.create",
            &["key", "labels", "kind", "params", "weight"],
        )),
        update: Some((
            "policy.signal.update",
            &["labels", "kind", "params", "weight"],
        )),
        close: Close::Object,
    },
    Collection {
        kind: EXEMPT_ROLE,
        name: "policy-exempt-roles",
        table: "policy_exempt_role",
        order: "t.role_key",
        only: None,
        filters: &[("role", "role_key", Compare::Equal, false)],
        create: Some(("policy.exempt_role.create", &["role"])),
        update: None,
        close: Close::Object,
    },
    Collection {
        kind: INFERENCE_RULE,
        name: "inference-rules",
        table: "policy_inference_rule",
        order: KEYED,
        only: None,
        filters: &[
            ("key", "key", Compare::Equal, false),
            ("axis", "axis_key", Compare::Equal, false),
            ("domain", "domain_id", Compare::Equal, true),
            ("parent", "parent_id", Compare::Equal, true),
        ],
        create: Some((
            "policy.inference_rule.create",
            &[
                "key", "axis", "parent", "priority", "criteria", "values", "domain",
            ],
        )),
        update: Some((
            "policy.inference_rule.update",
            &["priority", "criteria", "values"],
        )),
        close: Close::Object,
    },
];

/// The collection of the path `name`.
fn collection(name: &str) -> &'static Collection {
    COLLECTIONS
        .iter()
        .find(|c| c.name == name)
        .expect("a collection of the module")
}

/// The rows of `table` valid at `$1` as asserted at `$2` (now when null): its identifier,
/// its order and the object, its technical columns removed.
fn dated(c: &Collection) -> String {
    let table = c.table;
    let order = c.order;
    let only = c.only.map_or_else(String::new, |o| format!(" AND {o}"));
    let only_h = c
        .only
        .map_or_else(String::new, |o| format!(" AND {}", o.replace("t.", "h.")));
    let order_h = order.replace("t.", "h.");
    format!(
        "(SELECT t.id, ({order})::text AS sort, to_jsonb(t) AS body FROM policy.{table} t
           WHERE t.valid_period @> $1::timestamptz
             AND ($2::timestamptz IS NULL OR t.sys_start <= $2::timestamptz){only}
          UNION ALL
          SELECT h.id, ({order_h})::text, to_jsonb(h) FROM policy.{table}_history h
           WHERE $2::timestamptz IS NOT NULL AND h.valid_period @> $1::timestamptz
             AND h.sys_start <= $2::timestamptz AND h.sys_end > $2::timestamptz{only_h})"
    )
}

/// The object of a row as the routes show it: its period as two dates, without its
/// technical columns.
fn object_of(body: Value) -> Value {
    let Value::Object(mut row) = body else {
        return body;
    };
    for technical in ["sys_start", "sys_end", "changeset_id"] {
        row.remove(technical);
    }
    if let Some(Value::String(period)) = row.remove("valid_period") {
        let (from, to) = bounds(&period);
        row.insert("valid_from".into(), from.into());
        row.insert("valid_to".into(), to.into());
    }
    Value::Object(row)
}

/// The bounds of a range of PostgreSQL as text, `["a","b")`, each `None` when infinite.
fn bounds(period: &str) -> (Option<String>, Option<String>) {
    let inner = period
        .trim_start_matches(['[', '('])
        .trim_end_matches([']', ')']);
    let mut parts = inner.splitn(2, ',');
    let clean = |s: Option<&str>| {
        s.map(|s| s.trim().trim_matches('"').to_owned())
            .filter(|s| !s.is_empty() && s != "infinity" && s != "-infinity")
    };
    (clean(parts.next()), clean(parts.next()))
}

/// The condition that the row `x` of `kind` is readable at `$1` under the scope `s`.
fn visible(kind: &str) -> String {
    readable(
        "x.id",
        &format!("'{kind}'"),
        "(SELECT m.domain_id FROM module_object m
           WHERE m.id = x.id AND m.valid_period @> $1::timestamptz LIMIT 1)",
        "NULL::inet",
        "$1",
    )
}

/// The filters of `page`, each one of `c`, once.
fn wanted<'a>(c: &Collection, page: &'a Page) -> Result<BTreeMap<&'a str, String>, Problem> {
    let mut found = BTreeMap::new();
    for (name, value) in &page.filters {
        let Some((_, _, _, id)) = c.filters.iter().find(|f| f.0 == name) else {
            return Err(kit::bad(format!("unknown parameter {name}")));
        };
        let value = if *id {
            value
                .parse::<Uuid>()
                .map_err(|_| kit::bad(format!("{name} is not an identifier")))?
                .to_string()
        } else {
            value.clone()
        };
        if found.insert(name.as_str(), value).is_some() {
            return Err(kit::bad(format!("{name} is given more than once")));
        }
    }
    Ok(found)
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&time::format_description::well_known::Rfc3339)
        .expect("an instant formats")
}

/// `GET`/`QUERY /v1/<collection>` of the module: by order, filtered in conjunction.
async fn list(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    query: &str,
    headers: &HeaderMap,
    c: &'static Collection,
) -> Result<Response, Problem> {
    let page = Page::from_query(query, &api.ring, c.name)?;
    let filters = wanted(c, &page)?;
    let after = page
        .after()
        .map(|key| serde_json::from_value::<(String, Uuid)>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))?;
    let now = kit::now(api).await?;
    let moment = page.dates.at(now, api.bound)?;
    let scope = kit::scope(api, principal, now).await?;
    let mut conditions = Vec::new();
    let mut values = Vec::new();
    for (name, value) in &filters {
        let (_, member, compare, _) = c
            .filters
            .iter()
            .find(|f| f.0 == *name)
            .expect("a filter checked");
        let n = 4 + values.len();
        conditions.push(match compare {
            Compare::Equal => format!("x.body->>'{member}' = ${n}"),
            Compare::Holds => format!("x.body->'{member}' ? ${n}"),
        });
        values.push(value.clone());
    }
    let n = 4 + values.len();
    let filtered = if conditions.is_empty() {
        String::new()
    } else {
        format!(" AND {}", conditions.join(" AND "))
    };
    let sql = format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT x.id, x.sort, x.body FROM {} x
          WHERE (${n}::text IS NULL OR (x.sort, x.id) > (${n}, ${}::uuid)){filtered}
            AND ($3::jsonb IS NULL OR {})
          ORDER BY x.sort, x.id
          LIMIT ${}",
        dated(c),
        n + 1,
        visible(c.kind),
        n + 2,
    );
    let mut statement = sqlx::query_as::<_, (Uuid, String, Value)>(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(scope.to_json());
    for value in values {
        statement = statement.bind(value);
    }
    let mut rows = statement
        .bind(after.as_ref().map(|(sort, _)| sort.clone()))
        .bind(after.as_ref().map(|(_, id)| *id))
        .bind(i64::from(page.size) + 1)
        .fetch_all(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?;
    let size = usize::from(page.size);
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let last = &rows[size - 1];
        let cursor = page.next(&api.ring, c.name, &moment, json!([last.1, last.0]));
        next_href(&format!("/v1/{}", c.name), &cursor)
    });
    let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    kit::record_read(
        api,
        context,
        c.kind,
        &ids,
        json!({
            "collection": c.name,
            "filters": page.filters,
            "valid_at": rfc3339(moment.valid_at),
            "as_of": rfc3339(moment.as_of()),
        }),
    )
    .await?;
    let items: Vec<Value> = rows.into_iter().map(|r| object_of(r.2)).collect();
    let items = kit::versioned(api, &moment, &items).await?;
    Ok(respond_page(headers, &moment, &items, next.as_deref()))
}

/// The object `id` of `c` at `moment`.
async fn row_at(api: &Api, c: &Collection, id: Uuid, moment: &Moment) -> Result<Value, Problem> {
    let sql = format!("SELECT x.body FROM {} x WHERE x.id = $3", dated(c));
    let body: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(id)
        .fetch_optional(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    Ok(object_of(body))
}

/// `GET /v1/<collection>/{id}`: the object, its version as `ETag`, the read journaled.
async fn show(
    api: &Api,
    context: &RequestContext,
    id: Uuid,
    dates: Dates,
    headers: &HeaderMap,
    c: &'static Collection,
) -> Result<Response, Problem> {
    kit::gate(api, context, object_type(c.kind), id).await?;
    let now = kit::now(api).await?;
    let moment = dates.at(now, api.bound)?;
    let body = row_at(api, c, id, &moment).await?;
    let version = kit::version(api, id, &moment)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    kit::record_read(api, context, c.kind, &[id], json!({"id": id})).await?;
    Ok(kit::respond_object(headers, &moment, version, &body))
}

/// `DELETE /v1/environments/{id}`: `policy.environment.close` of its key, under
/// `If-Match`.
async fn close_environment(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    id: Uuid,
    headers: &HeaderMap,
    raw: &Bytes,
) -> Result<Response, Problem> {
    let c = collection("environments");
    kit::gate(api, context, object_type(c.kind), id).await?;
    let now = kit::now(api).await?;
    let moment = Dates::default().at(now, api.bound)?;
    let body = row_at(api, c, id, &moment).await?;
    let expected = kit::if_match(headers)?;
    let current = kit::version(api, id, &moment).await?;
    if current != Some(expected) {
        return Err(Problem::new(ProblemKind::PreconditionFailed));
    }
    let key = body
        .get("value_key")
        .cloned()
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let mut fixed = Map::new();
    fixed.insert("key".into(), key);
    kit::submit(
        api,
        principal,
        context,
        headers,
        raw,
        ("policy.environment.close", &[], fixed),
    )
    .await
}

// ---------------------------------------------------------------- the matrix

/// A cell of the matrix view (REQ-010-34).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyMatrixCell)]
pub struct CellDto {
    /// The environment of the source.
    source: String,
    /// The environment of the destination.
    destination: String,
    /// `allow`, `deny` or `conditional`.
    state: String,
    /// The patterns of a conditional authorisation.
    patterns: Vec<String>,
    /// The rules the cell stands for.
    rules: Vec<String>,
}

/// The matrix view of a flow type (REQ-010-34).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyMatrix)]
pub struct MatrixDto {
    /// The flow type.
    flow_type: String,
    /// Its cells; a cell absent is denied by default (S8).
    cells: Vec<CellDto>,
    /// Rules of the flow type the view does not show (more than one axis, more than one
    /// value).
    outside: usize,
}

fn matrix_dto(matrix: MatrixView) -> MatrixDto {
    MatrixDto {
        flow_type: matrix.flow_type.to_string(),
        cells: matrix
            .cells
            .into_iter()
            .map(|cell| CellDto {
                source: cell.source.to_string(),
                destination: cell.destination.to_string(),
                state: cell.state.as_str().to_owned(),
                patterns: cell.patterns.iter().map(ToString::to_string).collect(),
                rules: cell.rules.iter().map(ToString::to_string).collect(),
            })
            .collect(),
        outside: matrix.outside,
    }
}

/// The flow type of key `key` at `moment`: its identifier, 404 if it is absent or the caller
/// may not read the policy.
async fn flow_type_at(
    api: &Api,
    context: &RequestContext,
    key: &str,
    moment: &Moment,
) -> Result<Uuid, Problem> {
    let c = collection("flow-types");
    let sql = format!("SELECT x.id FROM {} x WHERE x.body->>'key' = $3", dated(c));
    let id: Uuid = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(key)
        .fetch_optional(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    kit::gate(api, context, object_type(FLOW_TYPE), id).await?;
    Ok(id)
}

/// The parameters of the matrix view.
#[derive(Debug, Deserialize)]
struct MatrixParams {
    flow_type: String,
}

/// `GET /v1/policy-matrix?flow_type=…`: the view of the rules of a flow type between
/// environments (REQ-010-34).
async fn show_matrix(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let raw = raw.unwrap_or_default();
    let mut flow_type = None;
    let mut dates = Map::new();
    for (name, value) in form_urlencoded_pairs(&raw) {
        match name.as_str() {
            "flow_type" if flow_type.is_none() => flow_type = Some(value),
            "valid_at" | "as_of" if !dates.contains_key(&name) => {
                dates.insert(name, Value::String(value));
            }
            other => return Err(kit::bad(format!("unknown or repeated parameter {other}"))),
        }
    }
    let params = MatrixParams {
        flow_type: flow_type.ok_or_else(|| kit::bad("flow_type is required"))?,
    };
    let key = Key::new(&params.flow_type).map_err(|_| kit::bad("flow_type is not a key"))?;
    let dates = dates_of(&dates)?;
    let now = kit::now(&api).await?;
    let moment = dates.at(now, api.bound)?;
    let id = flow_type_at(&api, &context, key.as_str(), &moment).await?;
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let snapshot = cache()
        .snapshot(&mut conn, moment.valid_at, moment.as_of)
        .await
        .map_err(Problem::from)?;
    kit::record_read(
        &api,
        &context,
        FLOW_TYPE,
        &[id],
        json!({"matrix": key.as_str()}),
    )
    .await?;
    let matrix = view(&snapshot, &key);
    // Strong: the fingerprint fixes the bytes served, and a write names it (S35).
    let tag = ETag::fingerprint(fingerprint(&matrix));
    Ok(respond(&headers, &moment, &tag, Json(matrix_dto(matrix))))
}

/// The members a write of a cell may carry (REQ-010-34).
const CELL_MEMBERS: &[&str] = &["state", "patterns", "texts", "labels"];

/// `PUT /v1/policy-matrix/{source}/{flow_type}/{destination}`: `policy.matrix.set` of the
/// cell, whose state the body gives (`allow`, `deny`, `conditional` or `none`), under
/// `If-Match` of the matrix of the flow type read at present (S35): 428 without, 412 when
/// the matrix changed since.
async fn set_cell(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path((source, flow_type, destination)): Path<(String, Uuid, String)>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let tag = kit::if_match_tag(
        &headers,
        "read the matrix of the flow type, then send its ETag in If-Match to set a cell",
    )?;
    let mut fixed = Map::new();
    fixed.insert("source".into(), source.into());
    fixed.insert("destination".into(), destination.into());
    fixed.insert("flow_type".into(), json!({ "id": flow_type }));
    fixed.insert("if_match".into(), tag.into());
    kit::submit_matched(
        &api,
        &principal,
        &context,
        &headers,
        &raw,
        ("policy.matrix.set", CELL_MEMBERS, fixed),
        &MATRIX_STALE,
    )
    .await
}

// ---------------------------------------------------------------- verdicts

/// An endpoint of a query (REQ-010-13).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = PolicyEndpoint)]
pub struct EndpointDto {
    /// `address`, `prefix`, `interface` or `machine`.
    kind: String,
    /// The object.
    id: Uuid,
}

/// The file joined to a query (REQ-010-38).
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = PolicyDossier)]
pub struct DossierDto {
    /// Fields present: `external_refs`, `requester`, `owners`, `data_nature`, `period`.
    #[serde(default)]
    fields: Vec<String>,
    /// Values of the file, by axis.
    #[serde(default)]
    values: BTreeMap<String, Vec<String>>,
}

/// The body of `QUERY /v1/verdicts` (design « API »): what is evaluated is one of a
/// service, a transport and a port, or a flow type.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = PolicyVerdictQuery)]
pub struct VerdictQueryDto {
    /// The source.
    source: EndpointDto,
    /// The destination.
    destination: EndpointDto,
    /// A service, by its key.
    #[serde(default)]
    service: Option<String>,
    /// A transport, with `port`.
    #[serde(default)]
    transport: Option<String>,
    /// A port, with `transport`.
    #[serde(default)]
    port: Option<u16>,
    /// A flow type, by its key.
    #[serde(default)]
    flow_type: Option<String>,
    /// The file, if any.
    #[serde(default)]
    dossier: Option<DossierDto>,
    /// The entity of the request, where `policy.flow.request` is looked for.
    #[serde(default)]
    entity: Option<Uuid>,
    /// Valid time (RFC 3339), now by default.
    #[serde(default)]
    valid_at: Option<String>,
    /// System time (RFC 3339), now by default.
    #[serde(default)]
    as_of: Option<String>,
}

/// The values of an endpoint on an axis, and where they come from (REQ-010-31).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyAxisResolution)]
pub struct AxisDto {
    /// The axis.
    axis: String,
    /// Its values, empty when not classified.
    values: Vec<String>,
    /// `tree` or `machine`.
    source: String,
    /// The object carrying the assignment (`tree`), absent in a reduced explanation.
    #[serde(skip_serializing_if = "Option::is_none")]
    origin: Option<Uuid>,
    /// The objects walked to the origin (`tree`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    chain: Vec<Uuid>,
    /// The machines carrying a value (`machine`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    machines: Vec<Uuid>,
}

/// A point of an endpoint: an address or a prefix and its values.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyPoint)]
pub struct PointDto {
    /// The address or the prefix; absent in a reduced explanation.
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Uuid>,
    /// Its values per axis.
    axes: Vec<AxisDto>,
}

/// What decided a verdict (REQ-010-12).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyDecisive)]
pub struct DecisiveDto {
    /// `unclassified`, `rule`, `pattern`, `pending`, `no_rule` or `unknown_service`.
    kind: String,
    /// The rule, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    rule: Option<String>,
    /// The pattern that matched (`pattern`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pattern: Option<String>,
    /// The patterns expected (`pending`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    expected: Vec<String>,
    /// The side not classified (`unclassified`).
    #[serde(skip_serializing_if = "Option::is_none")]
    side: Option<String>,
    /// The axis not classified (`unclassified`).
    #[serde(skip_serializing_if = "Option::is_none")]
    axis: Option<String>,
}

/// A value that made the decisive rule apply.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyCombinationValue)]
pub struct CombinationDto {
    /// `source` or `destination`.
    side: String,
    /// The axis.
    axis: String,
    /// The value.
    value: String,
}

/// A rule set aside, and why.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyDiscarded)]
pub struct DiscardedDto {
    /// The rule.
    rule: String,
    /// `deny_wins`, `less_specific` or `unconditional_first`.
    why: String,
}

/// An admissibility condition and its state (REQ-010-36).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyConditionResult)]
pub struct ConditionDto {
    /// The condition.
    condition: String,
    /// `met`, `unmet` or `not_evaluated`.
    state: String,
}

/// One evaluation: a pair of points and a service (REQ-010-11, 12, 37).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyEvaluation)]
pub struct EvaluationDto {
    /// Index of the source point.
    source: usize,
    /// Index of the destination point.
    destination: usize,
    /// The service; absent for a port no service covers.
    #[serde(skip_serializing_if = "Option::is_none")]
    service: Option<String>,
    /// `allowed`, `allowed_pending` or `denied`.
    verdict: String,
    /// What decided it.
    decisive: DecisiveDto,
    /// The values that made the decisive rule apply.
    combination: Vec<CombinationDto>,
    /// Applicable rules set aside, three at most.
    discarded: Vec<DiscardedDto>,
    /// Applicable conditions and their state.
    conditions: Vec<ConditionDto>,
    /// `automatic`, `review` or `noncompliant`.
    outcome: String,
    /// The risk signals raised.
    signals: Vec<String>,
    /// The informative score.
    score: i64,
}

/// The synthesis of the evaluations (REQ-010-15).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicySynthesis)]
pub struct SynthesisDto {
    /// `allowed`, `partial` or `denied`.
    verdict: String,
    /// Evaluations that allow.
    allowed: usize,
    /// Evaluations that deny.
    denied: usize,
    /// The worst outcome.
    outcome: String,
}

/// The answer of a query of compliance (REQ-010-13, 15, 25).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyVerdict)]
pub struct VerdictDto {
    /// `full` or `reduced`.
    access: String,
    /// Revision of the policy read.
    revision: u64,
    /// The points of the source.
    sources: Vec<PointDto>,
    /// The points of the destination.
    destinations: Vec<PointDto>,
    /// The evaluations.
    evaluations: Vec<EvaluationDto>,
    /// Their synthesis, absent without evaluation.
    #[serde(skip_serializing_if = "Option::is_none")]
    synthesis: Option<SynthesisDto>,
    /// `no_address` when an endpoint has no address at the date.
    #[serde(skip_serializing_if = "Option::is_none")]
    empty: Option<String>,
    /// Addresses left out, which the caller may not read one by one.
    hidden: usize,
}

const fn side(side: Side) -> &'static str {
    match side {
        Side::Source => "source",
        Side::Destination => "destination",
    }
}

const fn outcome(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Automatic => "automatic",
        Outcome::Review => "review",
        Outcome::Noncompliant => "noncompliant",
    }
}

fn decisive(decisive: Decisive) -> DecisiveDto {
    let mut dto = DecisiveDto {
        kind: String::new(),
        rule: None,
        pattern: None,
        expected: Vec::new(),
        side: None,
        axis: None,
    };
    match decisive {
        Decisive::Unclassified { side: s, axis } => {
            dto.kind = "unclassified".into();
            dto.side = Some(side(s).into());
            dto.axis = Some(axis.to_string());
        }
        Decisive::Rule(rule) => {
            dto.kind = "rule".into();
            dto.rule = Some(rule.to_string());
        }
        Decisive::Pattern { pattern, rule } => {
            dto.kind = "pattern".into();
            dto.pattern = Some(pattern.to_string());
            dto.rule = Some(rule.to_string());
        }
        Decisive::Pending { rule, expected } => {
            dto.kind = "pending".into();
            dto.rule = Some(rule.to_string());
            dto.expected = expected.iter().map(ToString::to_string).collect();
        }
        Decisive::NoRule => dto.kind = "no_rule".into(),
        Decisive::UnknownService => dto.kind = "unknown_service".into(),
    }
    dto
}

fn evaluation(source: usize, destination: usize, e: Evaluation) -> EvaluationDto {
    EvaluationDto {
        source,
        destination,
        service: e.service.map(|s| s.to_string()),
        verdict: match e.verdict {
            Verdict::Allowed => "allowed",
            Verdict::AllowedPending => "allowed_pending",
            Verdict::Denied => "denied",
        }
        .into(),
        decisive: decisive(e.decisive),
        combination: e
            .combination
            .into_iter()
            .map(|((s, axis), value)| CombinationDto {
                side: side(s).into(),
                axis: axis.to_string(),
                value: value.to_string(),
            })
            .collect(),
        discarded: e
            .discarded
            .into_iter()
            .map(|d| DiscardedDto {
                rule: d.rule.to_string(),
                why: match d.why {
                    Discard::DenyWins => "deny_wins",
                    Discard::LessSpecific => "less_specific",
                    Discard::UnconditionalFirst => "unconditional_first",
                }
                .into(),
            })
            .collect(),
        conditions: e
            .conditions
            .into_iter()
            .map(|c| ConditionDto {
                condition: c.condition.to_string(),
                state: match c.state {
                    ConditionState::Met => "met",
                    ConditionState::Unmet => "unmet",
                    ConditionState::NotEvaluated => "not_evaluated",
                }
                .into(),
            })
            .collect(),
        outcome: outcome(e.outcome).into(),
        signals: e.signals.iter().map(ToString::to_string).collect(),
        score: e.score,
    }
}

fn point(point: Point) -> PointDto {
    PointDto {
        id: point.id,
        axes: point
            .axes
            .into_iter()
            .map(|a| {
                let mut dto = AxisDto {
                    axis: a.axis.to_string(),
                    values: a.values.iter().map(ToString::to_string).collect(),
                    source: String::new(),
                    origin: None,
                    chain: Vec::new(),
                    machines: Vec::new(),
                };
                match a.source {
                    Source::Tree { origin, chain } => {
                        dto.source = "tree".into();
                        dto.origin = origin;
                        dto.chain = chain;
                    }
                    Source::Machine { machines } => {
                        dto.source = "machine".into();
                        dto.machines = machines;
                    }
                }
                dto
            })
            .collect(),
    }
}

fn synthesis(s: Synthesis) -> SynthesisDto {
    SynthesisDto {
        verdict: match s.verdict {
            SynthesisVerdict::Allowed => "allowed",
            SynthesisVerdict::Partial => "partial",
            SynthesisVerdict::Denied => "denied",
        }
        .into(),
        allowed: s.allowed,
        denied: s.denied,
        outcome: outcome(s.outcome).into(),
    }
}

fn verdict_dto(answer: Answer) -> VerdictDto {
    VerdictDto {
        access: match answer.access {
            Access::Full => "full",
            Access::Reduced => "reduced",
        }
        .into(),
        revision: answer.revision,
        sources: answer.sources.into_iter().map(point).collect(),
        destinations: answer.destinations.into_iter().map(point).collect(),
        evaluations: answer
            .evaluations
            .into_iter()
            .map(|p| evaluation(p.source, p.destination, p.evaluation))
            .collect(),
        synthesis: answer.synthesis.map(synthesis),
        empty: answer.empty.map(|e| match e {
            Empty::NoAddress => "no_address".to_owned(),
        }),
        hidden: answer.hidden,
    }
}

fn endpoint(name: &str, dto: &EndpointDto) -> Result<EndpointRef, Problem> {
    let kind = match dto.kind.as_str() {
        "address" => EndpointKind::Address,
        "prefix" => EndpointKind::Prefix,
        "interface" => EndpointKind::Interface,
        "machine" => EndpointKind::Machine,
        "range" => {
            return Err(Problem::new(ProblemKind::InvalidMember).detail(format!(
                "{name}: a range carries no classification (policy.query.unsupported_endpoint)"
            )));
        }
        other => return Err(kit::bad(format!("{name}: unknown kind {other}"))),
    };
    Ok(EndpointRef { kind, id: dto.id })
}

fn dossier(dto: DossierDto) -> Result<Dossier, Problem> {
    let mut dossier = Dossier::default();
    for field in dto.fields {
        dossier.fields.insert(match field.as_str() {
            "external_refs" => DossierField::ExternalRefs,
            "requester" => DossierField::Requester,
            "owners" => DossierField::Owners,
            "data_nature" => DossierField::DataNature,
            "period" => DossierField::Period,
            other => return Err(kit::bad(format!("dossier: unknown field {other}"))),
        });
    }
    for (axis, values) in dto.values {
        let axis =
            AxisKey::parse(&axis).map_err(|_| kit::bad(format!("dossier: {axis} is no axis")))?;
        dossier
            .values
            .insert(axis, values.into_iter().map(AxisValue::new).collect());
    }
    Ok(dossier)
}

fn instant(name: &str, text: &str) -> Result<OffsetDateTime, Problem> {
    OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339)
        .map_err(|_| kit::bad(format!("{name} is not an RFC 3339 date with its offset")))
}

/// The dates of `members` (`valid_at`, `as_of`), as a read takes them.
fn dates_of(members: &Map<String, Value>) -> Result<Dates, Problem> {
    let mut dates = Dates::default();
    if let Some(Value::String(text)) = members.get("valid_at") {
        dates.valid_at = Some(instant("valid_at", text)?);
    }
    if let Some(Value::String(text)) = members.get("as_of") {
        dates.as_of = Some(instant("as_of", text)?);
    }
    Ok(dates)
}

/// The query of `dto` at `moment`.
fn query_of(dto: VerdictQueryDto, moment: &Moment) -> Result<Query, Problem> {
    let key = |name: &str, text: &str| {
        Key::new(text).map_err(|_| kit::bad(format!("{name} is not a key")))
    };
    let object = match (&dto.service, &dto.transport, dto.port, &dto.flow_type) {
        (Some(service), None, None, None) => Object::Service(key("service", service)?),
        (None, Some(transport), Some(port), None) => Object::Port(
            Transport::parse(transport)
                .ok_or_else(|| kit::bad(format!("unknown transport {transport}")))?,
            port,
        ),
        (None, None, None, Some(flow_type)) => Object::FlowType(key("flow_type", flow_type)?),
        _ => {
            return Err(kit::bad(
                "one of service, transport with port, or flow_type is required",
            ));
        }
    };
    Ok(Query {
        source: endpoint("source", &dto.source)?,
        destination: endpoint("destination", &dto.destination)?,
        object,
        dossier: dto.dossier.map(dossier).transpose()?,
        entity: dto.entity,
        valid_at: moment.valid_at,
        as_of: moment.as_of,
    })
}

/// Answers the query of compliance `dto` for the caller (REQ-010-13, 25).
async fn answer(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    dto: VerdictQueryDto,
) -> Result<Response, Problem> {
    let mut dates = Map::new();
    if let Some(at) = &dto.valid_at {
        dates.insert("valid_at".into(), at.clone().into());
    }
    if let Some(at) = &dto.as_of {
        dates.insert("as_of".into(), at.clone().into());
    }
    let now = kit::now(api).await?;
    let moment = dates_of(&dates)?.at(now, api.bound)?;
    let q = query_of(dto, &moment)?;
    let reader = PgStore::new(api.pool.clone()).with_modules(api.modules.clone());
    let authorizer = api.authorizer();
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let subject = principal.subject();
    let answered = query(&mut conn, &reader, &authorizer, cache(), Some(&subject), &q).await;
    let answer = match answered {
        Ok(answer) => answer,
        Err(QueryError::NotFound) => return Err(Problem::new(ProblemKind::NotFound)),
        Err(QueryError::TooLarge(detail)) => {
            return Err(Problem::new(ProblemKind::PolicyQueryTooLarge).detail(detail));
        }
        Err(QueryError::Store(e)) => return Err(Problem::from(e)),
    };
    let ids = [q.source.id, q.destination.id];
    kit::record_read(
        api,
        context,
        crate::module::RULE,
        &[],
        json!({ "verdict": ids, "valid_at": rfc3339(moment.valid_at) }),
    )
    .await?;
    let mut response = Json(verdict_dto(answer)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

/// The pairs of a query string, decoded.
fn form_urlencoded_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(name), decode(value))
        })
        .collect()
}

/// A component of a query string, `+` and percent-escapes decoded.
fn decode(text: &str) -> String {
    let bytes = text.replace('+', " ").into_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// An endpoint of the query string, `<kind>:<id>`.
fn endpoint_param(name: &str, text: &str) -> Result<EndpointDto, Problem> {
    let (kind, id) = text
        .split_once(':')
        .ok_or_else(|| kit::bad(format!("{name} is <kind>:<id>")))?;
    Ok(EndpointDto {
        kind: kind.to_owned(),
        id: id
            .parse()
            .map_err(|_| kit::bad(format!("{name}: not an identifier")))?,
    })
}

/// `GET /v1/verdicts?source=<kind>:<id>&destination=…&service=…`: the simple form of the
/// query (design « API »).
async fn verdicts_get(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    RawQuery(raw): RawQuery,
) -> Result<Response, Problem> {
    let mut seen = Map::new();
    for (name, value) in form_urlencoded_pairs(&raw.unwrap_or_default()) {
        const KNOWN: &[&str] = &[
            "source",
            "destination",
            "service",
            "transport",
            "port",
            "flow_type",
            "entity",
            "valid_at",
            "as_of",
        ];
        if !KNOWN.contains(&name.as_str()) {
            return Err(kit::bad(format!("unknown parameter {name}")));
        }
        if seen.insert(name.clone(), Value::String(value)).is_some() {
            return Err(kit::bad(format!("{name} is given more than once")));
        }
    }
    let text = |name: &str| seen.get(name).and_then(Value::as_str).map(str::to_owned);
    let dto = VerdictQueryDto {
        source: endpoint_param(
            "source",
            &text("source").ok_or_else(|| kit::bad("source is required"))?,
        )?,
        destination: endpoint_param(
            "destination",
            &text("destination").ok_or_else(|| kit::bad("destination is required"))?,
        )?,
        service: text("service"),
        transport: text("transport"),
        port: text("port")
            .map(|p| p.parse().map_err(|_| kit::bad("port is a number")))
            .transpose()?,
        flow_type: text("flow_type"),
        dossier: None,
        entity: text("entity")
            .map(|e| {
                e.parse()
                    .map_err(|_| kit::bad("entity is not an identifier"))
            })
            .transpose()?,
        valid_at: text("valid_at"),
        as_of: text("as_of"),
    };
    answer(&api, &principal, &context, dto).await
}

/// `QUERY /v1/verdicts` (RFC 10008) with the body [`VerdictQueryDto`]; any other method but
/// `GET` and `HEAD` → 405.
async fn verdicts_query(State(api): State<Api>, request: Request) -> Response {
    let query_method = Method::from_bytes(b"QUERY").expect("QUERY is a token");
    if request.method() != query_method {
        return (
            [(
                header::ALLOW,
                axum::http::HeaderValue::from_static("GET, HEAD, QUERY"),
            )],
            Problem::new(ProblemKind::MethodNotAllowed).detail("allowed: GET, HEAD, QUERY"),
        )
            .into_response();
    }
    let json = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json {
        return Problem::new(ProblemKind::UnsupportedMediaType)
            .detail("a QUERY takes application/json")
            .into_response();
    }
    let (parts, body) = request.into_parts();
    let (Some(principal), Some(context)) = (
        parts.extensions.get::<Principal>().cloned(),
        parts.extensions.get::<RequestContext>().cloned(),
    ) else {
        return Problem::new(ProblemKind::Unauthenticated).into_response();
    };
    let Ok(body) = axum::body::to_bytes(body, usize::MAX).await else {
        return Problem::new(ProblemKind::BadRequest).into_response();
    };
    let dto: VerdictQueryDto = match serde_json::from_slice(&body) {
        Ok(dto) => dto,
        Err(e) => return kit::bad(format!("the body: {e}")).into_response(),
    };
    match answer(&api, &principal, &context, dto).await {
        Ok(response) => response,
        Err(problem) => problem.into_response(),
    }
}

// ---------------------------------------------------------------- reports

/// The permission of the audit report: it names nodes of the whole referential, so asking
/// for it and reading it are the administration of the rules (design « Droits »).
const REPORT_PERMISSION: &str = "policy.rule.admin";

/// Most items of a page of a rubric.
const ITEMS_PAGE: i64 = 100;

/// 404 unless the caller holds [`REPORT_PERMISSION`] (a global permission, checked on
/// `id`): a report is not disclosed to who may not ask for one.
async fn may_report(api: &Api, principal: &Principal, id: Uuid) -> Result<(), Problem> {
    use kartenn_service::authz::Authorizer as _;
    let lacking = api
        .authorizer()
        .authorize_many(&principal.subject(), REPORT_PERMISSION, &[id])
        .await
        .map_err(Problem::from)?;
    if lacking.is_empty() {
        Ok(())
    } else {
        Err(Problem::new(ProblemKind::NotFound))
    }
}

/// A report of the policy (REQ-010-23).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyReport)]
pub struct ReportDto {
    /// Identifier.
    id: Uuid,
    /// The date the report describes.
    valid_at: String,
    /// Who asked for it.
    requested_by: Option<Uuid>,
    /// When it was asked for.
    requested_at: String,
    /// `pending`, `running` or `done`.
    state: String,
    /// When it was over.
    done_at: Option<String>,
    /// The number of items of each rubric so far; `null` for a rubric the build cannot
    /// answer.
    counts: BTreeMap<String, Option<i64>>,
}

/// An item of a rubric of a report.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyReportItem)]
pub struct ItemDto {
    /// Rubric.
    rubric: String,
    /// Rank within the rubric, from 1.
    seq: i64,
    /// The object the item is about, if one.
    subject: Option<Uuid>,
    /// What the item says.
    detail: Value,
}

/// The body of `POST /v1/policy-reports`.
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = PolicyReportRequest)]
pub struct ReportRequestDto {
    /// The date to describe (RFC 3339), now by default.
    #[serde(default)]
    valid_at: Option<String>,
}

/// `POST /v1/policy-reports`: asks for a report at `valid_at` (REQ-010-23); 202 and its
/// `Location`, or 409 with the `Location` of the report being produced (one at a time).
async fn request_report(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    raw: Bytes,
) -> Result<Response, Problem> {
    let body: ReportRequestDto = if raw.is_empty() {
        ReportRequestDto::default()
    } else {
        serde_json::from_slice(&raw).map_err(|e| kit::bad(format!("the body: {e}")))?
    };
    may_report(&api, &principal, Uuid::nil()).await?;
    let valid_at = match body.valid_at {
        Some(text) => instant("valid_at", &text)?,
        None => kit::now(&api).await?,
    };
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let requested = crate::report::request(&mut conn, valid_at, Some(principal.identity))
        .await
        .map_err(Problem::from)?;
    let (status, id, problem) = match requested {
        crate::report::Requested::Started(id) => (axum::http::StatusCode::ACCEPTED, id, false),
        crate::report::Requested::Busy(id) => (axum::http::StatusCode::CONFLICT, id, true),
    };
    let location = format!("/v1/policy-reports/{id}");
    let mut response = if problem {
        Problem::new(ProblemKind::Conflict)
            .detail(format!("a report is being produced: {location}"))
            .into_response()
    } else {
        (status, Json(json!({ "id": id }))).into_response()
    };
    response.headers_mut().insert(
        header::LOCATION,
        axum::http::HeaderValue::from_str(&location).expect("a path is a header value"),
    );
    Ok(response)
}

fn report_dto(report: crate::report::Report) -> ReportDto {
    ReportDto {
        id: report.id,
        valid_at: rfc3339(report.valid_at),
        requested_by: report.requested_by,
        requested_at: rfc3339(report.requested_at),
        state: report.state,
        done_at: report.done_at.map(rfc3339),
        counts: report.counts,
    }
}

/// The name of the list of the reports, in its cursors.
const REPORTS: &str = "policy-reports";

/// `GET /v1/policy-reports`: the reports, the most recently asked for first, by pages
/// read at the current state, filtered by state, launcher and date of the request
/// (task 021-30); under the rights of a report.
async fn list_reports(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    may_report(&api, &principal, Uuid::nil()).await?;
    let page = Page::from_query(raw.as_deref().unwrap_or_default(), &api.ring, REPORTS)?;
    if page.after().is_none() && page.dates != Dates::default() {
        return Err(kit::bad("the reports take neither valid_at nor as_of"));
    }
    let mut filter = crate::report::ReportFilter::default();
    let once = |slot: bool, name: &str| {
        if slot {
            Err(kit::bad(format!("{name} is given more than once")))
        } else {
            Ok(())
        }
    };
    for (name, value) in &page.filters {
        match name.as_str() {
            "state" => match value.as_str() {
                "pending" | "running" | "done" => filter.states.push(value.clone()),
                other => return Err(kit::bad(format!("{other} is not a state of a report"))),
            },
            "requested_by" => {
                once(filter.requested_by.is_some(), name)?;
                filter.requested_by = Some(
                    value
                        .parse()
                        .map_err(|_| kit::bad("requested_by is not an identifier"))?,
                );
            }
            "from" => {
                once(filter.from.is_some(), name)?;
                filter.from = Some(instant(name, value)?);
            }
            "to" => {
                once(filter.to.is_some(), name)?;
                filter.to = Some(instant(name, value)?);
            }
            other => return Err(kit::bad(format!("unknown parameter {other}"))),
        }
    }
    filter.after = page
        .after()
        .map(|key| {
            let invalid = || Problem::new(ProblemKind::InvalidCursor);
            let (at, id) =
                serde_json::from_value::<(String, Uuid)>(key.clone()).map_err(|_| invalid())?;
            Ok::<_, Problem>((instant("cursor", &at).map_err(|_| invalid())?, id))
        })
        .transpose()?;
    let now = kit::now(&api).await?;
    let moment = Dates::default().at(now, api.bound)?;
    let size = usize::from(page.size);
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let mut found = crate::report::reports(&mut conn, &filter, i64::from(page.size) + 1)
        .await
        .map_err(Problem::from)?;
    let next = (found.len() > size).then(|| {
        found.truncate(size);
        let last = &found[size - 1];
        let cursor = page.next_current(
            &api.ring,
            REPORTS,
            &moment,
            json!([rfc3339(last.requested_at), last.id]),
        );
        kit::next_href(&format!("/v1/{REPORTS}"), &cursor)
    });
    let items: Vec<ReportDto> = found.into_iter().map(report_dto).collect();
    Ok(kit::respond_page(
        &headers,
        &moment,
        &items,
        next.as_deref(),
    ))
}

/// `GET /v1/policy-reports/{id}`: the report and the counts of its rubrics.
async fn show_report(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    may_report(&api, &principal, id).await?;
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let report = crate::report::report(&mut conn, id)
        .await
        .map_err(Problem::from)?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    Ok(Json(report_dto(report)).into_response())
}

/// `GET /v1/policy-reports/{id}/items?rubric=…&after=…`: the items of a rubric by rank,
/// [`ITEMS_PAGE`] at most, the next page linked.
async fn report_items(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    RawQuery(raw): RawQuery,
) -> Result<Response, Problem> {
    may_report(&api, &principal, id).await?;
    let mut rubric = None;
    let mut after = 0;
    for (name, value) in form_urlencoded_pairs(&raw.unwrap_or_default()) {
        match name.as_str() {
            "rubric" if rubric.is_none() => rubric = Some(value),
            "after" => after = value.parse().map_err(|_| kit::bad("after is a rank"))?,
            other => return Err(kit::bad(format!("unknown or repeated parameter {other}"))),
        }
    }
    let rubric = rubric.ok_or_else(|| kit::bad("rubric is required"))?;
    if !crate::report::RUBRICS.contains(&rubric.as_str()) {
        return Err(kit::bad(format!("unknown rubric {rubric}")));
    }
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    if crate::report::report(&mut conn, id)
        .await
        .map_err(Problem::from)?
        .is_none()
    {
        return Err(Problem::new(ProblemKind::NotFound));
    }
    let mut items = crate::report::items(&mut conn, id, &rubric, after, ITEMS_PAGE + 1)
        .await
        .map_err(Problem::from)?;
    let next = (items.len() > usize::try_from(ITEMS_PAGE).expect("a small page")).then(|| {
        items.truncate(usize::try_from(ITEMS_PAGE).expect("a small page"));
        format!(
            "/v1/policy-reports/{id}/items?rubric={rubric}&after={}",
            items.last().map_or(0, |i| i.seq)
        )
    });
    let items: Vec<ItemDto> = items
        .into_iter()
        .map(|i| ItemDto {
            rubric: i.rubric,
            seq: i.seq,
            subject: i.subject,
            detail: i.detail,
        })
        .collect();
    let mut body = json!({ "items": items });
    if let Some(next) = next {
        body["next"] = next.into();
    }
    Ok(Json(body).into_response())
}

// ---------------------------------------------------------------- the contract

/// A generic object of the policy, as its table holds it (design « Données »).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PolicyObject)]
pub struct ObjectDto {
    /// The identifier.
    id: Uuid,
    /// Start of validity.
    valid_from: Option<String>,
    /// End of validity, absent when open.
    valid_to: Option<String>,
    /// The members of the object, by its type.
    #[serde(flatten)]
    #[schema(additional_properties)]
    members: Map<String, Value>,
}

fn schemas() -> Vec<(String, RefOr<Schema>)> {
    let mut schemas = Vec::new();
    schemas_of::<ObjectDto>(&mut schemas);
    schemas_of::<MatrixDto>(&mut schemas);
    schemas_of::<CellDto>(&mut schemas);
    schemas_of::<VerdictQueryDto>(&mut schemas);
    schemas_of::<EndpointDto>(&mut schemas);
    schemas_of::<DossierDto>(&mut schemas);
    schemas_of::<VerdictDto>(&mut schemas);
    schemas_of::<PointDto>(&mut schemas);
    schemas_of::<AxisDto>(&mut schemas);
    schemas_of::<EvaluationDto>(&mut schemas);
    schemas_of::<DecisiveDto>(&mut schemas);
    schemas_of::<CombinationDto>(&mut schemas);
    schemas_of::<DiscardedDto>(&mut schemas);
    schemas_of::<ConditionDto>(&mut schemas);
    schemas_of::<SynthesisDto>(&mut schemas);
    schemas_of::<ReportDto>(&mut schemas);
    schemas_of::<ItemDto>(&mut schemas);
    schemas_of::<ReportRequestDto>(&mut schemas);
    schemas.push(listing_of::<ObjectDto>());
    schemas
}

/// The routes of the collection `c`.
#[allow(clippy::too_many_lines)] // the contract of each route beside its handlers
fn collection_routes(router: ModuleRouter, c: &'static Collection) -> ModuleRouter {
    let ty: &'static ObjectType = object_type(c.kind);
    let name = c.name;
    let operation_name = name.replace('-', "_");
    let mut list_op = op(
        &format!("list_{operation_name}"),
        "policy",
        &format!("The {name} of the policy."),
    );
    for (filter, member, _, _) in c.filters {
        list_op = list_op.query(filter, &format!("Equal to, or held by, `{member}`."));
    }
    let list_op = list_op
        .params::<DatesParams>()
        .params::<PageParams>()
        .answers("200", "A page of the collection.", "Listing_PolicyObject")
        .build();
    let mut methods = vec![(HttpMethod::Get, list_op)];
    let mut routing = kit::collection(
        move |State(api): State<Api>,
              Extension(principal): Extension<Principal>,
              Extension(context): Extension<RequestContext>,
              ListQuery(query): ListQuery,
              headers: HeaderMap| async move {
            list(&api, &principal, &context, &query, &headers, c).await
        },
    );
    if let Some((operation, members)) = c.create {
        methods.push((
            HttpMethod::Post,
            op(
                &format!("create_{operation_name}"),
                "policy",
                &format!("Creates in {name} ({operation})."),
            )
            .header(
                "Idempotency-Key",
                false,
                "Makes the write idempotent for 24 h.",
            )
            .body("application/json", "PolicyObject", true)
            .answers("201", "Created.", "ChangesetApplied")
            .answers("202", "Held for a decision.", "ChangesetPending")
            .build(),
        ));
        routing = routing.post(
            move |State(api): State<Api>,
                  Extension(principal): Extension<Principal>,
                  Extension(context): Extension<RequestContext>,
                  headers: HeaderMap,
                  raw: Bytes| async move {
                kit::create(
                    &api, &principal, &context, &headers, &raw, ty, operation, members,
                )
                .await
            },
        );
    }
    let mut router = router.routes(documented(&format!("/{name}"), methods, schemas(), routing));
    router = queryable(
        router,
        &format!("/{name}"),
        &format!("query_{operation_name}"),
    );
    let the = "The object.";
    let mut methods = vec![(
        HttpMethod::Get,
        op(&format!("show_{operation_name}"), "policy", the)
            .path("id", the, true)
            .params::<DatesParams>()
            .answers("200", kit::OBJECT_READ, "PolicyObject")
            .build(),
    )];
    let mut routing = get(
        move |State(api): State<Api>,
              Extension(context): Extension<RequestContext>,
              Path(id): Path<Uuid>,
              dates: Dates,
              headers: HeaderMap| async move {
            show(&api, &context, id, dates, &headers, c).await
        },
    );
    if let Some((operation, members)) = c.update {
        methods.push((
            HttpMethod::Patch,
            op(
                &format!("modify_{operation_name}"),
                "policy",
                &format!("Modifies the object ({operation}, JSON Merge Patch)."),
            )
            .path("id", the, true)
            .header("If-Match", true, kit::IF_MATCH_OBJECT)
            .body(MERGE_PATCH, "PolicyObject", true)
            .answers("200", "Modified.", "ChangesetApplied")
            .answers("202", "Held for a decision.", "ChangesetPending")
            .build(),
        ));
        routing = routing.patch(
            move |State(api): State<Api>,
                  Extension(principal): Extension<Principal>,
                  Extension(context): Extension<RequestContext>,
                  Path(id): Path<Uuid>,
                  headers: HeaderMap,
                  raw: Bytes| async move {
                kit::modify(
                    &api, &principal, &context, id, &headers, &raw, ty, operation, members,
                )
                .await
            },
        );
    }
    if c.close != Close::Never {
        methods.push((
            HttpMethod::Delete,
            op(
                &format!("close_{operation_name}"),
                "policy",
                "Closes the object.",
            )
            .path("id", the, true)
            .header("If-Match", true, kit::IF_MATCH_OBJECT)
            .answers("200", "Closed.", "ChangesetApplied")
            .answers("202", "Held for a decision.", "ChangesetPending")
            .build(),
        ));
        let environment = c.close == Close::Environment;
        routing = routing.delete(
            move |State(api): State<Api>,
                  Extension(principal): Extension<Principal>,
                  Extension(context): Extension<RequestContext>,
                  Path(id): Path<Uuid>,
                  headers: HeaderMap,
                  raw: Bytes| async move {
                if environment {
                    close_environment(&api, &principal, &context, id, &headers, &raw).await
                } else {
                    kit::close(&api, &principal, &context, id, &headers, &raw, ty).await
                }
            },
        );
    }
    router.routes(documented(
        &format!("/{name}/{{id}}"),
        methods,
        Vec::new(),
        routing,
    ))
}

/// Adds to the path item of `/verdicts` the `QUERY` of its body, in the extension
/// `x-query` (design 006 « Versions et contrat »).
fn query_extension(mut router: ModuleRouter) -> ModuleRouter {
    let operation = op(
        "query_verdicts",
        "policy",
        "The verdicts of a query of compliance (REQ-010-13).",
    )
    .body("application/json", "PolicyVerdictQuery", true)
    .answers("200", "The verdicts and their synthesis.", "PolicyVerdict")
    .build();
    let document = router.get_openapi_mut();
    let item = document
        .paths
        .paths
        .get_mut("/verdicts")
        .expect("the path of a QUERY is routed");
    item.extensions.get_or_insert_default().merge(
        utoipa::openapi::extensions::ExtensionsBuilder::new()
            .add(
                kartenn_rest::openapi::QUERY_EXTENSION,
                serde_json::to_value(&operation).expect("an operation serialises"),
            )
            .build(),
    );
    router
}

/// The routes of the module, mounted under `/v1` (ADR 0026, clause 9; REQ-010-27).
#[must_use]
#[allow(clippy::too_many_lines)] // the contract of each route beside its handler
pub fn routes() -> ModuleRouter {
    let mut router = ModuleRouter::new();
    for c in COLLECTIONS {
        router = collection_routes(router, c);
        let (key, columns) = presentation(c.name);
        router = kit::presented(router, &format!("/{}", c.name), key, columns);
    }
    let matrix_op = op(
        "show_policy_matrix",
        "policy",
        "The rules of a flow type between environments, as a matrix (REQ-010-34).",
    )
    .query("flow_type", "The flow type, by its key.")
    .params::<DatesParams>()
    .answers(
        "200",
        "The matrix; its fingerprint is its strong ETag.",
        "PolicyMatrix",
    )
    .build();
    let cell_op = op(
        "set_policy_matrix_cell",
        "policy",
        "Sets a cell of the matrix (policy.matrix.set): allow, deny, conditional or none.",
    )
    .path("source", "The environment of the source.", false)
    .path("flow_type", "The flow type.", true)
    .path("destination", "The environment of the destination.", false)
    .header(
        "If-Match",
        true,
        "The ETag of the matrix of the flow type, read at present.",
    )
    .body("application/json", "PolicyObject", true)
    .answers("200", "Set.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    let verdict_op = op(
        "show_verdicts",
        "policy",
        "The verdicts of a query of compliance, by parameters (REQ-010-13).",
    )
    .query("source", "The source, `<kind>:<id>`.")
    .query("destination", "The destination, `<kind>:<id>`.")
    .query("service", "A service, by its key.")
    .query("transport", "A transport, with `port`.")
    .query("port", "A port, with `transport`.")
    .query("flow_type", "A flow type, by its key.")
    .query("entity", "The entity of the request.")
    .params::<DatesParams>()
    .answers("200", "The verdicts and their synthesis.", "PolicyVerdict")
    .build();
    let router = router
        .routes(documented(
            "/policy-matrix",
            vec![(HttpMethod::Get, matrix_op)],
            Vec::new(),
            get(show_matrix),
        ))
        .routes(documented(
            "/policy-matrix/{source}/{flow_type}/{destination}",
            vec![(HttpMethod::Put, cell_op)],
            Vec::new(),
            axum::routing::put(set_cell),
        ))
        .routes(documented(
            "/verdicts",
            vec![(HttpMethod::Get, verdict_op)],
            Vec::new(),
            get(verdicts_get).fallback(verdicts_query),
        ));
    let report_op = op(
        "request_policy_report",
        "policy",
        "Asks for the audit report of the policy at a date (REQ-010-23); one at a time.",
    )
    .body("application/json", "PolicyReportRequest", false)
    .answers(
        "202",
        "Asked for; its Location is the report.",
        "PolicyReport",
    )
    .build();
    let list_reports_op = op(
        "list_policy_reports",
        "policy",
        "The reports of the policy, the most recently asked for first (021-30).",
    )
    .query(
        "state",
        "`pending`, `running` or `done` (repeated: one of).",
    )
    .query("requested_by", "Asked for by this identity.")
    .query("from", "Asked for from this instant (RFC 3339).")
    .query("to", "Asked for before this instant (RFC 3339).")
    .params::<PageParams>()
    .answers("200", "A page of the reports.", "Listing_PolicyReport")
    .build();
    let show_report_op = op("show_policy_report", "policy", "A report of the policy.")
        .path("id", "The report.", true)
        .answers(
            "200",
            "The report and the counts of its rubrics.",
            "PolicyReport",
        )
        .build();
    let items_op = op(
        "policy_report_items",
        "policy",
        "The items of a rubric of a report, by rank.",
    )
    .path("id", "The report.", true)
    .query("rubric", "The rubric.")
    .query("after", "The rank to start after.")
    .answers("200", "A page of items.", "PolicyReportItem")
    .build();
    let router = router
        .routes(documented(
            "/policy-reports",
            vec![
                (HttpMethod::Get, list_reports_op),
                (HttpMethod::Post, report_op),
            ],
            vec![listing_of::<ReportDto>()],
            get(list_reports).post(request_report),
        ))
        .routes(documented(
            "/policy-reports/{id}",
            vec![(HttpMethod::Get, show_report_op)],
            Vec::new(),
            get(show_report),
        ))
        .routes(documented(
            "/policy-reports/{id}/items",
            vec![(HttpMethod::Get, items_op)],
            Vec::new(),
            get(report_items),
        ));
    let router = kit::commanded(router, "/verdicts", &HttpMethod::Get, "check");
    let router = kit::commanded(router, "/policy-reports", &HttpMethod::Post, "report");
    let router = kit::presented(
        router,
        "/policy-reports",
        &[],
        &["requested_at", "state", "valid_at", "done_at"],
    );
    query_extension(router)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_reads_into_two_dates() {
        assert_eq!(
            bounds("[\"2026-01-01 00:00:00+00\",)"),
            (Some("2026-01-01 00:00:00+00".into()), None)
        );
        assert_eq!(
            bounds("[\"2026-01-01 00:00:00+00\",\"2027-01-01 00:00:00+00\")"),
            (
                Some("2026-01-01 00:00:00+00".into()),
                Some("2027-01-01 00:00:00+00".into())
            )
        );
    }

    #[test]
    fn a_query_string_decodes() {
        assert_eq!(
            form_urlencoded_pairs("source=address%3Aab&x=a+b"),
            vec![
                ("source".to_owned(), "address:ab".to_owned()),
                ("x".to_owned(), "a b".to_owned())
            ]
        );
    }

    #[test]
    fn every_collection_is_a_type_of_the_module() {
        for c in COLLECTIONS {
            assert_eq!(object_type(c.kind).kind, c.kind);
        }
        assert!(collection("environments").only.is_some());
    }
}
