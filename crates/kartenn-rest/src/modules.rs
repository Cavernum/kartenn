//! The routes the core provides for the modules (spec 008, REQ-008-9, 11; ADR 0026,
//! clauses 9 and 11): for each type of object of a module, its history, its properties,
//! their explanation and their suggestion, as for an object of the core; and the edges of
//! the topology graph the modules declare, filtered by the rights of the caller on both
//! ends. The routes of a module itself come from its crate, merged by
//! [`crate::api_with`].

use std::collections::{BTreeMap, BTreeSet};

use axum::Extension;
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::HeaderMap;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use kartenn_core::audit::rfc3339;
use kartenn_service::RequestError;
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authz::Authorizer;
use kartenn_service::module::{EdgeType, ModuleRegistry};
use kartenn_service::reads::objects_read;
use kartenn_store_pg::list::{self, Version};
use kartenn_store_pg::modules::PgModuleRead;
use kartenn_store_pg::{property_read, read};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::OffsetDateTime;
use utoipa::openapi::path::HttpMethod;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::changesets::ChangesetRef;
use crate::cursor::{Page, PageParams, next_href, respond_page};
use crate::dated::{Dates, DatesParams, ETag, respond};
use crate::objects::{Api, Target, audited, gate_target, instant, store, version_key};
use crate::openapi::{documented, listing_of, op, schemas_of};
use crate::problem::{Problem, ProblemKind};

/// The transversal routes of every type of object of the modules of `modules`.
pub fn routes(modules: &ModuleRegistry) -> OpenApiRouter<Api> {
    let mut router = OpenApiRouter::new();
    for ty in modules.object_types() {
        let target = Target::Module(ty);
        router = history_route(router, target);
        router = crate::properties::object_routes(router, target);
    }
    router
}

/// The route of the graph, served whatever the modules.
pub(crate) fn graph() -> OpenApiRouter<Api> {
    OpenApiRouter::new().routes(routes!(edges))
}

// ---------------------------------------------------------------- history

/// The record of an object of a module (ADR 0026, clause 4); the module serves its own
/// attributes.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = ModuleObject)]
struct ModuleObjectDto {
    /// Identifier.
    id: Uuid,
    /// Its qualified type (`vlan.vlan`).
    kind: String,
    /// Its parent in the tree of its module.
    parent: Option<Uuid>,
    /// Its addressing domain, when its type is anchored in one.
    domain: Option<Uuid>,
    /// Its URL.
    href: String,
}

/// A version of an object of a module, on both axes (REQ-004-18).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = ModuleVersion)]
struct ModuleVersionDto {
    /// The start of its validity, RFC 3339.
    valid_from: String,
    /// The end of its validity, RFC 3339; `null` while open.
    valid_to: Option<String>,
    /// The start of its assertion, RFC 3339.
    asserted_from: String,
    /// The end of its assertion, RFC 3339; `null` while asserted.
    asserted_to: Option<String>,
    /// The changeset that wrote it.
    changeset: ChangesetRef,
    /// The object, as it was.
    object: ModuleObjectDto,
}

fn module_version_of(target: Target, version: &Version) -> ModuleVersionDto {
    let at = |t: OffsetDateTime| rfc3339(instant(t));
    let uuid = |name: &str| {
        version
            .fields
            .get(name)
            .and_then(Value::as_str)
            .and_then(|text| text.parse().ok())
    };
    let id = uuid("id").unwrap_or_default();
    ModuleVersionDto {
        valid_from: at(version.valid_from),
        valid_to: version.valid_to.map(at),
        asserted_from: at(version.asserted_from),
        asserted_to: version.asserted_to.map(at),
        changeset: crate::changesets::summary(version.changeset),
        object: ModuleObjectDto {
            id,
            kind: target.as_str().to_owned(),
            parent: uuid("parent_id"),
            domain: uuid("domain_id"),
            href: target.href(id),
        },
    }
}

