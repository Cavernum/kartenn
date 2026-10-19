//! Imports (spec 006, REQ-006-17 to 19; task 006-21; design « Imports et tâches de
//! fond », arbitrage of 2026-10-19): `POST /v1/imports` opens an import, `PUT
//! …/chunks/{n}` deposits numbered chunks of at most [`MAX_CHUNK`] elements, `POST
//! …/start` sorts and cuts them in batches run by the background task `import:<id>`,
//! `…/resume` and `…/cancel` steer it, `GET …/{id}` follows it. Only its author sees an
//! import: for anyone else it does not exist (404).

use axum::Extension;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kartenn_service::authn::Principal;
use kartenn_service::import::{ImportStatus, batches, import_status, open_import};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::ImportItem;
use kartenn_store_pg::imports::{self, Deposit, ImportHeader, Start};
use kartenn_store_pg::write::PgStore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::objects::{Api, bad};
use crate::operation::ImportItemDto;
use crate::problem::{Problem, ProblemKind};
use crate::quota::{Charge, chunk_weight};
use crate::resources::{members, typed};

/// The most elements of a chunk (design « Transport »).
pub const MAX_CHUNK: usize = 10_000;

/// The indicative wait between two reads of an import that runs, in seconds.
const RETRY_AFTER: u32 = 5;

/// The routes of the imports.
pub fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::new()
        .routes(routes!(create))
        .routes(routes!(show))
        .routes(routes!(deposit))
        .routes(routes!(start))
        .routes(routes!(resume))
        .routes(routes!(cancel))
}

/// The path of an import.
#[must_use]
pub fn href(id: Uuid) -> String {
    format!("/v1/imports/{id}")
}

/// The body of `POST /v1/imports`.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct NewImport {
    /// The source of the elements, not empty.
    source: String,
}

/// The body of a chunk.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct Chunk {
    /// The elements, at most 10 000.
    elements: Vec<ImportItemDto>,
}

/// An import, as followed by its author.
#[derive(Debug, Serialize, ToSchema)]
struct ImportDto {
    /// Identifier.
    id: Uuid,
    /// The source of the elements.
    source: String,
    /// `uploading`, `ready`, `running`, `paused`, `done`, `failed` or `cancelled`.
    state: String,
    /// The chunks received.
    chunks: i64,
    /// The elements received.
    elements: i64,
    /// The batches.
    batches: Batches,
    /// The report of the batches run.
    #[schema(value_type = Object)]
    report: Value,
    /// Links.
    links: ImportLinks,
}

/// The batches of an import.
#[derive(Debug, Serialize, ToSchema)]
struct Batches {
    /// Applied.
    applied: usize,
    /// Still to run.
    remaining: usize,
}

/// The links of an import.
#[derive(Debug, Serialize, ToSchema)]
struct ImportLinks {
    /// The import.
    #[serde(rename = "self")]
    this: String,
}

/// The import `id` if `principal` wrote it; 404 otherwise.
async fn owned(api: &Api, principal: &Principal, id: Uuid) -> Result<ImportHeader, Problem> {
    match imports::header(&api.pool, id).await? {
        Some(found) if found.author == principal.identity => Ok(found),
        _ => Err(Problem::new(ProblemKind::NotFound)),
    }
}

/// The representation of import `id`, `Retry-After` while it waits or runs.
async fn represented(api: &Api, id: Uuid, status: StatusCode) -> Result<Response, Problem> {
    let header = imports::header(&api.pool, id)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let progress = import_status(&PgStore::new(api.pool.clone()), id)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let running = matches!(progress.state.as_str(), "ready" | "running");
    let mut response = (status, axum::Json(representation(id, header, progress))).into_response();
    if running {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from(RETRY_AFTER));
    }
    Ok(response)
}

fn representation(id: Uuid, header: ImportHeader, progress: ImportStatus) -> ImportDto {
    ImportDto {
        id,
        source: header.source,
        state: progress.state,
        chunks: header.chunks,
        elements: header.elements,
        batches: Batches {
            applied: progress.applied,
            remaining: progress.remaining,
        },
        report: progress.report,
        links: ImportLinks { this: href(id) },
    }
}

/// With `Location` to import `id`.
fn located(mut response: Response, id: Uuid) -> Response {
    if let Ok(location) = HeaderValue::from_str(&href(id)) {
        response.headers_mut().insert(header::LOCATION, location);
    }
    response
}

fn conflict(detail: &str) -> Problem {
    Problem::new(ProblemKind::Conflict).detail(detail)
}

/// `POST /v1/imports {source}` → 201.
#[utoipa::path(
    post,
    path = "/imports",
    operation_id = "create_import",
    tag = "imports",
    request_body = NewImport,
    responses((status = 201, description = "The import, open to chunks.", body = ImportDto))
)]
async fn create(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let NewImport { source } = typed(
        "import",
        Value::Object(members(&headers, &raw, "application/json")?),
    )?;
    if source.is_empty() {
        return Err(bad("source must not be empty"));
    }
    let id = open_import(&PgStore::new(api.pool.clone()), principal.identity, &source).await?;
    Ok(located(
        represented(&api, id, StatusCode::CREATED).await?,
        id,
    ))
}

