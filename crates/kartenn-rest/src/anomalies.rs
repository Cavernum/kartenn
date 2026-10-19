//! Anomalies (spec 006, REQ-006-4, 23; task 006-12c; 002/REQ-002-36): the open anomalies
//! of the objects the caller may read, filtered by rule, domain or entity; their
//! acknowledgement with a motive, which needs `core.anomaly.acknowledge` on the object of
//! the anomaly and is audited. The acknowledgement holds for the current version of the
//! object: no `If-Match`. The anomaly of an object the caller may not read answers 404, as
//! an absent one.

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Router};
use kartenn_core::audit::{ANOMALY_ACKNOWLEDGED, AUTHZ_DENIED, rfc3339};
use kartenn_core::authz::ANOMALY_ACKNOWLEDGE;
use kartenn_service::RequestError;
use kartenn_service::audit::{AuditJournal, Outcome, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::authz::Authorizer;
use kartenn_service::reads::objects_read;
use kartenn_store_pg::anomalies::{self, Anomaly, AnomalyFilter};
use kartenn_store_pg::list::Kind;
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::read;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::cursor::respond_page;
use crate::dated::Dates;
use crate::objects::{Api, audited, bad, gate, href, instant, store};
use crate::problem::{Problem, ProblemKind};

/// The routes of the anomalies.
pub fn routes(api: Api) -> Router {
    Router::new()
        .route("/anomalies", get(list))
        .route("/anomalies/{id}/acknowledgement", post(acknowledge))
        .with_state(api)
}

/// The filter of the list: rule, domain, entity.
type Wanted = (Option<String>, Option<Uuid>, Option<Uuid>);

/// `rule`, `domain` and `entity`, each at most once; anything else → 400.
fn filter(query: Option<&str>) -> Result<Wanted, Problem> {
    let (mut rule, mut domain, mut entity) = (None, None, None);
    for (name, value) in form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        let slot = match name.as_ref() {
            "rule" => &mut rule,
            "domain" => &mut domain,
            "entity" => &mut entity,
            other => return Err(bad(format!("unknown parameter {other}"))),
        };
        if slot.replace(value.into_owned()).is_some() {
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

/// An anomaly as listed, with a link to its object of kind `kind`.
fn represent(anomaly: &Anomaly, kind: Option<Kind>) -> Value {
    json!({
        "id": anomaly.id,
        "rule": anomaly.rule_id,
        "object": {
            "kind": kind.map(Kind::as_str),
            "id": anomaly.object_id,
            "href": kind.map(|k| href(k, anomaly.object_id)),
        },
        "message": anomaly.message,
        "opened_at": rfc3339(instant(anomaly.opened_at)),
        "links": { "acknowledgement": format!("/v1/anomalies/{}/acknowledgement", anomaly.id) },
    })
}

/// `GET /v1/anomalies?rule=…&domain=…&entity=…`: the open anomalies whose object the caller
/// may read now, oldest first (REQ-002-36).
async fn list(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let (rule, domain, entity) = filter(query.as_deref())?;
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment = Dates::default().at(now, api.bound)?;
    let scope = PgAuthorizer::new(api.pool.clone())
        .read_scope(&principal.subject(), moment.now)
        .await?;
    let wanted = AnomalyFilter {
        rule: rule.as_deref(),
        domain,
        entity,
    };
    let mut found = anomalies::open_within(&api.pool, &wanted, &scope)
        .await
        .map_err(|e| store(&e))?;
    crate::properties::mask_gaps(&api, &context, &mut found).await?;
    let items: Vec<Value> = found.iter().map(|(a, k)| represent(a, *k)).collect();
    let ids: Vec<Uuid> = found.iter().map(|(a, _)| a.object_id).collect();
    let criteria = json!({ "anomalies": { "rule": rule, "domain": domain, "entity": entity } });
    let event = objects_read(&context, "object", &ids, criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(respond_page(&headers, &moment, &items, None))
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
    let denied = PgAuthorizer::new(api.pool.clone())
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
        let domain = Uuid::now_v7();
        let (rule, found, entity) = filter(Some(&format!("rule=core.x&domain={domain}"))).unwrap();
        assert_eq!(
            (rule.as_deref(), found, entity),
            (Some("core.x"), Some(domain), None)
        );
        assert!(filter(Some("domain=x")).is_err());
        assert!(filter(Some("rule=a&rule=b")).is_err());
        assert!(filter(Some("valid_at=2020-01-01T00:00:00Z")).is_err());
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
