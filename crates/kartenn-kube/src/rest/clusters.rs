//! The collection of the clusters (task 019-8a; REQ-019-5, 38; design « Routes »,
//! « Données »): `GET, POST /v1/kube-clusters` (filter `?slug=`, suggestion `q`) and
//! `GET, PATCH, DELETE /v1/kube-clusters/{id}`. Reads dated (`valid_at`, `as_of`), under
//! the rights of the caller (`kube.cluster.read`, on its owner and site: a cluster is
//! anchored in no domain) and journaled; writes by changeset, `kube.create_cluster` and
//! `kube.update_cluster` in JSON Merge Patch under `If-Match`, the closure by the core.

use axum::Extension;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use kartenn_rest::ModuleRouter;
use kartenn_rest::kit::{
    self, Api, Dates, DatesParams, ListQuery, MERGE_PATCH, Page, PageParams, Problem, ProblemKind,
    documented, listing_of, next_href, op, presented, queryable, respond_page, schemas_of,
};
use kartenn_service::audit::RequestContext;
use kartenn_service::authn::Principal;
use kartenn_store_pg::rbac::readable;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use sqlx::postgres::PgRow;
use utoipa::ToSchema;
use utoipa::openapi::path::HttpMethod;
use uuid::Uuid;

use super::IdRef;
use super::reading::{criteria, dated, listed};
use crate::module::{CLUSTER, cluster_type};

/// The collection, under `/v1`.
const COLLECTION: &str = "kube-clusters";

/// The members of a creation (`kube.create_cluster`, `CLUSTER_FIELDS` of the handler,
/// with `owner` and `site`).
const CREATE: &[&str] = &[
    "slug",
    "owner",
    "site",
    "pod_domain",
    "service_domain",
    "external_domain",
    "pod_containers",
    "service_networks",
    "machines_cluster",
    "retention_days",
    "tracked_label_keys",
    "batch_window_seconds",
    "batch_max_operations",
    "create_node_machines",
    "connector_group",
];

/// The members a patch removes with `null` (RFC 7396): the optional references.
const NULLABLE: &[&str] = &[
    "site",
    "external_domain",
    "machines_cluster",
    "connector_group",
];

// ---------------------------------------------------------------- representations

/// A declared cluster (REQ-019-5).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = KubeCluster)]
struct ClusterDto {
    /// Identifier.
    id: Uuid,
    /// Slug, unique among the living clusters.
    slug: String,
    /// Its resolved owner.
    owner: Option<IdRef>,
    /// Its resolved site.
    site: Option<IdRef>,
    /// The addressing domain of its pods (and of its nodes and namespaces).
    pod_domain: IdRef,
    /// The addressing domain of its services.
    service_domain: IdRef,
    /// The addressing domain of its `LoadBalancer` addresses and `externalIPs`; those of
    /// the services when `null`.
    external_domain: Option<IdRef>,
    /// The containers of the core that hold its pod ranges.
    pod_containers: Vec<IdRef>,
    /// The networks of the core of its services.
    service_networks: Vec<IdRef>,
    /// The cluster of machines (`machines.cluster`) it runs on, shown only.
    machines_cluster: Option<IdRef>,
    /// The days its observations are kept, 1 to 400.
    retention_days: i16,
    /// The label keys it tracks beyond those of the module.
    tracked_label_keys: Vec<String>,
    /// The window of a batch of its connector, in seconds, 1 to 300.
    batch_window_seconds: i16,
    /// The most operations of a batch of its connector, 1 to 10 000.
    batch_max_operations: i32,
    /// Whether a node with no machine creates one.
    create_node_machines: bool,
    /// The group of kartenn of its connector.
    connector_group: Option<IdRef>,
    /// Its URL.
    href: String,
}

