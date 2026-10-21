//! The routes of the module under `/v1` (feature `rest`; spec 008, REQ-008-19 to 22;
//! design « Lectures et routes »), mounted by kartenn-server: groups and VLANs as
//! collections, dated, paged, filtered and readable as the routes of the core; writes by
//! changeset, Merge Patch under `If-Match`; the occupancy of a group, its next free VID
//! and the networks of a VLAN.
//!
//! An object the caller may not read answers as an absent one (ADR 0021); a list keeps
//! the objects of the read scope of the caller, by the predicate of the core.

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::response::Response;
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
use kartenn_store_pg::list::{self, Criteria, Key, Kind};
use kartenn_store_pg::rbac::{ReadScope, readable};
use kartenn_store_pg::temporal::snapshot;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa::openapi::path::HttpMethod;
use utoipa::openapi::{RefOr, Schema};
use uuid::Uuid;

use crate::group::{SliceState, VidRange, next_free, occupancy};
use crate::module::{group_type, vlan_type};
use crate::vlan::{Vid, VlanStatus};

/// The members a write of a group may carry.
const GROUP_MEMBERS: &[&str] = &["slug", "anchor", "vid_min", "vid_max", "owner"];
/// The members a write of a VLAN may carry.
const VLAN_MEMBERS: &[&str] = &["group", "vid", "name", "status"];

// ---------------------------------------------------------------- representations

/// The identifier of an object, as the operations name it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[schema(as = VlanObjectRef)]
pub struct IdRef {
    /// Identifier.
    pub id: Uuid,
}

/// The anchor of a group (V3): an entity, a site or an addressing domain.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = VlanGroupAnchor)]
pub enum Anchor {
    /// An entity.
    Entity(IdRef),
    /// A site.
    Site(IdRef),
    /// An addressing domain.
    Domain(IdRef),
}

impl Anchor {
    fn of(kind: &str, id: Uuid) -> Self {
        match kind {
            "entity" => Self::Entity(IdRef { id }),
            "site" => Self::Site(IdRef { id }),
            _ => Self::Domain(IdRef { id }),
        }
    }
}

/// A group of VLANs (REQ-008-12).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = VlanGroup)]
pub struct GroupDto {
    /// Identifier.
    pub id: Uuid,
    /// Slug, unique among the living groups.
    pub slug: String,
    /// Anchor.
    pub anchor: Anchor,
    /// First VID of its range.
    pub vid_min: i16,
    /// Last VID of its range.
    pub vid_max: i16,
    /// Its URL.
    pub href: String,
}

/// A VLAN (REQ-008-14).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Vlan)]
pub struct VlanDto {
    /// Identifier.
    pub id: Uuid,
    /// Its group.
    pub group: IdRef,
    /// VID, 1 to 4094, unique in the group.
    pub vid: i16,
    /// Name, unique in the group.
    pub name: String,
    /// `active`, `reserved` or `deprecated`.
    pub status: String,
    /// Its URL.
    pub href: String,
}

/// A run of VIDs in the same state.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = VlanSlice)]
pub struct SliceDto {
    /// First VID.
    pub first: u16,
    /// Last VID.
    pub last: u16,
    /// `used`, `reserved` or `free`.
    pub state: &'static str,
}

/// The occupancy of a group at a date (REQ-008-20).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = VlanOccupancy)]
pub struct OccupancyDto {
    /// The group.
    pub group: IdRef,
    /// Its range, from `vid_min` to `vid_max`, in runs.
    pub slices: Vec<SliceDto>,
    /// VIDs of active or deprecated VLANs.
    pub used: u16,
    /// VIDs of reserved VLANs.
    pub reserved: u16,
    /// VIDs of no VLAN.
    pub free: u16,
}

/// The next free VID of a group (REQ-008-20).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = VlanNextFree)]
pub struct NextFreeDto {
    /// The group.
    pub group: IdRef,
    /// The smallest free VID, at or after `from`.
    pub vid: u16,
}

