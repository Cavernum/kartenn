//! Anomalies (spec 006, REQ-006-4, 23; task 006-12c; 002/REQ-002-36): the open anomalies
//! of the objects the caller may read, filtered by rule, domain or entity; their
//! acknowledgement with a motive, which needs `core.anomaly.acknowledge` on the object of
//! the anomaly and is audited. The acknowledgement holds for the current version of the
//! object: no `If-Match`. The anomaly of an object the caller may not read answers 404, as
//! an absent one.

use axum::Extension;
use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kartenn_core::audit::{ANOMALY_ACKNOWLEDGED, AUTHZ_DENIED, rfc3339};
use kartenn_core::authz::ANOMALY_ACKNOWLEDGE;
use kartenn_service::RequestError;
use kartenn_service::audit::{AuditJournal, Outcome, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::authz::Authorizer;
use kartenn_service::reads::objects_read;
use kartenn_store_pg::anomalies::{self, Anomaly, AnomalyFilter};
use kartenn_store_pg::list::Kind;
use kartenn_store_pg::read;
use serde::Serialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::cursor::{Listing, Page, PageParams, next_href, respond_page};
use crate::dated::{Dates, Moment};
use crate::objects::{Api, audited, bad, gate, href, instant, store};
use crate::problem::{Problem, ProblemKind};

/// The routes of the anomalies.
pub fn routes() -> OpenApiRouter<Api> {
    let router = OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(acknowledge));
    crate::openapi::presented(
        router,
        "/anomalies",
        &[],
        &["rule", "object", "message", "opened_at"],
    )
}

/// The filter of the list: rule, domain, entity.
type Wanted = (Option<String>, Option<Uuid>, Option<Uuid>);

/// `rule`, `domain` and `entity` among the parameters left by the page, each at most
/// once; anything else → 400.
fn filter(params: &[(String, String)]) -> Result<Wanted, Problem> {
    let (mut rule, mut domain, mut entity) = (None, None, None);
    for (name, value) in params {
        let slot = match name.as_str() {
            "rule" => &mut rule,
            "domain" => &mut domain,
            "entity" => &mut entity,
            other => return Err(bad(format!("unknown parameter {other}"))),
        };
        if slot.replace(value.clone()).is_some() {
            return Err(bad(format!("{name} is given more than once")));
        }
    }
    let id = |name: &str, value: Option<String>| {
        value
            .map(|v| v.parse::<Uuid>())
            .transpose()
            .map_err(|_| bad(format!("{name} must be a UUID")))
    };
    Ok((rule, id("domain", domain)?, id("entity", entity)?))
}

/// An open anomaly.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct AnomalyDto {
    /// Identifier.
    id: Uuid,
    /// The rule that raised it.
    rule: String,
    /// Its object.
    object: ObjectRef,
    /// The message of the rule.
    message: String,
    /// Its opening, RFC 3339.
    opened_at: String,
    /// Links.
    links: AnomalyLinks,
}

/// An object named by its kind and identifier.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ObjectRef {
    /// Its kind; `null` if unknown.
    kind: Option<&'static str>,
    /// Identifier.
    id: Uuid,
    /// Its URL; `null` if its kind is unknown.
    href: Option<String>,
}

impl ObjectRef {
    pub(crate) fn new(kind: Option<Kind>, id: Uuid) -> Self {
        Self {
            kind: kind.map(Kind::as_str),
            id,
            href: kind.map(|k| href(k, id)),
        }
    }
}

/// The links of an anomaly.
#[derive(Debug, Serialize, ToSchema)]
struct AnomalyLinks {
    /// Its acknowledgement.
    acknowledgement: String,
}

/// An anomaly as listed, with a link to its object of kind `kind`.
fn represent(anomaly: &Anomaly, kind: Option<Kind>) -> AnomalyDto {
    AnomalyDto {
        id: anomaly.id,
        rule: anomaly.rule_id.clone(),
        object: ObjectRef::new(kind, anomaly.object_id),
        message: anomaly.message.clone(),
        opened_at: rfc3339(instant(anomaly.opened_at)),
        links: AnomalyLinks {
            acknowledgement: format!("/v1/anomalies/{}/acknowledgement", anomaly.id),
        },
    }
}