/// The body of the declaration of a cluster.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = KubeClusterCreation)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct ClusterCreation {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Slug.
    slug: String,
    /// The owner (`core.owner`).
    owner: IdRef,
    /// The site (`core.site`).
    site: Option<IdRef>,
    /// The addressing domain of its pods.
    pod_domain: IdRef,
    /// The addressing domain of its services.
    service_domain: IdRef,
    /// The addressing domain of its external addresses; those of the services by default.
    external_domain: Option<IdRef>,
    /// Containers of the pod domain; none by default.
    pod_containers: Option<Vec<IdRef>>,
    /// Networks of the service domain; none by default.
    service_networks: Option<Vec<IdRef>>,
    /// The cluster of machines it runs on.
    machines_cluster: Option<IdRef>,
    /// 1 to 400; 30 by default.
    retention_days: Option<i16>,
    /// Label keys tracked beyond those of the module; none by default.
    tracked_label_keys: Option<Vec<String>>,
    /// 1 to 300; 10 by default.
    batch_window_seconds: Option<i16>,
    /// 1 to 10 000; 500 by default.
    batch_max_operations: Option<i32>,
    /// `false` by default.
    create_node_machines: Option<bool>,
    /// The group of kartenn of its connector.
    connector_group: Option<IdRef>,
}

/// The body of the modification of a cluster (JSON Merge Patch); `null` removes `site`,
/// `external_domain`, `machines_cluster` and `connector_group`.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = KubeClusterPatch)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct ClusterPatch {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Slug.
    slug: Option<String>,
    /// The owner.
    owner: Option<IdRef>,
    /// The site.
    site: Option<IdRef>,
    /// The addressing domain of its pods, refused while it has living objects.
    pod_domain: Option<IdRef>,
    /// The addressing domain of its services.
    service_domain: Option<IdRef>,
    /// The addressing domain of its external addresses.
    external_domain: Option<IdRef>,
    /// Containers of the pod domain.
    pod_containers: Option<Vec<IdRef>>,
    /// Networks of the service domain.
    service_networks: Option<Vec<IdRef>>,
    /// The cluster of machines it runs on.
    machines_cluster: Option<IdRef>,
    /// 1 to 400.
    retention_days: Option<i16>,
    /// Label keys tracked beyond those of the module.
    tracked_label_keys: Option<Vec<String>>,
    /// 1 to 300.
    batch_window_seconds: Option<i16>,
    /// 1 to 10 000.
    batch_max_operations: Option<i32>,
    /// Whether a node with no machine creates one.
    create_node_machines: Option<bool>,
    /// The group of kartenn of its connector.
    connector_group: Option<IdRef>,
}

/// The optional body of the closure of a cluster.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = KubeClosure)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct Closure {
    /// Why; required by the rules.
    motive: String,
    /// The start of the closure, RFC 3339; now by default.
    valid_from: Option<String>,
    /// Also close what cites the cluster: its nodes and namespaces, and theirs.
    cascade: Option<bool>,
}

// ---------------------------------------------------------------- reading

/// The columns of a cluster in its table.
const COLUMNS: &str = "id, slug, pod_domain_id, service_domain_id, external_domain_id, \
     pod_container_ids, service_network_ids, machines_cluster_id, retention_days, \
     tracked_label_keys, batch_window_seconds, batch_max_operations, create_node_machines, \
     connector_group_id";

/// The select list of a cluster: its columns, then its resolved owner and site at `$1`.
const SELECT: &str = "x.id, x.slug, x.pod_domain_id, x.service_domain_id, \
     x.external_domain_id, x.pod_container_ids, x.service_network_ids, x.machines_cluster_id, \
     x.retention_days, x.tracked_label_keys, x.batch_window_seconds, x.batch_max_operations, \
     x.create_node_machines, x.connector_group_id, kartenn_owner(x.id, $1) AS owner, \
     kartenn_site(x.id, $1) AS site";

/// A row of [`SELECT`].
struct ClusterRow(ClusterDto);

impl<'r> sqlx::FromRow<'r, PgRow> for ClusterRow {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let one = |name: &str| row.try_get::<Uuid, _>(name).map(|id| IdRef { id });
        let optional = |name: &str| {
            Ok::<_, sqlx::Error>(row.try_get::<Option<Uuid>, _>(name)?.map(|id| IdRef { id }))
        };
        let many = |name: &str| {
            Ok::<_, sqlx::Error>(
                row.try_get::<Vec<Uuid>, _>(name)?
                    .into_iter()
                    .map(|id| IdRef { id })
                    .collect(),
            )
        };
        let id: Uuid = row.try_get("id")?;
        Ok(Self(ClusterDto {
            id,
            slug: row.try_get("slug")?,
            owner: optional("owner")?,
            site: optional("site")?,
            pod_domain: one("pod_domain_id")?,
            service_domain: one("service_domain_id")?,
            external_domain: optional("external_domain_id")?,
            pod_containers: many("pod_container_ids")?,
            service_networks: many("service_network_ids")?,
            machines_cluster: optional("machines_cluster_id")?,
            retention_days: row.try_get("retention_days")?,
            tracked_label_keys: row.try_get("tracked_label_keys")?,
            batch_window_seconds: row.try_get("batch_window_seconds")?,
            batch_max_operations: row.try_get("batch_max_operations")?,
            create_node_machines: row.try_get("create_node_machines")?,
            connector_group: optional("connector_group_id")?,
            href: format!("/v1/{COLLECTION}/{id}"),
        }))
    }
}

