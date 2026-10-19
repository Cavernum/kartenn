//! The addressing tree (spec 006, REQ-006-4, 23; task 006-12b; 002/REQ-002-42 to 44):
//! under `/v1/prefixes/{id}`, its direct children and its ancestors, as far as the caller
//! may read them; its occupancy; the first free prefix of a length or the first free
//! address, proposed, not assigned. A prefix the caller may not read answers 404, as an
//! absent one. The occupancy and the next free value count the whole content, readable or
//! not: the first is a measure of the prefix, the second must not propose what is taken.

use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Json, Router};
use kartenn_core::addr::Addr;
use kartenn_core::audit::rfc3339;
use kartenn_core::family::ALL;
use kartenn_core::objects::{Nature, PrefixObject, RangeUsage};
use kartenn_service::RequestError;
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::reads::objects_read;
use kartenn_store_pg::list::Kind;
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::read;
use kartenn_store_pg::tree::{self, ChildKey, Node};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::cursor::{Page, next_href, respond_page};
use crate::dated::{Dates, ETag, Moment, respond};
use crate::objects::{Api, audited, bad, gate, href, instant, store};
use crate::problem::{Problem, ProblemKind};

/// The routes of the tree.
pub fn routes(api: Api) -> Router {
    Router::new()
        .route("/prefixes/{id}/children", get(children))
        .route("/prefixes/{id}/ancestors", get(ancestors))
        .route("/prefixes/{id}/occupancy", get(occupancy))
        .route("/prefixes/{id}/next-free", get(next_free))
        .with_state(api)
}

/// A node of the tree, as listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeDto {
    /// A prefix.
    Prefix {
        /// Identifier.
        id: Uuid,
        /// Canonical CIDR.
        value: String,
        /// Container or network.
        nature: Nature,
        /// Its URL.
        href: String,
    },
    /// A range.
    Range {
        /// Identifier.
        id: Uuid,
        /// First address.
        first: String,
        /// Last address, included.
        last: String,
        /// Usage.
        usage: RangeUsage,
        /// Its URL.
        href: String,
    },
    /// An address.
    Address {
        /// Identifier.
        id: Uuid,
        /// Canonical address.
        value: String,
        /// Its URL.
        href: String,
    },
}

impl From<&Node> for NodeDto {
    fn from(node: &Node) -> Self {
        match *node {
            Node::Prefix { id, value, nature } => Self::Prefix {
                id,
                value: value.to_string(),
                nature,
                href: href(Kind::Prefix, id),
            },
            Node::Range {
                id,
                first,
                last,
                usage,
            } => Self::Range {
                id,
                first: first.to_string(),
                last: last.to_string(),
                usage,
                href: href(Kind::Range, id),
            },
            Node::Address { id, value } => Self::Address {
                id,
                value: value.to_string(),
                href: href(Kind::Address, id),
            },
        }
    }
}