/// The page of a list of anomalies `name` asks for (task 021-29): read at the current
/// state on each page, it takes neither `valid_at` nor `as_of`; its key, after a first
/// page.
pub(crate) fn anomaly_page(
    api: &Api,
    query: Option<&str>,
    name: &str,
) -> Result<(Page, Option<(OffsetDateTime, Uuid)>), Problem> {
    let page = Page::from_query(query.unwrap_or_default(), &api.ring, name)?;
    if page.after().is_none() && page.dates != Dates::default() {
        return Err(bad(format!("the {name} take neither valid_at nor as_of")));
    }
    let after = page
        .after()
        .map(|key| {
            let invalid = || Problem::new(ProblemKind::InvalidCursor);
            let (at, id) =
                serde_json::from_value::<(String, Uuid)>(key.clone()).map_err(|_| invalid())?;
            let at = OffsetDateTime::parse(&at, &Rfc3339).map_err(|_| invalid())?;
            Ok::<_, Problem>((at, id))
        })
        .transpose()?;
    Ok((page, after))
}

/// The link of the page after `found` in the list `name`, read at `moment`, when the
/// store gave more than a page: `found` is cut to the page.
pub(crate) fn anomaly_next(
    api: &Api,
    page: &Page,
    name: &str,
    moment: &Moment,
    found: &mut Vec<(Anomaly, Option<Kind>)>,
) -> Option<String> {
    let size = usize::from(page.size);
    (found.len() > size).then(|| {
        found.truncate(size);
        let last = &found[size - 1].0;
        let at = last
            .opened_at
            .format(&Rfc3339)
            .expect("a date of the database formats");
        let cursor = page.next_current(&api.ring, name, moment, json!([at, last.id]));
        next_href(&format!("/v1/{name}"), &cursor)
    })
}

