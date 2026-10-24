//! The catalogue of the rules of the build at `GET /v1/rules` (spec 018, REQ-018-23;
//! Q-018-8): the identifier, the severity and the module of each rule of the core, of the
//! sort of an import (006/H18) and of the modules of the build, for the filters and the
//! suggestions of the clients. Their labels come from the dictionary `GET /v1/labels`
//! (space `rule`). Codes of the software, read by any authenticated caller.

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::response::{IntoResponse, Response};
use kartenn_core::rules::{Rule, Severity, all_rules, import_rules};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::cursor::{Listing, ListingLinks};
use crate::objects::Api;
use crate::problem::{Problem, ProblemKind};

/// The module of the rules of the core.
const CORE: &str = "core";

/// A rule of the build.
#[derive(Debug, Serialize, ToSchema)]
struct RuleItem {
    /// Its identifier, stable.
    id: &'static str,
    /// `error` (refuses a changeset) or `warning` (records an anomaly).
    severity: &'static str,
    /// The module that declares it; `core` for the core.
    module: String,
}

impl RuleItem {
    fn of(rule: &Rule, module: &str) -> Self {
        Self {
            id: rule.id,
            severity: match rule.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            },
            module: module.to_owned(),
        }
    }
}

pub(crate) fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::new().routes(routes!(rules))
}

/// `GET /v1/rules`: every rule of the build, the core's in catalogue order, then those
/// of the sort of an import and of each module; `q` keeps those whose identifier starts
/// with it, ignoring case. One page: the catalogue is short.
#[utoipa::path(
    get,
    path = "/rules",
    operation_id = "list_rules",
    tag = "operations",
    params(("q" = Option<String>, Query, description = "Prefix of the identifier, ignoring case, 100 characters at most.")),
    responses((status = 200, description = "The rules of the build, in one page.", body = Listing<RuleItem>))
)]
async fn rules(State(api): State<Api>, RawQuery(query): RawQuery) -> Result<Response, Problem> {
    let mut params: Vec<(String, String)> =
        form_urlencoded::parse(query.as_deref().unwrap_or_default().as_bytes())
            .into_owned()
            .collect();
    let prefix = crate::filter::prefix(&mut params)?;
    if let Some((name, _)) = params.first() {
        return Err(
            Problem::new(ProblemKind::BadRequest).detail(format!("unknown parameter {name}"))
        );
    }
    let core = all_rules()
        .chain(import_rules())
        .map(|rule| RuleItem::of(rule, CORE));
    let modules = api.modules.modules().flat_map(|module| {
        let name = module.name();
        module
            .rules()
            .iter()
            .map(move |rule| RuleItem::of(rule, name))
    });
    let items: Vec<RuleItem> = core
        .chain(modules)
        .filter(|item| {
            prefix
                .as_ref()
                .is_none_or(|p| item.id.starts_with(p.as_str()))
        })
        .collect();
    let listing = Listing {
        items: &items,
        links: ListingLinks { next: None },
    };
    Ok(Json(&listing).into_response())
}
