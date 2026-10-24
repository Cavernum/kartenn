//! The simulation of any write (spec 024, REQ-024-17, M6): every write route that submits
//! a changeset takes `?simulate=true` and answers 200 with the simulation of
//! `POST /v1/changesets?simulate=true` (`ChangesetSimulated`), writing nothing; the
//! authentication reads the flag into the [`RequestContext`] and
//! [`crate::changesets::submit_answered`] honours it. A write that submits no changeset
//! (a decision on a changeset, an import, a login…) answers 400 `simulate-unsupported`
//! before its handler runs ([`gate`]).
//!
//! The contract says which is which: a write simulates when its 202 is a changeset
//! pending (`ChangesetPending`, `UndoPending`); its extension `x-kartenn` carries
//! `simulate: true|false`, and the gate of the router is drawn from it.
//!
//! [`RequestContext`]: kartenn_service::audit::RequestContext

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::extract::{MatchedPath, Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use utoipa::PartialSchema;
use utoipa::openapi::path::{Operation, ParameterBuilder, ParameterIn};
use utoipa::openapi::schema::OneOfBuilder;
use utoipa::openapi::{ContentBuilder, OpenApi, Ref, RefOr, Required, ResponseBuilder, Schema};

use crate::changesets::{SIMULATE, simulate_asked};
use crate::openapi::{CLIENT_EXTENSION, extend};
use crate::problem::{Problem, ProblemKind};

/// The schemas of a 202 that says a changeset is pending.
const PENDING: [&str; 2] = ["ChangesetPending", "UndoPending"];

/// The schema of a simulation.
const SIMULATED: &str = "ChangesetSimulated";

/// The prefix of the routes of the API, absent from the paths of a router not yet nested.
const V1: &str = "/v1";

fn schema_name(schema: Option<&RefOr<Schema>>) -> Option<&str> {
    match schema? {
        RefOr::Ref(r) => r.ref_location.rsplit('/').next(),
        RefOr::T(_) => None,
    }
}

/// Whether `operation` submits a changeset: its 202 is a changeset pending.
fn submits_changeset(operation: &Operation) -> bool {
    let Some(RefOr::T(accepted)) = operation.responses.responses.get("202") else {
        return false;
    };
    accepted.content.values().any(|content| match content {
        RefOr::T(content) => {
            schema_name(content.schema.as_ref()).is_some_and(|name| PENDING.contains(&name))
        }
        RefOr::Ref(_) => false,
    })
}

/// Declares the simulation on every write of `document`: `x-kartenn.simulate`, and, on a
/// write that submits a changeset, the parameter `simulate` and its 200
/// (`ChangesetSimulated`, beside the 200 of a write applied when it has one).
pub(crate) fn declare(document: &mut OpenApi) {
    for item in document.paths.paths.values_mut() {
        for operation in [
            &mut item.post,
            &mut item.put,
            &mut item.patch,
            &mut item.delete,
        ]
        .into_iter()
        .flatten()
        {
            let simulates = submits_changeset(operation);
            extend(&mut operation.extensions, json!({ "simulate": simulates }));
            if simulates {
                declared(operation);
            }
        }
    }
}

/// The parameter `simulate` and the 200 of a simulation, unless the operation declares
/// them itself (`POST /v1/changesets`, `…/undo`).
fn declared(operation: &mut Operation) {
    let parameters = operation.parameters.get_or_insert_default();
    if parameters
        .iter()
        .any(|p| matches!(p, RefOr::T(p) if p.name == SIMULATE))
    {
        return;
    }
    parameters.push(RefOr::T(
        ParameterBuilder::new()
            .name(SIMULATE)
            .parameter_in(ParameterIn::Query)
            .required(Required::False)
            .description(Some(
                "`true`: validate and report without writing (200 `ChangesetSimulated`, \
                 as `POST /v1/changesets?simulate=true`; spec 024, REQ-024-17).",
            ))
            .schema(Some(bool::schema()))
            .build(),
    ));
    let simulated = RefOr::Ref(Ref::from_schema_name(SIMULATED));
    let responses = &mut operation.responses.responses;
    match responses.get_mut("200") {
        Some(RefOr::T(ok)) => {
            ok.description
                .push_str(" Simulated with `simulate=true`: `ChangesetSimulated`.");
            for content in ok.content.values_mut() {
                if let RefOr::T(content) = content {
                    let applied = content.schema.take();
                    content.schema = Some(match applied {
                        Some(applied) => OneOfBuilder::new()
                            .item(applied)
                            .item(simulated.clone())
                            .into(),
                        None => simulated.clone(),
                    });
                }
            }
        }
        Some(RefOr::Ref(_)) => {}
        None => {
            responses.insert(
                "200".to_owned(),
                ResponseBuilder::new()
                    .description("Simulated (`simulate=true`): nothing is written.")
                    .content(
                        "application/json",
                        ContentBuilder::new().schema(Some(simulated)).build(),
                    )
                    .build()
                    .into(),
            );
        }
    }
}

/// The writes of a router that simulate, `(method, path)` with the path as the router
/// matches it under `/v1` or not, drawn from the extension `x-kartenn` of its contract.
#[derive(Debug, Clone, Default)]
pub struct Simulable(Arc<BTreeSet<(String, String)>>);

impl Simulable {
    /// The writes of `document` whose `x-kartenn.simulate` is `true`.
    ///
    /// # Panics
    ///
    /// Never: an OpenAPI document serialises.
    #[must_use]
    pub fn of(document: &OpenApi) -> Self {
        let value = serde_json::to_value(document).expect("an OpenAPI document serialises");
        let mut found = BTreeSet::new();
        for (path, item) in value["paths"].as_object().into_iter().flatten() {
            for method in ["post", "put", "patch", "delete"] {
                if item[method][CLIENT_EXTENSION]["simulate"] == Value::Bool(true) {
                    found.insert((method.to_ascii_uppercase(), unprefixed(path).to_owned()));
                }
            }
        }
        Self(Arc::new(found))
    }

    fn holds(&self, method: &Method, path: &str) -> bool {
        self.0
            .contains(&(method.as_str().to_owned(), unprefixed(path).to_owned()))
    }
}

fn unprefixed(path: &str) -> &str {
    path.strip_prefix(V1)
        .filter(|rest| rest.starts_with('/'))
        .unwrap_or(path)
}

/// Refuses `?simulate=true` on a write that submits no changeset (400
/// `simulate-unsupported`), before its handler runs: nothing is written. A layer of the
/// routes (`route_layer`), where the path matched is known.
pub async fn gate(State(simulable): State<Simulable>, request: Request, next: Next) -> Response {
    if request.method().is_safe() {
        return next.run(request).await;
    }
    match simulate_asked(request.uri().query()) {
        Err(problem) => return problem.into_response(),
        Ok(false) => {}
        Ok(true) => {
            let path = request.extensions().get::<MatchedPath>().map_or_else(
                || request.uri().path().to_owned(),
                |m| m.as_str().to_owned(),
            );
            if !simulable.holds(request.method(), &path) {
                return Problem::new(ProblemKind::SimulateUnsupported)
                    .detail(format!(
                        "{} {path} submits no changeset: it cannot be simulated",
                        request.method()
                    ))
                    .into_response();
            }
        }
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_known_under_v1_or_not() {
        assert_eq!(unprefixed("/v1/prefixes/{id}"), "/prefixes/{id}");
        assert_eq!(unprefixed("/prefixes"), "/prefixes");
        assert_eq!(unprefixed("/v1x"), "/v1x");
    }
}