/// A network that carries a VLAN (REQ-008-21).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = VlanNetwork)]
pub struct NetworkDto {
    /// Identifier.
    pub id: Uuid,
    /// The network, CIDR.
    pub prefix: String,
    /// Its URL.
    pub href: String,
}

/// The body of the creation of a group.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = VlanGroupCreation)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct GroupCreation {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Slug.
    slug: String,
    /// Anchor.
    anchor: Anchor,
    /// First VID, 1 by default.
    vid_min: Option<i16>,
    /// Last VID, 4094 by default.
    vid_max: Option<i16>,
    /// The owner, the entity of the anchor by default.
    owner: Option<IdRef>,
}

/// The body of the modification of a group (JSON Merge Patch).
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = VlanGroupPatch)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct GroupPatch {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Slug.
    slug: Option<String>,
    /// Anchor.
    anchor: Option<Anchor>,
    /// First VID.
    vid_min: Option<i16>,
    /// Last VID.
    vid_max: Option<i16>,
    /// The owner.
    owner: Option<IdRef>,
}

/// The body of the creation of a VLAN.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = VlanCreation)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct VlanCreation {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Its group.
    group: IdRef,
    /// VID.
    vid: i16,
    /// Name.
    name: String,
    /// `active` by default, `reserved` or `deprecated`.
    status: Option<String>,
}

/// The body of the modification of a VLAN (JSON Merge Patch).
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = VlanPatch)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct VlanPatch {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Its group.
    group: Option<IdRef>,
    /// VID.
    vid: Option<i16>,
    /// Name.
    name: Option<String>,
    /// Status.
    status: Option<String>,
}

/// The optional body of the closure of an object of the module (REQ-008-10).
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = VlanClosure)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct Closure {
    /// Why; required by the rules.
    motive: String,
    /// The start of the closure, RFC 3339; now by default.
    valid_from: Option<String>,
    /// Also close the VLANs of a group, and remove the VLAN of its networks.
    cascade: Option<bool>,
}

// ---------------------------------------------------------------- reading

/// The rows of `table` (a table of the module) valid at `$1`, as asserted at `$2` (now
/// when null): the relation of a dated read, as `temporal::snapshot` builds it for the
/// tables of the core.
fn dated(table: &str, columns: &str) -> String {
    format!(
        "(SELECT {columns} FROM {table}
           WHERE valid_period @> $1::timestamptz
             AND ($2::timestamptz IS NULL OR sys_start <= $2::timestamptz)
          UNION ALL
          SELECT {columns} FROM {table}_history
           WHERE $2::timestamptz IS NOT NULL AND valid_period @> $1::timestamptz
             AND sys_start <= $2::timestamptz AND sys_end > $2::timestamptz)"
    )
}

/// The condition that the row `x` of `kind` is readable at `$1` under the scope `s`: its
/// domain is that of its record in the core.
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

type GroupRow = (Uuid, String, String, Uuid, i16, i16);
type VlanRow = (Uuid, Uuid, i16, String, String);

fn group_dto((id, slug, kind, anchor, vid_min, vid_max): GroupRow) -> GroupDto {
    GroupDto {
        id,
        slug,
        anchor: Anchor::of(&kind, anchor),
        vid_min,
        vid_max,
        href: format!("/v1/vlan-groups/{id}"),
    }
}

fn vlan_dto((id, group, vid, name, status): VlanRow) -> VlanDto {
    VlanDto {
        id,
        group: IdRef { id: group },
        vid,
        name,
        status,
        href: format!("/v1/vlans/{id}"),
    }
}

const GROUP_COLUMNS: &str = "id, slug, anchor_kind, anchor_id, vid_min, vid_max";
const VLAN_COLUMNS: &str = "id, group_id, vid, name, status";