/// The table of the clusters.
const TABLE: &str = "kube.cluster";

/// `GET`/`QUERY /v1/kube-clusters`: by slug, filtered by `slug`, suggested by `q`.
async fn list(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    ListQuery(query): ListQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let (page, prefix) = kit::suggestion(Page::from_query(&query, &api.ring, COLLECTION)?)?;
    let mut slug = None;
    for (name, value) in &page.filters {
        match name.as_str() {
            "slug" if slug.is_none() => slug = Some(value.as_str()),
            "slug" => return Err(kit::bad("slug is given more than once")),
            "q" => {}
            _ => return Err(kit::bad(format!("unknown parameter {name}"))),
        }
    }
    let after = page
        .after()
        .map(|key| serde_json::from_value::<(String, Uuid)>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))?;
    let now = kit::now(&api).await?;
    let moment = page.dates.at(now, api.bound)?;
    let scope = kit::scope(&api, &principal, now).await?;
    let sql = format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT {SELECT}
           FROM {} x
          WHERE ($4::text IS NULL OR x.slug = $4)
            AND ($5::text IS NULL OR (x.slug, x.id) > ($5, $6::uuid))
            AND ($8::text IS NULL OR x.slug LIKE $8)
            AND ($3::jsonb IS NULL OR {})
          ORDER BY x.slug, x.id
          LIMIT $7",
        listed(TABLE, COLUMNS, moment.as_of.is_none()),
        readable(
            "x.id",
            &format!("'{CLUSTER}'"),
            "NULL::uuid",
            "NULL::inet",
            "$1"
        ),
    );
    // The rows of the page and their versions are read in one snapshot (task 021-11c).
    let mut snapshot = kit::snapshot(&api).await?;
    let mut rows: Vec<ClusterRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        // A prefix `q` is planned with its value (an unnamed statement).
        .persistent(prefix.is_none())
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(scope.to_json())
        .bind(slug)
        .bind(after.as_ref().map(|(slug, _)| slug.as_str()))
        .bind(after.as_ref().map(|(_, id)| *id))
        .bind(i64::from(page.size) + 1)
        .bind(prefix.as_deref())
        .fetch_all(&mut *snapshot)
        .await
        .map_err(|e| kit::store(&e))?;
    let size = usize::from(page.size);
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let last = &rows[size - 1].0;
        let cursor = page.next(&api.ring, COLLECTION, &moment, json!([last.slug, last.id]));
        next_href(&format!("/v1/{COLLECTION}"), &cursor)
    });
    let ids: Vec<Uuid> = rows.iter().map(|r| r.0.id).collect();
    let versions = kit::versions(&mut *snapshot, &moment, &ids).await?;
    kit::release(snapshot);
    kit::record_read(
        &api,
        &context,
        CLUSTER,
        &ids,
        criteria(COLLECTION, &page, &moment),
    )
    .await?;
    let items: Vec<ClusterDto> = rows.into_iter().map(|r| r.0).collect();
    let items = kit::versioned(&items, &versions);
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// `GET /v1/kube-clusters/{id}`: 404, journaled, for a cluster the caller may not read.
async fn show(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    kit::gate(&api, &context, cluster_type(), id).await?;
    let now = kit::now(&api).await?;
    let moment = dates.at(now, api.bound)?;
    let sql = format!(
        "SELECT {SELECT} FROM {} x WHERE x.id = $3",
        dated(TABLE, COLUMNS)
    );
    let (ClusterRow(body), version) = kit::read_one(&api, sql, id, &moment).await?;
    kit::record_read(&api, &context, CLUSTER, &[id], json!({"id": id})).await?;
    Ok(kit::respond_object(&headers, &moment, version, &body))
}

// ---------------------------------------------------------------- routes