/// `GET /v1/anomalies?rule=…&domain=…&entity=…`: the open anomalies whose object the caller
/// may read now, oldest first (REQ-002-36), by pages (task 021-29).
#[utoipa::path(
    get,
    path = "/anomalies",
    operation_id = "list_anomalies",
    tag = "core",
    params(
        ("rule" = Option<String>, Query, description = "The rule."),
        ("domain" = Option<Uuid>, Query, description = "The domain of the object."),
        ("entity" = Option<Uuid>, Query, description = "The entity of the object."),
        PageParams,
    ),
    responses((status = 200, description = "The open anomalies.", body = Listing<AnomalyDto>))
)]
async fn list(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    const NAME: &str = "anomalies";
    let (page, after) = anomaly_page(&api, query.as_deref(), NAME)?;
    let (rule, domain, entity) = filter(&page.filters)?;
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment = Dates::default().at(now, api.bound)?;
    let scope = api
        .authorizer()
        .read_scope(&principal.subject(), moment.now)
        .await?;
    let wanted = AnomalyFilter {
        rule: rule.as_deref(),
        domain,
        entity,
        property: None,
    };
    let limit = i64::from(page.size) + 1;
    let mut found = anomalies::open_page(&api.pool, &wanted, &scope, after, Some(limit))
        .await
        .map_err(|e| store(&e))?;
    let next = anomaly_next(&api, &page, NAME, &moment, &mut found);
    crate::properties::mask_gaps(&api, &context, &mut found).await?;
    let items: Vec<AnomalyDto> = found.iter().map(|(a, k)| represent(a, *k)).collect();
    let ids: Vec<Uuid> = found.iter().map(|(a, _)| a.object_id).collect();
    let criteria = json!({ "anomalies": { "rule": rule, "domain": domain, "entity": entity } });
    let event = objects_read(&context, "object", &ids, criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// The body of an action that needs a motive.
#[derive(Debug, ToSchema)]
pub(crate) struct Motive {
    /// Why, not blank.
    #[allow(dead_code)]
    motive: String,
}

/// The motive of a body that needs one (an acknowledgement, a rejection): a JSON object
/// with a non-blank `motive`. Another media type → 415, malformed JSON → 400, a missing or
/// blank motive → 422 with `missing` as detail.
pub(crate) fn motive(headers: &HeaderMap, body: &[u8], missing: &str) -> Result<String, Problem> {
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !body.is_empty() && !json {
        return Err(Problem::new(ProblemKind::UnsupportedMediaType));
    }
    let value: Value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(body).map_err(|_| bad("the body is not JSON"))?
    };
    match value.get("motive").and_then(Value::as_str).map(str::trim) {
        Some(motive) if !motive.is_empty() => Ok(motive.to_owned()),
        _ => Err(Problem::new(ProblemKind::MotiveRequired).detail(missing)),
    }
}

/// `POST /v1/anomalies/{id}/acknowledgement` with `{"motive": "…"}`: acknowledges the open
/// anomaly for the current version of its object (REQ-002-36); 204. An anomaly closed,
/// absent or of an object the caller may not read → 404; without
/// `core.anomaly.acknowledge` on its object → 403.
#[utoipa::path(
    post,
    path = "/anomalies/{id}/acknowledgement",
    operation_id = "acknowledge_anomaly",
    tag = "core",
    params(("id" = Uuid, Path, description = "The anomaly.")),
    request_body(content = Motive, content_type = "application/json"),
    responses((status = 204, description = "Acknowledged."))
)]
async fn acknowledge(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Problem> {
    let (anomaly, kind) = anomalies::find(&api.pool, id)
        .await
        .map_err(|e| store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    gate(&api, &context, kind, anomaly.object_id).await?;
    let permission = ANOMALY_ACKNOWLEDGE.name;
    let denied = api
        .authorizer()
        .authorize_many(&principal.subject(), permission, &[anomaly.object_id])
        .await?;
    if !denied.is_empty() {
        let event = context
            .event(&AUTHZ_DENIED, Outcome::Denied)
            .map_err(audited)?
            .target("anomaly", [id])
            .reason(permission)
            .detail("permission", permission)
            .map_err(audited)?;
        api.journal
            .record(&event)
            .await
            .map_err(RequestError::Audit)?;
        return Err(Problem::new(ProblemKind::Forbidden));
    }
    // The body is read once the anomaly is known visible: its errors are no oracle.
    let motive = motive(&headers, &body, "an acknowledgement needs a motive")?;
    let done = anomalies::acknowledge(&api.pool, id, principal.identity, &motive)
        .await
        .map_err(|e| store(&e))?;
    if !done {
        // Closed in between.
        return Err(Problem::new(ProblemKind::NotFound));
    }
    let event = context
        .event(&ANOMALY_ACKNOWLEDGED, Outcome::Success)
        .map_err(audited)?
        .target("anomaly", [id])
        .detail("rule", anomaly.rule_id.as_str())
        .and_then(|e| e.detail("object", anomaly.object_id.to_string()))
        .map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filter_takes_a_rule_a_domain_and_an_entity() {
        let pairs = |query: &str| -> Vec<(String, String)> {
            form_urlencoded::parse(query.as_bytes())
                .map(|(n, v)| (n.into_owned(), v.into_owned()))
                .collect()
        };
        let domain = Uuid::now_v7();
        let (rule, found, entity) =
            filter(&pairs(&format!("rule=core.x&domain={domain}"))).unwrap();
        assert_eq!(
            (rule.as_deref(), found, entity),
            (Some("core.x"), Some(domain), None)
        );
        assert!(filter(&pairs("domain=x")).is_err());
        assert!(filter(&pairs("rule=a&rule=b")).is_err());
        assert!(filter(&pairs("colour=red")).is_err());
    }

    #[test]
    fn the_motive_is_required_and_json() {
        let json = {
            let mut h = HeaderMap::new();
            h.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
            h
        };
        let kind = |r: Result<String, Problem>| r.unwrap_err().kind();
        let motive = |h: &HeaderMap, b: &[u8]| motive(h, b, "a motive");
        assert_eq!(motive(&json, br#"{"motive":" known "}"#).unwrap(), "known");
        assert_eq!(kind(motive(&json, b"{}")), ProblemKind::MotiveRequired);
        assert_eq!(
            kind(motive(&json, br#"{"motive":"  "}"#)),
            ProblemKind::MotiveRequired
        );
        assert_eq!(
            kind(motive(&HeaderMap::new(), b"")),
            ProblemKind::MotiveRequired
        );
        assert_eq!(kind(motive(&json, b"{")), ProblemKind::BadRequest);
        assert_eq!(
            kind(motive(&HeaderMap::new(), b"motive=x")),
            ProblemKind::UnsupportedMediaType
        );
    }
}