fn history_route(router: OpenApiRouter<Api>, target: Target) -> OpenApiRouter<Api> {
    let name = target.collection();
    let mut schemas = Vec::new();
    schemas_of::<ModuleVersionDto>(&mut schemas);
    schemas.push(listing_of::<ModuleVersionDto>());
    let module = target.as_str().split('.').next().unwrap_or_default();
    let history_op = op(
        &format!("{}_history", target.operation_name()),
        module,
        &format!(
            "The versions of the record of the {}, on both axes (REQ-004-18).",
            target.as_str()
        ),
    )
    .path("id", &format!("The {}.", target.as_str()), true)
    .params::<DatesParams>()
    .params::<PageParams>()
    .answers("200", "A page of versions.", "Listing_ModuleVersion")
    .build();
    router.routes(documented(
        &format!("/{name}/{{id}}/history"),
        vec![(HttpMethod::Get, history_op)],
        schemas,
        get(
            move |state: State<Api>,
                  context: Extension<RequestContext>,
                  id: Path<Uuid>,
                  query: RawQuery,
                  headers: HeaderMap| {
                history(target, state, context, id, query, headers)
            },
        ),
    ))
}

/// `GET /v1/<collection>/{id}/history` of a type of a module: the versions of the record
/// of an object the caller may read now, as for an object of the core (REQ-008-9).
async fn history(
    target: Target,
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let name = format!("{}/{id}/history", target.collection());
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, &name)?;
    page.unfiltered()?;
    let (after, valid_at) = match page.after().map(version_key).transpose()? {
        Some((after, valid_at)) => (Some(after), valid_at),
        None => (None, page.dates.valid_at),
    };
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment = page.dates.at(now, api.bound)?;
    gate_target(&api, &context, target, id).await?;
    let size = usize::from(page.size);
    let mut versions = list::module_history_page(
        &api.pool,
        target.as_str(),
        id,
        valid_at,
        moment.as_of,
        after,
        Some(i64::from(page.size) + 1),
    )
    .await
    .map_err(|e| store(&e))?;
    if versions.is_empty() && after.is_none() {
        return Err(Problem::new(ProblemKind::NotFound));
    }
    let next = (versions.len() > size).then(|| {
        versions.truncate(size);
        let last = &versions[size - 1];
        let at = |t: OffsetDateTime| rfc3339(instant(t));
        let cursor = page.next(
            &api.ring,
            &name,
            &moment,
            json!([
                at(last.valid_from),
                at(last.asserted_from),
                valid_at.map(at)
            ]),
        );
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let items: Vec<ModuleVersionDto> = versions
        .iter()
        .map(|v| module_version_of(target, v))
        .collect();
    let criteria = json!({ "object": id, "history": true });
    let event =
        objects_read(&context, target.as_str(), &[id], criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

// ---------------------------------------------------------------- graph

/// The object whose edges are read.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct EdgesParams {
    /// The object, at an end of the edges.
    object: Uuid,
}

/// An end of an edge.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = GraphNode)]
struct EndDto {
    /// Its kind of the core, or its qualified type.
    kind: &'static str,
    /// Identifier.
    id: Uuid,
    /// Its URL.
    href: String,
}

/// An edge of the topology graph (ADR 0026, clause 11).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = GraphEdge)]
struct EdgeDto {
    /// Its type (`vlan.carries`).
    #[serde(rename = "type")]
    edge_type: &'static str,
    /// Its origin.
    from: EndDto,
    /// Its end.
    to: EndDto,
    /// The start of its validity, RFC 3339.
    valid_from: String,
    /// The end of its validity, RFC 3339; `null` while open.
    valid_to: Option<String>,
}

/// The edges of an object.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = GraphEdges)]
struct EdgesDto {
    /// The edges, by type, origin then end.
    items: Vec<EdgeDto>,
}

/// `GET /v1/graph/edges?object=`: the edges the modules declare that start or end at the
/// object, at the dates of the read; only those whose two ends the caller may read now
/// (REQ-008-11). 404 for an object absent at the dates or that the caller may not read.
#[utoipa::path(
    get,
    path = "/graph/edges",
    operation_id = "graph_edges",
    tag = "graph",
    params(EdgesParams, DatesParams),
    responses((status = 200, description = "The edges of the object.", body = EdgesDto))
)]
#[allow(clippy::too_many_lines)] // the object, the edges of each module, their ends, the audit
async fn edges(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Query(params): Query<EdgesParams>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment = dates.at(now, api.bound)?;
    let object = params.object;
    let kinds = property_read::kind_names(&api.pool, &[object], moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?;
    let target = kinds
        .get(&object)
        .and_then(|name| Target::named(name, &api.modules))
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    gate_target(&api, &context, target, object).await?;
    let mut found: Vec<(&'static EdgeType, kartenn_service::module::Edge)> = Vec::new();
    for module in api.modules.modules() {
        let declared = module.edges();
        let Some(handler) = module.handler().filter(|_| !declared.is_empty()) else {
            continue;
        };
        let mut reader = PgModuleRead::open(
            &api.pool,
            instant(moment.valid_at),
            moment.as_of.map(instant),
        )
        .await
        .map_err(|e| store(&e))?;
        for edge in handler.edges(&mut reader, object).await? {
            if let Some(ty) = declared.iter().find(|ty| ty.name == edge.name) {
                found.push((ty, edge));
            }
        }
    }
    // The ends the caller may read, type by type.
    let mut ends: BTreeMap<&'static str, BTreeSet<Uuid>> = BTreeMap::new();
    for (ty, edge) in &found {
        ends.entry(ty.from).or_default().insert(edge.from);
        ends.entry(ty.to).or_default().insert(edge.to);
    }
    let mut readable: BTreeSet<(&'static str, Uuid)> = BTreeSet::new();
    let authorizer = api.authorizer();
    for (name, ids) in &ends {
        let Some(end) = Target::named(name, &api.modules) else {
            continue;
        };
        let ids: Vec<Uuid> = ids.iter().copied().collect();
        let denied: BTreeSet<Uuid> = match context.subject() {
            Some(subject) => authorizer
                .authorize_many(&subject, &end.read_permission(), &ids)
                .await?
                .into_iter()
                .collect(),
            None => BTreeSet::new(),
        };
        readable.extend(
            ids.into_iter()
                .filter(|id| !denied.contains(id))
                .map(|id| (*name, id)),
        );
    }
    let end_of = |name: &'static str, id: Uuid| {
        Target::named(name, &api.modules).map(|end| EndDto {
            kind: end.as_str(),
            id,
            href: end.href(id),
        })
    };
    let at = |t: OffsetDateTime| rfc3339(instant(t));
    let mut items: Vec<EdgeDto> = found
        .into_iter()
        .filter(|(ty, edge)| {
            readable.contains(&(ty.from, edge.from)) && readable.contains(&(ty.to, edge.to))
        })
        .filter_map(|(ty, edge)| {
            Some(EdgeDto {
                edge_type: ty.name,
                from: end_of(ty.from, edge.from)?,
                to: end_of(ty.to, edge.to)?,
                valid_from: rfc3339(edge.period.start()),
                valid_to: edge.period.end().map(rfc3339),
            })
        })
        .collect();
    items.sort_by(|a, b| {
        (a.edge_type, a.from.id, a.to.id, &a.valid_from).cmp(&(
            b.edge_type,
            b.from.id,
            b.to.id,
            &b.valid_from,
        ))
    });
    let criteria = json!({
        "object": object,
        "graph": true,
        "valid_at": at(moment.valid_at),
        "as_of": at(moment.as_of()),
    });
    let event =
        objects_read(&context, target.as_str(), &[object], criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    let bytes = serde_json::to_vec(&EdgesDto { items }).expect("JSON values serialise");
    let etag = ETag::page(&bytes);
    let response = ([(CONTENT_TYPE, "application/json")], bytes).into_response();
    Ok(respond(&headers, &moment, &etag, response))
}