/// The routes of the collection of the clusters.
#[allow(clippy::too_many_lines)] // the contract of each route beside its handlers
pub(super) fn routes(router: ModuleRouter) -> ModuleRouter {
    let mut schemas = Vec::new();
    schemas_of::<ClusterDto>(&mut schemas);
    schemas_of::<ClusterCreation>(&mut schemas);
    schemas_of::<ClusterPatch>(&mut schemas);
    schemas_of::<Closure>(&mut schemas);
    schemas.push(listing_of::<ClusterDto>());
    let list_op = op(
        "list_kube_clusters",
        "kube",
        "The clusters the caller may read (REQ-019-38), by slug.",
    )
    .query("slug", "Equal to.")
    .suggestion("the slug")
    .params::<DatesParams>()
    .params::<PageParams>()
    .answers("200", "A page of the collection.", "Listing_KubeCluster")
    .build();
    let create_op = op(
        "create_kube_cluster",
        "kube",
        "Declares a cluster (REQ-019-5): its slug, owner, pod and service domains are \
         required; the ranges cited lie in the domains it designates.",
    )
    .header(
        "Idempotency-Key",
        false,
        "Makes the write idempotent for 24 h.",
    )
    .body("application/json", "KubeClusterCreation", true)
    .answers("201", "Created.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    let the = "The cluster.";
    let show_op = op("show_kube_cluster", "kube", the)
        .path("id", the, true)
        .params::<DatesParams>()
        .answers("200", kit::OBJECT_READ, "KubeCluster")
        .build();
    let modify_op = op(
        "modify_kube_cluster",
        "kube",
        "Modifies the declaration of the cluster (JSON Merge Patch).",
    )
    .path("id", the, true)
    .header("If-Match", true, kit::IF_MATCH_OBJECT)
    .body(MERGE_PATCH, "KubeClusterPatch", true)
    .answers("200", "Modified.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    let close_op = op("close_kube_cluster", "kube", "Closes the cluster.")
        .path("id", the, true)
        .header("If-Match", true, kit::IF_MATCH_OBJECT)
        .body("application/json", "KubeClosure", false)
        .answers("200", "Closed.", "ChangesetApplied")
        .answers("202", "Held for a decision.", "ChangesetPending")
        .build();
    let path = format!("/{COLLECTION}");
    let mut router = router.routes(documented(
        &path,
        vec![(HttpMethod::Get, list_op), (HttpMethod::Post, create_op)],
        schemas,
        kit::collection(list).post(
            |State(api): State<Api>,
             Extension(principal): Extension<Principal>,
             Extension(context): Extension<RequestContext>,
             headers: HeaderMap,
             raw: Bytes| async move {
                kit::create(
                    &api,
                    &principal,
                    &context,
                    &headers,
                    &raw,
                    cluster_type(),
                    "kube.create_cluster",
                    CREATE,
                )
                .await
            },
        ),
    ));
    router = queryable(router, &path, "query_kube_clusters");
    router = router.routes(documented(
        &format!("{path}/{{id}}"),
        vec![
            (HttpMethod::Get, show_op),
            (HttpMethod::Patch, modify_op),
            (HttpMethod::Delete, close_op),
        ],
        Vec::new(),
        get(show)
            .patch(
                |State(api): State<Api>,
                 Extension(principal): Extension<Principal>,
                 Extension(context): Extension<RequestContext>,
                 Path(id): Path<Uuid>,
                 headers: HeaderMap,
                 raw: Bytes| async move {
                    kit::modify_nullable(
                        &api,
                        &principal,
                        &context,
                        id,
                        &headers,
                        &raw,
                        cluster_type(),
                        ("kube.update_cluster", CREATE, NULLABLE),
                    )
                    .await
                },
            )
            .delete(
                |State(api): State<Api>,
                 Extension(principal): Extension<Principal>,
                 Extension(context): Extension<RequestContext>,
                 Path(id): Path<Uuid>,
                 headers: HeaderMap,
                 raw: Bytes| async move {
                    kit::close(
                        &api,
                        &principal,
                        &context,
                        id,
                        &headers,
                        &raw,
                        cluster_type(),
                    )
                    .await
                },
            ),
    ));
    presented(
        router,
        &path,
        &["slug"],
        &["slug", "owner", "site", "pod_domain", "service_domain"],
    )
}