/// The filters of a list, each given once, among `names`.
fn filters<'a>(
    page: &'a Page,
    names: &[&str],
) -> Result<std::collections::BTreeMap<&'a str, &'a str>, Problem> {
    let mut found = std::collections::BTreeMap::new();
    for (name, value) in &page.filters {
        if !names.contains(&name.as_str()) {
            return Err(kit::bad(format!("unknown parameter {name}")));
        }
        if found.insert(name.as_str(), value.as_str()).is_some() {
            return Err(kit::bad(format!("{name} is given more than once")));
        }
    }
    Ok(found)
}

fn uuid(name: &str, text: &str) -> Result<Uuid, Problem> {
    text.parse()
        .map_err(|_| kit::bad(format!("{name} is not an identifier")))
}

fn vid(name: &str, text: &str) -> Result<i16, Problem> {
    text.parse::<i64>()
        .ok()
        .and_then(|v| Vid::new(v).ok())
        .and_then(|v| i16::try_from(v.get()).ok())
        .ok_or_else(|| kit::bad(format!("{name} must be a VID, 1 to 4094")))
}

/// `vid=10` or `vid=10..20`.
fn vid_range(text: &str) -> Result<(i16, i16), Problem> {
    match text.split_once("..") {
        Some((low, high)) => {
            let (low, high) = (vid("vid", low)?, vid("vid", high)?);
            if low > high {
                return Err(kit::bad("vid: the range is empty"));
            }
            Ok((low, high))
        }
        None => vid("vid", text).map(|v| (v, v)),
    }
}

/// The dates and the read scope of a list.
async fn reading(
    api: &Api,
    principal: &Principal,
    page: &Page,
) -> Result<(Moment, ReadScope), Problem> {
    let now = kit::now(api).await?;
    let moment = page.dates.at(now, api.bound)?;
    let scope = kit::scope(api, principal, now).await?;
    Ok((moment, scope))
}

fn criteria(collection: &str, page: &Page, moment: &Moment) -> Value {
    json!({
        "collection": collection,
        "filters": page.filters,
        "valid_at": rfc3339(moment.valid_at),
        "as_of": rfc3339(moment.as_of()),
    })
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&time::format_description::well_known::Rfc3339)
        .expect("an instant formats")
}

/// `GET`/`QUERY /v1/vlan-groups`: by slug, filtered by `slug` and `anchor`.
async fn list_groups(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    ListQuery(query): ListQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    const NAME: &str = "vlan-groups";
    let page = Page::from_query(&query, &api.ring, NAME)?;
    let wanted = filters(&page, &["slug", "anchor"])?;
    let anchor = wanted
        .get("anchor")
        .map(|a| uuid("anchor", a))
        .transpose()?;
    let after = page
        .after()
        .map(|key| serde_json::from_value::<(String, Uuid)>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))?;
    let (moment, scope) = reading(&api, &principal, &page).await?;
    let sql = format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT x.id, x.slug, x.anchor_kind, x.anchor_id, x.vid_min, x.vid_max
           FROM {} x
          WHERE ($4::text IS NULL OR x.slug = $4)
            AND ($5::uuid IS NULL OR x.anchor_id = $5)
            AND ($6::text IS NULL OR (x.slug, x.id) > ($6, $7::uuid))
            AND ($3::jsonb IS NULL OR {})
          ORDER BY x.slug, x.id
          LIMIT $8",
        dated("vlan.vlan_group", GROUP_COLUMNS),
        visible(crate::module::GROUP),
    );
    let mut rows: Vec<GroupRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(scope.to_json())
        .bind(wanted.get("slug").copied())
        .bind(anchor)
        .bind(after.as_ref().map(|(slug, _)| slug.as_str()))
        .bind(after.as_ref().map(|(_, id)| *id))
        .bind(i64::from(page.size) + 1)
        .fetch_all(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?;
    let size = usize::from(page.size);
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let last = &rows[size - 1];
        let cursor = page.next(&api.ring, NAME, &moment, json!([last.1, last.0]));
        next_href(&format!("/v1/{NAME}"), &cursor)
    });
    let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    kit::record_read(
        &api,
        &context,
        crate::module::GROUP,
        &ids,
        criteria(NAME, &page, &moment),
    )
    .await?;
    let items: Vec<GroupDto> = rows.into_iter().map(group_dto).collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// `GET`/`QUERY /v1/vlans`: by group then VID, filtered by `group`, `vid` (a value or
