//! Changesets (spec 006, REQ-006-11, 21, 22; task 006-14): `POST /v1/changesets`
//! submits an atomic batch of operations (004) with its motive and period, or simulates it
//! with `?simulate=true`. Applied → 201, pending → 202, simulated → 200; refused by the
//! rules → 422 with every violation, each with the index of its operation and a JSON
//! Pointer into the request. The response links to the journaled changeset.

use std::collections::BTreeSet;

use axum::body::Bytes;
use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json, Router};
use kartenn_core::rules::{PROP_EXPECTED_MISMATCH, Severity, Violation};
use kartenn_service::audit::RequestContext;
use kartenn_service::authn::Principal;
use kartenn_service::operations::PeriodSpec;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, submit_in};
use kartenn_store_pg::property_read;
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::read;
use kartenn_store_pg::write::PgStore;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::dated;
use crate::objects::{Api, bad, instant, store};
use crate::problem::{Problem, ProblemKind};
use crate::properties::masked_gap;

/// The relation type of the link to a changeset: an extension type, hence an URI
/// (RFC 8288 §2.1.2).
pub const REL_CHANGESET: &str = "urn:kartenn:rel:changeset";

/// The routes of the changesets.
pub fn routes(api: Api) -> Router {
    Router::new()
        .route("/changesets", post(submit))
        .with_state(api)
}

/// The path of a changeset.
#[must_use]
pub fn href(id: Uuid) -> String {
    format!("/v1/changesets/{id}")
}

/// The body of a submission (design « Écritures », arbitrage of 2026-10-19).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    #[serde(default)]
    motive: String,
    valid_from: Option<String>,
    valid_to: Option<String>,
    operations: Vec<Operation>,
}

/// Whether to simulate: `simulate=true|false`, at most once; anything else → 400.
fn simulate(query: Option<&str>) -> Result<bool, Problem> {
    let mut simulate = None;
    for (name, value) in form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        if name != "simulate" {
            return Err(bad(format!("unknown parameter {name}")));
        }
        let flag = match value.as_ref() {
            "true" => true,
            "false" => false,
            _ => return Err(bad("simulate must be true or false")),
        };
        if simulate.replace(flag).is_some() {
            return Err(bad("simulate is given more than once"));
        }
    }
    Ok(simulate.unwrap_or(false))
}

/// The body as a submission: JSON only (another media type → 415), a changeset or 400.
fn body(headers: &HeaderMap, body: &[u8]) -> Result<Body, Problem> {
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json {
        return Err(Problem::new(ProblemKind::UnsupportedMediaType));
    }
    serde_json::from_slice(body).map_err(|e| bad(format!("the body is not a changeset: {e}")))
}

/// The period of the changeset: `valid_to` absent or `null` is unbounded, `valid_from`
/// absent is the time of the database (REQ-004-3); both absent, the service takes the
/// time of its transaction.
async fn period(api: &Api, body: &Body) -> Result<Option<PeriodSpec>, Problem> {
    let date = |name: &str, text: &Option<String>| {
        text.as_deref()
            .map(|t| dated::instant(name, t).map(|at| instant(at).0))
            .transpose()
    };
    let (from, to) = (
        date("valid_from", &body.valid_from)?,
        date("valid_to", &body.valid_to)?,
    );
    let start = match (from, to) {
        (None, None) => return Ok(None),
        (Some(start), _) => start,
        (None, Some(_)) => instant(read::now(&api.pool).await.map_err(|e| store(&e))?).0,
    };
    Ok(Some(PeriodSpec { start, end: to }))
}

/// A violation as served: its operation and a pointer to it in the request
/// (RFC 6901), `null` for a rule of the final state (REQ-004-7).
fn violation(v: &Violation) -> Value {
    json!({
        "rule": v.rule.id,
        "severity": match v.rule.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        },
        "detail": v.message,
        "subjects": v.subjects,
        "operation": v.operation,
        "pointer": v.operation.map(|i| format!("/operations/{i}")),
    })
}

/// The violations as served, the message of a deviation of a sensitive property replaced:
/// it cites the resolved value (REQ-005-21, design « Erreurs »).
async fn violations(api: &Api, found: &[Violation]) -> Result<Value, Problem> {
    let sensitive = if found.iter().any(|v| v.rule.id == PROP_EXPECTED_MISMATCH.id) {
        property_read::sensitive_keys(&api.pool)
            .await
            .map_err(|e| store(&e))?
    } else {
        BTreeSet::new()
    };
    Ok(Value::Array(
        found
            .iter()
            .map(|v| {
                let mut served = violation(v);
                if v.rule.id == PROP_EXPECTED_MISMATCH.id
                    && let Some((key, _)) = v.message.split_once(':')
                    && sensitive.contains(key)
                {
                    served["detail"] = Value::String(masked_gap(key));
                }
                served
            })
            .collect(),
    ))
}

fn summary(id: Uuid) -> Value {
    json!({ "id": id, "href": href(id) })
}

/// `Link` to changeset `id`, and `Location` too for a created one.
fn linked(mut response: Response, id: Uuid, created: bool) -> Response {
    let path = href(id);
    if let Ok(link) = HeaderValue::try_from(format!("<{path}>; rel=\"{REL_CHANGESET}\"")) {
        response.headers_mut().insert(header::LINK, link);
    }
    if created && let Ok(location) = HeaderValue::try_from(path) {
        response.headers_mut().insert(header::LOCATION, location);
    }
    response
}