/// The moment of the read and the prefix `id` then, or 404 if the caller may not read it
/// or it is absent then.
async fn prefix(
    api: &Api,
    context: &RequestContext,
    id: Uuid,
    dates: Dates,
) -> Result<(Moment, PrefixObject), Problem> {
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment = dates.at(now, api.bound)?;
    gate(api, context, Kind::Prefix, id).await?;
    let found = tree::prefix_as_of(&api.pool, id, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    Ok((moment, found))
}

/// Records the read of `ids` by `context` for the route `route` of the prefix `id`.
async fn record(
    api: &Api,
    context: &RequestContext,
    moment: &Moment,
    route: &str,
    id: Uuid,
    ids: &[Uuid],
) -> Result<(), Problem> {
    let criteria = json!({
        "prefix": id,
        "tree": route,
        "valid_at": rfc3339(instant(moment.valid_at)),
        "as_of": rfc3339(instant(moment.as_of())),
    });
    let event = objects_read(context, "object", ids, criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(())
}

/// The nodes as listed, and their identifiers.
fn listed(nodes: &[Node]) -> Result<(Vec<Value>, Vec<Uuid>), Problem> {
    let items = nodes
        .iter()
        .map(|n| serde_json::to_value(NodeDto::from(n)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Problem::new(ProblemKind::Internal))?;
    Ok((items, nodes.iter().map(Node::id).collect()))
}

/// A document as served, its weak `ETag` that of its bytes.
fn document(headers: &HeaderMap, moment: &Moment, body: &Value) -> Response {
    let etag = ETag::page(body.to_string().as_bytes());
    respond(headers, moment, &etag, Json(body.clone()))
}

/// The key of a child as written in a cursor by [`children`].
fn child_key(key: &Value) -> Result<ChildKey, Problem> {
    let invalid = || Problem::new(ProblemKind::InvalidCursor);
    let (start, rank, id) =
        serde_json::from_value::<(String, i16, Uuid)>(key.clone()).map_err(|_| invalid())?;
    let start = Addr::parse_any(&ALL, &start).map_err(|_| invalid())?;
    Ok(ChildKey { start, rank, id })
}

/// `GET /v1/prefixes/{id}/children`: the direct children the caller may read, in address
/// order (REQ-002-42), by pages (REQ-006-7, 8): the key of a child is its first address,
/// its rank (prefix, range, address) and its identifier.
async fn children(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let name = format!("prefixes/{id}/children");
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, &name)?;
    page.unfiltered()?;
    let after = page.after().map(child_key).transpose()?;
    let (moment, parent) = prefix(&api, &context, id, page.dates).await?;
    let scope = PgAuthorizer::new(api.pool.clone())
        .read_scope(&principal.subject(), moment.now)
        .await?;
    let size = usize::from(page.size);
    let mut nodes = tree::children_of(
        &api.pool,
        &parent,
        moment.valid_at,
        moment.as_of,
        &scope,
        after.as_ref(),
        Some(i64::from(page.size) + 1),
    )
    .await
    .map_err(|e| store(&e))?;
    let next = (nodes.len() > size).then(|| {
        nodes.truncate(size);
        let last = nodes[size - 1].key();
        let cursor = page.next(
            &api.ring,
            &name,
            &moment,
            json!([last.start.to_string(), last.rank, last.id]),
        );
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let (items, ids) = listed(&nodes)?;
    record(&api, &context, &moment, "children", id, &ids).await?;
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// `GET /v1/prefixes/{id}/ancestors`: the ancestors the caller may read, nearest first
/// (REQ-002-42).
async fn ancestors(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let (moment, found) = prefix(&api, &context, id, dates).await?;
    let scope = PgAuthorizer::new(api.pool.clone())
        .read_scope(&principal.subject(), moment.now)
        .await?;
    let nodes = tree::ancestors_of(&api.pool, &found, moment.valid_at, moment.as_of, &scope)
        .await
        .map_err(|e| store(&e))?;
    let (items, ids) = listed(&nodes)?;
    record(&api, &context, &moment, "ancestors", id, &ids).await?;
    Ok(respond_page(&headers, &moment, &items, None))
}

/// `GET /v1/prefixes/{id}/occupancy`: used and total addresses (REQ-002-43), as decimal
/// strings: an IPv6 count exceeds the integers a JSON number carries exactly (RFC 8259 §6).
async fn occupancy(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let (moment, found) = prefix(&api, &context, id, dates).await?;
    let measure = tree::occupancy_as_of(&api.pool, id, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    record(&api, &context, &moment, "occupancy", id, &[id]).await?;
    let body = json!({
        "prefix": { "id": id, "value": found.value.to_string(), "href": href(Kind::Prefix, id) },
        "nature": found.nature,
        "used": measure.used.to_string(),
        "total": measure.total.to_string(),
    });
    Ok(document(&headers, &moment, &body))
}

/// What `next-free` asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wanted {
    Prefix(u8),
    Address,
}

/// `kind=prefix&length=<n>` or `kind=address`, beside the dates; anything else → 400.
fn wanted(query: Option<&str>) -> Result<Wanted, Problem> {
    let (mut kind, mut length) = (None, None);
    for (name, value) in form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        let slot = match name.as_ref() {
            "valid_at" | "as_of" => continue,
            "kind" => &mut kind,
            "length" => &mut length,
            other => return Err(bad(format!("unknown parameter {other}"))),
        };
        if slot.replace(value.into_owned()).is_some() {
            return Err(bad(format!("{name} is given more than once")));
        }
    }
    match (kind.as_deref(), length) {
        (Some("prefix"), Some(length)) => length
            .parse()
            .map(Wanted::Prefix)
            .map_err(|_| bad("length must be a prefix length")),
        (Some("prefix"), None) => Err(bad("kind=prefix needs a length")),
        (Some("address"), None) => Ok(Wanted::Address),
        (Some("address"), Some(_)) => Err(bad("kind=address takes no length")),
        _ => Err(bad("kind must be prefix or address")),
    }
}

/// `GET /v1/prefixes/{id}/next-free?kind=prefix&length=<n>` in a container, or
/// `?kind=address` in a network: the first free value then, `null` when none is left,
/// proposed and not assigned (REQ-002-44). A length outside the container → 400; a
/// container asked for an address, or a network for a prefix → 409.
async fn next_free(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let wanted = wanted(query.as_deref())?;
    let (moment, found) = prefix(&api, &context, id, dates).await?;
    let (kind, value) = match wanted {
        Wanted::Prefix(length) => {
            let max = found.value.family().width();
            if length <= found.value.prefix_len() || length > max {
                return Err(bad(format!(
                    "length must be longer than {} and at most {max}",
                    found.value.prefix_len()
                )));
            }
            if found.nature != Nature::Container {
                return Err(Problem::new(ProblemKind::Conflict)
                    .detail("a free prefix is proposed in a container"));
            }
            let free =
                tree::next_free_prefix_as_of(&api.pool, id, length, moment.valid_at, moment.as_of)
                    .await
                    .map_err(|e| store(&e))?;
            ("prefix", free.map(|p| p.to_string()))
        }
        Wanted::Address => {
            if found.nature != Nature::Network {
                return Err(Problem::new(ProblemKind::Conflict)
                    .detail("a free address is proposed in a network"));
            }
            let free = tree::next_free_address_as_of(&api.pool, id, moment.valid_at, moment.as_of)
                .await
                .map_err(|e| store(&e))?;
            ("address", free.map(|a| a.to_string()))
        }
    };
    record(&api, &context, &moment, "next-free", id, &[id]).await?;
    let body = json!({
        "prefix": { "id": id, "value": found.value.to_string(), "href": href(Kind::Prefix, id) },
        "kind": kind,
        "value": value,
    });
    Ok(document(&headers, &moment, &body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_free_takes_a_kind_and_a_length_for_a_prefix() {
        assert_eq!(
            wanted(Some("kind=prefix&length=26")).unwrap(),
            Wanted::Prefix(26)
        );
        assert_eq!(
            wanted(Some("kind=address&valid_at=2026-01-01T00:00:00Z")).unwrap(),
            Wanted::Address
        );
        for refused in [
            None,
            Some("kind=prefix"),
            Some("kind=prefix&length=x"),
            Some("kind=address&length=24"),
            Some("kind=range"),
            Some("kind=address&kind=address"),
            Some("kind=address&other=1"),
        ] {
            assert!(wanted(refused).is_err(), "{refused:?}");
        }
    }
}