/// `a..b`), `name`, `name_prefix` and `status`, in conjunction (REQ-008-19).
async fn list_vlans(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    ListQuery(query): ListQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    const NAME: &str = "vlans";
    let page = Page::from_query(&query, &api.ring, NAME)?;
    let wanted = filters(&page, &["group", "vid", "name", "name_prefix", "status"])?;
    let group = wanted.get("group").map(|g| uuid("group", g)).transpose()?;
    let range = wanted.get("vid").map(|v| vid_range(v)).transpose()?;
    if let Some(status) = wanted.get("status")
        && VlanStatus::parse(status).is_none()
    {
        return Err(kit::bad("status must be active, reserved or deprecated"));
    }
    let after = page
        .after()
        .map(|key| serde_json::from_value::<(Uuid, i16, Uuid)>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))?;
    let (moment, scope) = reading(&api, &principal, &page).await?;
    // The read predicate above the filtered rows in their order (`OFFSET 0` keeps it out of
    // the subquery): the limit stops it after the page. Beside the filters, it ran on every
    // row they keep before the sort (task 008-16: 654 ms for a page of 100 at 10⁵ VLANs,
    // 19 ms after).
    let sql = format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT x.id, x.group_id, x.vid, x.name, x.status
           FROM (SELECT x.* FROM {} x
                  WHERE ($4::uuid IS NULL OR x.group_id = $4)
                    AND ($5::smallint IS NULL OR x.vid BETWEEN $5 AND $6)
                    AND ($7::text IS NULL OR x.name = $7)
                    AND ($8::text IS NULL OR starts_with(x.name, $8))
                    AND ($9::text IS NULL OR x.status = $9)
                    AND ($10::uuid IS NULL
                         OR (x.group_id, x.vid, x.id) > ($10, $11::smallint, $12::uuid))
                  ORDER BY x.group_id, x.vid, x.id
                  OFFSET 0) x
          WHERE ($3::jsonb IS NULL OR {})
          ORDER BY x.group_id, x.vid, x.id
          LIMIT $13",
        dated("vlan.vlan", VLAN_COLUMNS),
        visible(crate::module::VLAN),
    );
    let mut rows: Vec<VlanRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(scope.to_json())
        .bind(group)
        .bind(range.map(|r| r.0))
        .bind(range.map(|r| r.1))
        .bind(wanted.get("name").copied())
        .bind(wanted.get("name_prefix").copied())
        .bind(wanted.get("status").copied())
        .bind(after.map(|a| a.0))
        .bind(after.map(|a| a.1))
        .bind(after.map(|a| a.2))
        .bind(i64::from(page.size) + 1)
        .fetch_all(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?;
    let size = usize::from(page.size);
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let last = &rows[size - 1];
        let cursor = page.next(&api.ring, NAME, &moment, json!([last.1, last.2, last.0]));
        next_href(&format!("/v1/{NAME}"), &cursor)
    });
    let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    kit::record_read(
        &api,
        &context,
        crate::module::VLAN,
        &ids,
        criteria(NAME, &page, &moment),
    )
    .await?;
    let items: Vec<VlanDto> = rows.into_iter().map(vlan_dto).collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// The dates of a read by identifier, once the caller may read the object `id` of `ty`.
async fn dated_read(
    api: &Api,
    context: &RequestContext,
    ty: &'static ObjectType,
    id: Uuid,
    dates: Dates,
) -> Result<Moment, Problem> {
    kit::gate(api, context, ty, id).await?;
    let now = kit::now(api).await?;
    dates.at(now, api.bound)
}