/// `GET /v1/imports/{id}`.
#[utoipa::path(
    get,
    path = "/imports/{id}",
    operation_id = "show_import",
    tag = "imports",
    params(("id" = Uuid, Path, description = "The import.")),
    responses((status = 200, description = "The import.", body = ImportDto))
)]
async fn show(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    owned(&api, &principal, id).await?;
    represented(&api, id, StatusCode::OK).await
}

/// `PUT /v1/imports/{id}/chunks/{n} {elements}`: new → 201, identical → 200, different
/// → 409, too large → 413; the chunk weighs `⌈elements / 1 000⌉` on the quota.
#[utoipa::path(
    put,
    path = "/imports/{id}/chunks/{n}",
    operation_id = "deposit_import_chunk",
    tag = "imports",
    params(
        ("id" = Uuid, Path, description = "The import."),
        ("n" = i32, Path, description = "The number of the chunk, 0 or more."),
    ),
    request_body = Chunk,
    responses(
        (status = 201, description = "The chunk is received.", body = ImportDto),
        (status = 200, description = "The same chunk was received already.", body = ImportDto),
    )
)]
async fn deposit(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    charge: Option<Extension<Charge>>,
    Path((id, n)): Path<(Uuid, i32)>,
    headers: HeaderMap,
    raw: Bytes,
) -> Response {
    let elements = match chunk(n, &headers, &raw) {
        Ok(elements) => elements,
        Err(problem) => return problem.into_response(),
    };
    if let Some(Extension(charge)) = charge
        && let Err(refused) = charge.weigh(chunk_weight(elements.len()))
    {
        return refused.into_response();
    }
    deposited(&api, &principal, id, n, &elements)
        .await
        .into_response()
}

/// The elements of chunk `n` from its body: 400 for a negative number or a malformed body,
/// 413 beyond [`MAX_CHUNK`] elements.
fn chunk(n: i32, headers: &HeaderMap, raw: &[u8]) -> Result<Vec<ImportItem>, Problem> {
    if n < 0 {
        return Err(bad("the number of a chunk is 0 or more"));
    }
    let chunk: Chunk = typed(
        "chunk",
        Value::Object(members(headers, raw, "application/json")?),
    )?;
    if chunk.elements.len() > MAX_CHUNK {
        return Err(Problem::new(ProblemKind::PayloadTooLarge)
            .detail(format!("a chunk holds at most {MAX_CHUNK} elements")));
    }
    Ok(chunk.elements.into_iter().map(Into::into).collect())
}

async fn deposited(
    api: &Api,
    principal: &Principal,
    id: Uuid,
    n: i32,
    elements: &[ImportItem],
) -> Result<Response, Problem> {
    owned(api, principal, id).await?;
    let serialized = serde_json::to_vec(elements).map_err(|e| bad(e.to_string()))?;
    let digest = Sha256::digest(serialized);
    let status = match imports::deposit(&api.pool, id, n, &digest, elements).await? {
        Deposit::Created => StatusCode::CREATED,
        Deposit::Same => StatusCode::OK,
        Deposit::Different => {
            return Err(conflict("another chunk was received under this number"));
        }
        Deposit::Closed => return Err(conflict("the import no longer takes chunks")),
    };
    represented(api, id, status).await
}

/// `POST /v1/imports/{id}/start` → 202: sorted, cut in batches and ready.
#[utoipa::path(
    post,
    path = "/imports/{id}/start",
    operation_id = "start_import",
    tag = "imports",
    params(("id" = Uuid, Path, description = "The import.")),
    responses((status = 202, description = "The import is ready to run.", body = ImportDto))
)]
async fn start(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    owned(&api, &principal, id).await?;
    match imports::start(&api.pool, id, |items| batches(items, MAX_OPERATIONS)).await? {
        Start::Started => Ok(located(
            represented(&api, id, StatusCode::ACCEPTED).await?,
            id,
        )),
        Start::Closed => Err(conflict("the import is started already")),
        Start::Invalid(e) => Err(e.into()),
    }
}

/// `POST /v1/imports/{id}/resume` → 202: a paused import is ready again.
#[utoipa::path(
    post,
    path = "/imports/{id}/resume",
    operation_id = "resume_import",
    tag = "imports",
    params(("id" = Uuid, Path, description = "The import.")),
    responses((status = 202, description = "The import is ready again.", body = ImportDto))
)]
async fn resume(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    owned(&api, &principal, id).await?;
    if !imports::transition(&api.pool, id, &["paused"], "ready", true).await? {
        return Err(conflict("only a paused import resumes"));
    }
    Ok(located(
        represented(&api, id, StatusCode::ACCEPTED).await?,
        id,
    ))
}

/// `POST /v1/imports/{id}/cancel` → 200: the batch in progress ends, the batches applied
/// stay.
#[utoipa::path(
    post,
    path = "/imports/{id}/cancel",
    operation_id = "cancel_import",
    tag = "imports",
    params(("id" = Uuid, Path, description = "The import.")),
    responses((status = 200, description = "The import is cancelled.", body = ImportDto))
)]
async fn cancel(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    owned(&api, &principal, id).await?;
    let open = ["uploading", "ready", "running", "paused"];
    if !imports::transition(&api.pool, id, &open, "cancelled", false).await? {
        return Err(conflict("the import is over"));
    }
    represented(&api, id, StatusCode::OK).await
}