fn cascaded(closures: &[(kartenn_service::ObjectKind, Uuid)]) -> Value {
    closures
        .iter()
        .map(|(kind, id)| json!({ "kind": kind, "id": id }))
        .collect()
}

/// `POST /v1/changesets[?simulate=true]` with `{motive, valid_from, valid_to,
/// operations}`. The problems come in the order of HTTP (REQ-006-22): the authentication
/// (layer 6) before the syntax, the syntax (400, 415) before the rules (422).
async fn submit(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let simulate = simulate(query.as_deref())?;
    let body = body(&headers, &raw)?;
    let request = ChangesetRequest {
        author: principal.identity,
        motive: body.motive.clone(),
        period: period(&api, &body).await?,
        operations: body.operations,
        simulate,
        idempotency_key: None,
    };
    let store = PgStore::new(api.pool.clone()).with_instance(api.journal.instance().clone());
    let outcome = submit_in(
        &store,
        &PgAuthorizer::new(api.pool.clone()),
        &api.sources,
        &context,
        &request,
        MAX_OPERATIONS,
    )
    .await?;
    let response = match outcome {
        Outcome::Applied {
            changeset,
            warnings,
            diff,
            cascaded: closures,
        } => linked(
            (
                StatusCode::CREATED,
                Json(json!({
                    "changeset": summary(changeset),
                    "state": "applied",
                    "warnings": violations(&api, &warnings).await?,
                    "diff": diff,
                    "cascaded": cascaded(&closures),
                })),
            )
                .into_response(),
            changeset,
            true,
        ),
        Outcome::Pending {
            changeset,
            kind,
            policy,
            diff,
        } => linked(
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "changeset": summary(changeset),
                    "state": "pending",
                    "kind": kind.as_str(),
                    "policy": policy,
                    "diff": diff,
                })),
            )
                .into_response(),
            changeset,
            false,
        ),
        Outcome::Refused {
            changeset,
            violations: found,
        } => linked(
            Problem::new(ProblemKind::ChangesetRefused)
                .detail(format!(
                    "the changeset is refused by {} violation(s)",
                    found.iter().filter(|v| v.is_error()).count()
                ))
                .extension("violations", violations(&api, &found).await?)
                .extension("changeset", summary(changeset))
                .into_response(),
            changeset,
            false,
        ),
        // Nothing journaled: no changeset to link to (REQ-004-9).
        Outcome::Simulated {
            violations: found,
            diff,
            sample,
            cascaded: closures,
        } => Json(json!({
            "valid": !found.iter().any(Violation::is_error),
            "violations": violations(&api, &found).await?,
            "diff": diff,
            "sample": sample,
            "cascaded": cascaded(&closures),
        }))
        .into_response(),
        // A replay needs an idempotency key (006-15), a failed revalidation a pending
        // changeset being validated (006-17): neither comes from this route yet.
        other => {
            tracing::error!(name: "http.changeset.unexpected", outcome = ?other, "a submission had an unexpected outcome");
            Problem::new(ProblemKind::Internal).into_response()
        }
    };
    Ok(response)
}

#[cfg(test)]
mod tests {
    use kartenn_core::rules;

    use super::*;

    #[test]
    fn simulate_is_a_flag_given_once() {
        assert!(!simulate(None).unwrap());
        assert!(simulate(Some("simulate=true")).unwrap());
        for wrong in ["simulate=yes", "simulate=true&simulate=false", "dry=true"] {
            assert_eq!(
                simulate(Some(wrong)).unwrap_err().kind(),
                ProblemKind::BadRequest,
                "{wrong}"
            );
        }
    }

    #[test]
    fn the_body_is_a_json_changeset() {
        let mut json = HeaderMap::new();
        json.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
        let mut patch = HeaderMap::new();
        patch.insert(
            header::CONTENT_TYPE,
            "application/merge-patch+json".parse().unwrap(),
        );
        let ok = br#"{"motive": "m", "operations": [{"op": "create_domain", "local": 1, "slug": "lab"}]}"#;
        assert_eq!(body(&json, ok).unwrap().operations.len(), 1);
        assert_eq!(
            body(&patch, ok).unwrap_err().kind(),
            ProblemKind::UnsupportedMediaType
        );
        for wrong in [
            &b"{"[..],
            br#"{"motive": "m"}"#,
            br#"{"operations": [], "x": 1}"#,
        ] {
            assert_eq!(
                body(&json, wrong).unwrap_err().kind(),
                ProblemKind::BadRequest
            );
        }
    }

    #[test]
    fn a_violation_points_to_its_operation() {
        let v = Violation::new(&rules::PREFIX_DUPLICATE, "dup").at(2);
        let served = violation(&v);
        assert_eq!(served["operation"], 2);
        assert_eq!(served["pointer"], "/operations/2");
        assert_eq!(served["severity"], "error");
        let final_state = violation(&Violation::new(&rules::PREFIX_DUPLICATE, "dup"));
        assert_eq!(final_state["pointer"], Value::Null);
    }
}