async fn group_at(api: &Api, id: Uuid, moment: &Moment) -> Result<GroupRow, Problem> {
    let sql = format!(
        "SELECT {GROUP_COLUMNS} FROM {} x WHERE x.id = $3",
        dated("vlan.vlan_group", GROUP_COLUMNS)
    );
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(id)
        .fetch_optional(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))
}

/// The answer of a read by identifier: the object, its version as `ETag`, the read
/// journaled.
async fn answer<T: Serialize>(
    api: &Api,
    context: &RequestContext,
    headers: &HeaderMap,
    kind: &str,
    id: Uuid,
    moment: &Moment,
    body: T,
) -> Result<Response, Problem> {
    let version = kit::version(api, id, moment)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    kit::record_read(api, context, kind, &[id], json!({"id": id})).await?;
    Ok(respond(
        headers,
        moment,
        &ETag::version(version),
        Json(body),
    ))
}

/// `GET /v1/vlan-groups/{id}`.
async fn show_group(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let moment = dated_read(&api, &context, group_type(), id, dates).await?;
    let row = group_at(&api, id, &moment).await?;
    answer(
        &api,
        &context,
        &headers,
        crate::module::GROUP,
        id,
        &moment,
        group_dto(row),
    )
    .await
}

/// `GET /v1/vlans/{id}`.
async fn show_vlan(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let moment = dated_read(&api, &context, vlan_type(), id, dates).await?;
    let sql = format!(
        "SELECT {VLAN_COLUMNS} FROM {} x WHERE x.id = $3",
        dated("vlan.vlan", VLAN_COLUMNS)
    );
    let row: VlanRow = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(id)
        .fetch_optional(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    answer(
        &api,
        &context,
        &headers,
        crate::module::VLAN,
        id,
        &moment,
        vlan_dto(row),
    )
    .await
}

/// The range of the group `row` and its VLANs at `moment`, `(vid, status)`.
async fn living(
    api: &Api,
    row: &GroupRow,
    moment: &Moment,
) -> Result<(VidRange, Vec<(Vid, VlanStatus)>), Problem> {
    let range = VidRange::new(i64::from(row.4), i64::from(row.5))
        .map_err(|v| Problem::new(ProblemKind::Internal).detail(v.message))?;
    let sql = format!(
        "SELECT x.vid, x.status FROM {} x WHERE x.group_id = $3",
        dated("vlan.vlan", VLAN_COLUMNS)
    );
    let vlans: Vec<(i16, String)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(row.0)
        .fetch_all(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?;
    let live = vlans
        .into_iter()
        .filter_map(|(vid, status)| {
            Some((Vid::new(i64::from(vid)).ok()?, VlanStatus::parse(&status)?))
        })
        .collect();
    Ok((range, live))
}

/// `GET /v1/vlan-groups/{id}/occupancy`: runs of used, reserved and free VIDs.
async fn group_occupancy(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let moment = dated_read(&api, &context, group_type(), id, dates).await?;
    let row = group_at(&api, id, &moment).await?;
    let (range, live) = living(&api, &row, &moment).await?;
    let found = occupancy(range, &live);
    let body = OccupancyDto {
        group: IdRef { id },
        slices: found
            .slices
            .iter()
            .map(|s| SliceDto {
                first: s.first.get(),
                last: s.last.get(),
                state: match s.state {
                    SliceState::Used => "used",
                    SliceState::Reserved => "reserved",
                    SliceState::Free => "free",
                },
            })
            .collect(),
        used: found.used,
        reserved: found.reserved,
        free: found.free,
    };
    kit::record_read(
        &api,
        &context,
        crate::module::GROUP,
        &[id],
        json!({"occupancy": id}),
    )
    .await?;
    Ok(respond(
        &headers,
        &moment,
        &ETag::page(&serde_json::to_vec(&body).expect("JSON")),
        Json(body),
    ))
}

/// `GET /v1/vlan-groups/{id}/next-free[?from=]`: the smallest free VID, 404 `exhausted`
/// when none is left from there.
async fn group_next_free(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let mut from = None;
    for (name, value) in form_pairs(query.as_deref().unwrap_or_default()) {
        match name.as_str() {
            "from" if from.is_none() => {
                from = Some(
                    Vid::new(i64::from(vid("from", &value)?))
                        .map_err(|_| kit::bad("from must be a VID"))?,
                );
            }
            "valid_at" | "as_of" => {}
            other => return Err(kit::bad(format!("unknown parameter {other}"))),
        }
    }
    let moment = dated_read(&api, &context, group_type(), id, dates).await?;
    let row = group_at(&api, id, &moment).await?;
    let (range, live) = living(&api, &row, &moment).await?;
    let taken: Vec<Vid> = live.iter().map(|(v, _)| *v).collect();
    kit::record_read(
        &api,
        &context,
        crate::module::GROUP,
        &[id],
        json!({"next_free": id}),
    )
    .await?;
    let vid = next_free(range, &taken, from).ok_or_else(|| {
        Problem::new(ProblemKind::Exhausted).detail("no VID is free in the group from there")
    })?;
    let body = NextFreeDto {
        group: IdRef { id },
        vid: vid.get(),
    };
    Ok(respond(
        &headers,
        &moment,
        &ETag::page(&serde_json::to_vec(&body).expect("JSON")),
        Json(body),
    ))
}

fn form_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (name, value) = p.split_once('=').unwrap_or((p, ""));
            (name.to_owned(), value.to_owned())
        })
        .collect()
}

/// `GET /v1/vlans/{id}/prefixes`: the networks that carry the VLAN at the date, those the
/// caller may read, in the order of the prefixes (REQ-008-21).
async fn vlan_prefixes(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let name = format!("vlans/{id}/prefixes");
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, &name)?;
    page.unfiltered()?;
    kit::gate(&api, &context, vlan_type(), id).await?;
    let (moment, scope) = reading(&api, &principal, &page).await?;
    let relation = snapshot(
        "property_assignment",
        &["object_id", "property_id", "kind", "v_uuid"],
    )
    .map_err(|e| Problem::new(ProblemKind::Internal).detail(e.to_string()))?;
    let carriers: Vec<Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT DISTINCT a.object_id FROM {relation} a
           JOIN property_declaration d ON d.id = a.property_id AND d.key = 'vlan.vlan'
          WHERE a.kind = 'value' AND a.v_uuid = $3"
    )))
    .bind(moment.valid_at)
    .bind(moment.as_of)
    .bind(id)
    .fetch_all(&api.pool)
    .await
    .map_err(|e| kit::store(&e))?;
    let after = page
        .after()
        .map(|key| serde_json::from_value::<(Option<Uuid>, String, Uuid)>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))?
        .map(|(domain, value, id)| Key { domain, value, id });
    let mut rows = if carriers.is_empty() {
        Vec::new()
    } else {
        list::objects(
            &api.pool,
            Kind::Prefix,
            &Criteria {
                ids: &carriers,
                ..Criteria::default()
            },
            moment.valid_at,
            Some(moment.as_of()),
            after.as_ref(),
            i64::from(page.size) + 1,
            &scope,
        )
        .await
        .map_err(|e| kit::store(&e))?
    };
    let size = usize::from(page.size);
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let last = &rows[size - 1].key;
        let cursor = page.next(
            &api.ring,
            &name,
            &moment,
            json!([last.domain, last.value, last.id]),
        );
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let items: Vec<NetworkDto> = rows
        .iter()
        .map(|row| NetworkDto {
            id: row.key.id,
            prefix: row.fields["prefix"]
                .as_str()
                .unwrap_or(&row.key.value)
                .to_owned(),
            href: format!("/v1/prefixes/{}", row.key.id),
        })
        .collect();
    let ids: Vec<Uuid> = items.iter().map(|i| i.id).collect();
    kit::record_read(
        &api,
        &context,
        "prefix",
        &ids,
        criteria(&name, &page, &moment),
    )
    .await?;
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

// ---------------------------------------------------------------- the routes

/// The schemas of the routes.
fn schemas() -> Vec<(String, RefOr<Schema>)> {
    let mut schemas = Vec::new();
    schemas_of::<GroupDto>(&mut schemas);
    schemas_of::<VlanDto>(&mut schemas);
    schemas_of::<OccupancyDto>(&mut schemas);
    schemas_of::<NextFreeDto>(&mut schemas);
    schemas_of::<NetworkDto>(&mut schemas);
    schemas_of::<GroupCreation>(&mut schemas);
    schemas_of::<GroupPatch>(&mut schemas);
    schemas_of::<VlanCreation>(&mut schemas);
    schemas_of::<VlanPatch>(&mut schemas);
    schemas_of::<Closure>(&mut schemas);
    schemas.push(listing_of::<GroupDto>());
    schemas.push(listing_of::<VlanDto>());
    schemas.push(listing_of::<NetworkDto>());
    schemas
}

/// The collection `name` of `ty`, its objects and their writes.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)] // the contract of each route beside its handlers
fn collection_routes(
    router: ModuleRouter,
    ty: &'static ObjectType,
    one: &str,
    dto: &str,
    bodies: (&str, &str),
    filters: &[(&str, &str)],
    members: &'static [&'static str],
    operations: (&'static str, &'static str),
) -> ModuleRouter {
    let name = ty.collection;
    let operation_name = name.replace('-', "_");
    let mut list_op = op(
        &format!("list_{operation_name}"),
        "vlan",
        &format!("The {name} the caller may read."),
    );
    for (filter, description) in filters {
        list_op = list_op.query(filter, description);
    }
    let list_op = list_op
        .params::<DatesParams>()
        .params::<PageParams>()
        .answers(
            "200",
            "A page of the collection.",
            &format!("Listing_{dto}"),
        )
        .build();
    let create_op = op(
        &format!("create_{one}"),
        "vlan",
        &format!("Creates a {one}."),
    )
    .header(
        "Idempotency-Key",
        false,
        "Makes the write idempotent for 24 h.",
    )
    .body("application/json", bodies.0, true)
    .answers("201", "Created.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    let list = if ty.kind == crate::module::GROUP {
        kit::collection(list_groups)
    } else {
        kit::collection(list_vlans)
    };
    let (create, modify) = operations;
    let mut router = router.routes(documented(
        &format!("/{name}"),
        vec![(HttpMethod::Get, list_op), (HttpMethod::Post, create_op)],
        schemas(),
        list.post(
            move |State(api): State<Api>,
                  Extension(principal): Extension<Principal>,
                  Extension(context): Extension<RequestContext>,
                  headers: HeaderMap,
                  raw: Bytes| async move {
                kit::create(
                    &api, &principal, &context, &headers, &raw, ty, create, members,
                )
                .await
            },
        ),
    ));
    router = queryable(
        router,
        &format!("/{name}"),
        &format!("query_{operation_name}"),
    );
    let the = format!("The {one}.");
    let show_op = op(&format!("show_{one}"), "vlan", &the)
        .path("id", &the, true)
        .params::<DatesParams>()
        .answers("200", "The object; its version is its ETag.", dto)
        .build();
    let modify_op = op(
        &format!("modify_{one}"),
        "vlan",
        &format!("Modifies the {one} (JSON Merge Patch)."),
    )
    .path("id", &the, true)
    .header("If-Match", true, "The version read, as its ETag.")
    .body(MERGE_PATCH, bodies.1, true)
    .answers("200", "Modified.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    let close_op = op(
        &format!("close_{one}"),
        "vlan",
        &format!("Closes the {one}."),
    )
    .path("id", &the, true)
    .header("If-Match", true, "The version read, as its ETag.")
    .body("application/json", "VlanClosure", false)
    .answers("200", "Closed.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    let show = if ty.kind == crate::module::GROUP {
        get(show_group)
    } else {
        get(show_vlan)
    };
    router.routes(documented(
        &format!("/{name}/{{id}}"),
        vec![
            (HttpMethod::Get, show_op),
            (HttpMethod::Patch, modify_op),
            (HttpMethod::Delete, close_op),
        ],
        Vec::new(),
        show.patch(
            move |State(api): State<Api>,
                  Extension(principal): Extension<Principal>,
                  Extension(context): Extension<RequestContext>,
                  Path(id): Path<Uuid>,
                  headers: HeaderMap,
                  raw: Bytes| async move {
                kit::modify(
                    &api, &principal, &context, id, &headers, &raw, ty, modify, members,
                )
                .await
            },
        )
        .delete(
            move |State(api): State<Api>,
                  Extension(principal): Extension<Principal>,
                  Extension(context): Extension<RequestContext>,
                  Path(id): Path<Uuid>,
                  headers: HeaderMap,
                  raw: Bytes| async move {
                kit::close(&api, &principal, &context, id, &headers, &raw, ty).await
            },
        ),
    ))
}

/// The routes of the module, mounted under `/v1` (ADR 0026, clause 9).
#[must_use]
pub fn routes() -> ModuleRouter {
    let mut router = ModuleRouter::new();
    router = collection_routes(
        router,
        group_type(),
        "vlan_group",
        "VlanGroup",
        ("VlanGroupCreation", "VlanGroupPatch"),
        &[
            ("slug", "Equal to."),
            (
                "anchor",
                "The entity, site or domain the group is anchored on.",
            ),
        ],
        GROUP_MEMBERS,
        ("vlan.create_group", "vlan.update_group"),
    );
    router = collection_routes(
        router,
        vlan_type(),
        "vlan",
        "Vlan",
        ("VlanCreation", "VlanPatch"),
        &[
            ("group", "The group."),
            ("vid", "A VID, or a range `a..b`."),
            ("name", "Equal to."),
            ("name_prefix", "Starting with."),
            ("status", "`active`, `reserved` or `deprecated`."),
        ],
        VLAN_MEMBERS,
        ("vlan.create_vlan", "vlan.update_vlan"),
    );
    let group = "The group.";
    let occupancy_op = op(
        "vlan_group_occupancy",
        "vlan",
        "The used, reserved and free VIDs of the group, in runs (REQ-008-20).",
    )
    .path("id", group, true)
    .params::<DatesParams>()
    .answers("200", "The occupancy.", "VlanOccupancy")
    .build();
    let next_op = op(
        "vlan_group_next_free",
        "vlan",
        "The smallest free VID of the group, at or after `from`; 404 `exhausted` when none.",
    )
    .path("id", group, true)
    .query(
        "from",
        "The VID to start from; the first of the range by default.",
    )
    .params::<DatesParams>()
    .answers("200", "The next free VID.", "VlanNextFree")
    .build();
    let prefixes_op = op(
        "vlan_prefixes",
        "vlan",
        "The networks that carry the VLAN, those the caller may read (REQ-008-21).",
    )
    .path("id", "The VLAN.", true)
    .params::<DatesParams>()
    .params::<PageParams>()
    .answers("200", "A page of networks.", "Listing_VlanNetwork")
    .build();
    router
        .routes(documented(
            "/vlan-groups/{id}/occupancy",
            vec![(HttpMethod::Get, occupancy_op)],
            Vec::new(),
            get(group_occupancy),
        ))
        .routes(documented(
            "/vlan-groups/{id}/next-free",
            vec![(HttpMethod::Get, next_op)],
            Vec::new(),
            get(group_next_free),
        ))
        .routes(documented(
            "/vlans/{id}/prefixes",
            vec![(HttpMethod::Get, prefixes_op)],
            Vec::new(),
            get(vlan_prefixes),
        ))
}
