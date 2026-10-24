//! Filters of the lists (spec 006, REQ-006-9, 10; task 006-11): conditions in query
//! parameters, combined in conjunction, or in the JSON body of a `QUERY` (RFC 10008).
//!
//! A parameter names a field of the core (`domain`, `within`, `usage`…) or a
//! property `prop.<key>`, then its operator after `:` (a key never holds one): none for
//! equality (repeated: one of), `between` with bounds `low..high` (one may be open),
//! `exists` with `true` or `false` (design « Filtres »). The body of a `QUERY` is
//! written back as such parameters, so that both methods read the same list through
//! the same code; a cursor then holds them as it holds those of a `GET`.

use std::collections::BTreeMap;

use axum::extract::{FromRequestParts, Request, State};
use axum::handler::Handler;
use axum::http::request::Parts;
use axum::http::{HeaderName, HeaderValue, Method, header};
use axum::middleware::map_response;
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get};
use kartenn_store_pg::filter::Condition;
use serde::Deserialize;
use serde_json::Value;
use utoipa::ToSchema;

use crate::problem::{Problem, ProblemKind};

/// The `Accept-Query` header (RFC 10008 §3).
pub static ACCEPT_QUERY: HeaderName = HeaderName::from_static("accept-query");
/// The media types a `QUERY` takes, as a structured list (RFC 10008 §3).
const QUERY_TYPES: &str = "\"application/json\"";
/// The methods of a collection.
const ALLOW: &str = "GET, HEAD, QUERY";

/// What a filter applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Field {
    /// A field of the core, among those of the collection.
    Core(&'static str),
    /// A property, by its key.
    Property(String),
}

/// A condition on a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    /// The field.
    pub field: Field,
    /// The condition, that of the filter engine of spec 003.
    pub condition: Condition,
}

fn bad(detail: impl Into<String>) -> Problem {
    Problem::new(ProblemKind::BadRequest).detail(detail)
}

/// Whether `key` is a property key: `[a-z][a-z0-9_]*.[a-z][a-z0-9_]*` (REQ-003-2).
fn is_key(key: &str) -> bool {
    let word = |w: &str| {
        w.starts_with(|c: char| c.is_ascii_lowercase())
            && w.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    key.split_once('.').is_some_and(|(a, b)| word(a) && word(b))
}

/// The filters of the parameters `params` (those left by [`crate::cursor::Page`]) in a
/// collection whose fields of the core are `core`, filtered by property when
/// `properties`.
///
/// # Errors
///
/// 400 naming the parameter: unknown field or operator, a `between` or an `exists` given
/// twice, a malformed or empty value.
pub fn parse(
    params: &[(String, String)],
    core: &[&'static str],
    properties: bool,
) -> Result<Vec<Filter>, Problem> {
    let mut grouped: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (name, value) in params {
        grouped.entry(name).or_default().push(value);
    }
    grouped
        .into_iter()
        .map(|(name, values)| {
            let (field, op) = name.split_once(':').unwrap_or((name, ""));
            let field = match field.strip_prefix("prop.") {
                Some(_) if !properties => {
                    return Err(bad(format!(
                        "unknown parameter {name}: properties filter the prefixes, ranges \
                         and addresses of a domain"
                    )));
                }
                Some(key) if is_key(key) => Field::Property(key.to_owned()),
                Some(_) => return Err(bad(format!("{name}: malformed property key"))),
                None => match core.iter().find(|c| **c == field) {
                    Some(core) => Field::Core(core),
                    None => return Err(bad(format!("unknown parameter {name}"))),
                },
            };
            Ok(Filter {
                field,
                condition: condition(name, op, &values)?,
            })
        })
        .collect()
}

/// Most characters of a prefix `q`.
const MAX_PREFIX: usize = 100;

/// The prefix `q` of a suggestion (spec 018, REQ-018-22, 23; Q-018-8), taken out of
/// `params` (those left by [`crate::cursor::Page`], which a cursor keeps): given once, not
/// empty, 100 characters at most, lower-cased (the comparison ignores case).
///
/// # Errors
///
/// 400 `bad-request` naming `q`.
pub fn prefix(params: &mut Vec<(String, String)>) -> Result<Option<String>, Problem> {
    let mut found = None;
    let mut error = None;
    params.retain(|(name, value)| {
        if name != "q" {
            return true;
        }
        if found.is_some() {
            error = Some(bad("q is given more than once"));
        } else if value.is_empty() {
            error = Some(bad("q has an empty value"));
        } else if value.chars().count() > MAX_PREFIX {
            error = Some(bad(format!("q is longer than {MAX_PREFIX} characters")));
        }
        found = Some(value.to_lowercase());
        false
    });
    error.map_or(Ok(found), Err)
}

fn condition(name: &str, op: &str, values: &[&str]) -> Result<Condition, Problem> {
    if values.iter().any(|v| v.is_empty()) {
        return Err(bad(format!("{name} has an empty value")));
    }
    let once = || match values {
        [value] => Ok(*value),
        _ => Err(bad(format!("{name} is given more than once"))),
    };
    match op {
        "" => Ok(match values {
            [value] => Condition::Equals((*value).to_owned()),
            _ => Condition::OneOf(values.iter().map(|v| (*v).to_owned()).collect()),
        }),
        "between" => {
            let bound = |b: &str| (!b.is_empty()).then(|| b.to_owned());
            match once()?.split_once("..") {
                Some((low, high)) if !(low.is_empty() && high.is_empty()) => {
                    Ok(Condition::Between(bound(low), bound(high)))
                }
                _ => Err(bad(format!("{name} expects low..high, one bound at least"))),
            }
        }
        "exists" => match once()? {
            "true" => Ok(Condition::Present),
            "false" => Ok(Condition::Absent),
            _ => Err(bad(format!("{name} expects true or false"))),
        },
        _ => Err(bad(format!("{name}: unknown operator {op}"))),
    }
}

/// The body of a `QUERY` (design « Filtres »).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Search {
    /// The date of validity, RFC 3339.
    valid_at: Option<String>,
    /// The date of knowledge, RFC 3339.
    as_of: Option<String>,
    /// The conditions, in conjunction.
    #[serde(default)]
    filters: Vec<Wanted>,
    /// The size of a page.
    page_size: Option<u16>,
}

/// A condition on a field of the core or a property `prop.<key>`.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct Wanted {
    /// The field.
    field: String,
    /// The operator; equality by default.
    #[serde(default)]
    op: Op,
    /// A scalar or a list of scalars (`eq`), `[low, high]` (`between`), a boolean
    /// (`exists`).
    value: Value,
}

/// The operator of a condition.
#[derive(Debug, Default, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
enum Op {
    #[default]
    Eq,
    Between,
    Exists,
}

/// A scalar as written in a parameter.
fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The query string equivalent to the body of a `QUERY`.
fn search_query(body: &[u8]) -> Result<String, Problem> {
    let search: Search = serde_json::from_slice(body).map_err(|e| bad(e.to_string()))?;
    let mut query = form_urlencoded::Serializer::new(String::new());
    for (name, date) in [("valid_at", &search.valid_at), ("as_of", &search.as_of)] {
        if let Some(date) = date {
            query.append_pair(name, date);
        }
    }
    if let Some(size) = search.page_size {
        query.append_pair("page_size", &size.to_string());
    }
    for (n, wanted) in search.filters.iter().enumerate() {
        let wrong = |what: &str| bad(format!("/filters/{n}: {what}"));
        if wanted.field.contains(':') || matches!(wanted.field.as_str(), "" | "cursor") {
            return Err(wrong("field is not a field"));
        }
        let field = &wanted.field;
        match (wanted.op, &wanted.value) {
            (Op::Eq, Value::Array(values)) if !values.is_empty() => {
                for value in values {
                    let value = scalar(value).ok_or_else(|| wrong("value expects scalars"))?;
                    query.append_pair(field, &value);
                }
            }
            (Op::Eq, value) => {
                let value = scalar(value).ok_or_else(|| wrong("value expects a scalar"))?;
                query.append_pair(field, &value);
            }
            (Op::Between, Value::Array(bounds)) if bounds.len() == 2 => {
                let bound = |b: &Value| match b {
                    Value::Null => Some(String::new()),
                    b => scalar(b).filter(|b| !b.contains("..")),
                };
                let (Some(low), Some(high)) = (bound(&bounds[0]), bound(&bounds[1])) else {
                    return Err(wrong("bounds are scalars without .., or null"));
                };
                query.append_pair(&format!("{field}:between"), &format!("{low}..{high}"));
            }
            (Op::Exists, Value::Bool(b)) => {
                query.append_pair(&format!("{field}:exists"), &b.to_string());
            }
            (Op::Between, _) => return Err(wrong("value expects [low, high]")),
            (Op::Exists, _) => return Err(wrong("value expects true or false")),
        }
    }
    Ok(query.finish())
}

/// The parameters of a list: the query string of a `GET`, or the body of a `QUERY` written
/// as one after that of its URL (RFC 10008 §2 lets both carry the query).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListQuery(pub String);

impl<S: Send + Sync> FromRequestParts<S> for ListQuery {
    type Rejection = std::convert::Infallible;

    fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let query = parts
            .extensions
            .get::<Self>()
            .cloned()
            .unwrap_or_else(|| Self(parts.uri.query().unwrap_or_default().to_owned()));
        std::future::ready(Ok(query))
    }
}

/// The route of a collection read by `list`, which takes its parameters from
/// [`ListQuery`]: `GET` and `HEAD`, and `QUERY` with a JSON body; any other method → 405
/// with `Allow`. Every response announces `Accept-Query`.
pub fn collection<H, T, S>(list: H) -> MethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    // `MethodFilter` knows no `QUERY`: it is taken by the fallback, as other methods.
    get(list.clone())
        .fallback(move |State(state): State<S>, request: Request| query(list, state, request))
        .layer(map_response(announce))
}

async fn announce(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(ACCEPT_QUERY.clone(), HeaderValue::from_static(QUERY_TYPES));
    response
}

fn is_json(request: &Request) -> bool {
    request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
}

async fn query<H, T, S>(list: H, state: S, request: Request) -> Response
where
    H: Handler<T, S>,
    S: Clone + Send + Sync + 'static,
{
    if request.method() != Method::from_bytes(b"QUERY").expect("QUERY is a token") {
        return (
            [(header::ALLOW, HeaderValue::from_static(ALLOW))],
            Problem::new(ProblemKind::MethodNotAllowed).detail(format!("allowed: {ALLOW}")),
        )
            .into_response();
    }
    if !is_json(&request) {
        return Problem::new(ProblemKind::UnsupportedMediaType)
            .detail("a QUERY takes application/json")
            .into_response();
    }
    let (mut parts, body) = request.into_parts();
    // The size is bounded by layer 5.
    let Ok(body) = axum::body::to_bytes(body, usize::MAX).await else {
        return Problem::new(ProblemKind::BadRequest).into_response();
    };
    let search = match search_query(&body) {
        Ok(search) => search,
        Err(problem) => return problem.into_response(),
    };
    let query = match parts.uri.query().filter(|q| !q.is_empty()) {
        Some(url) if search.is_empty() => url.to_owned(),
        Some(url) => format!("{url}&{search}"),
        None => search,
    };
    parts.extensions.insert(ListQuery(query));
    list.call(Request::from_parts(parts, axum::body::Body::empty()), state)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE: &[&str] = &["domain", "within", "usage"];

    fn params(query: &str) -> Vec<(String, String)> {
        let mut params: Vec<_> = form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();
        params.sort();
        params
    }

    fn filters(query: &str) -> Result<Vec<Filter>, Problem> {
        parse(&params(query), CORE, true)
    }

    fn detail(query: &str) -> String {
        let problem = filters(query).unwrap_err();
        assert_eq!(problem.kind(), ProblemKind::BadRequest);
        let body: Value = serde_json::from_slice(&problem.body()).unwrap();
        body["detail"].as_str().unwrap().to_owned()
    }

    #[test]
    fn parameters_become_conditions() {
        let prop = |k: &str| Field::Property(k.to_owned());
        assert_eq!(
            filters(
                "domain=lab&prop.policy.env=prod&prop.policy.env=test\
                 &prop.dns.ttl:between=..3600&prop.dns.fqdn:exists=false&within=10.0.0.0/8"
            )
            .unwrap(),
            [
                Filter {
                    field: Field::Core("domain"),
                    condition: Condition::Equals("lab".into()),
                },
                Filter {
                    field: prop("dns.fqdn"),
                    condition: Condition::Absent,
                },
                Filter {
                    field: prop("dns.ttl"),
                    condition: Condition::Between(None, Some("3600".into())),
                },
                Filter {
                    field: prop("policy.env"),
                    condition: Condition::OneOf(vec!["prod".into(), "test".into()]),
                },
                Filter {
                    field: Field::Core("within"),
                    condition: Condition::Equals("10.0.0.0/8".into()),
                },
            ]
        );
        assert_eq!(
            filters("usage:between=a..&usage:exists=true")
                .unwrap()
                .len(),
            2,
            "two operators on a field are two conditions"
        );
    }

    #[test]
    fn a_wrong_parameter_is_named() {
        assert_eq!(detail("colour=red"), "unknown parameter colour");
        for (query, name) in [
            ("prop.env=prod", "prop.env"),
            ("prop.Dns.ttl=1", "prop.Dns.ttl"),
            ("prop.dns.ttl:above=1", "prop.dns.ttl:above"),
            ("prop.dns.ttl:between=..", "prop.dns.ttl:between"),
            ("prop.dns.ttl:between=60", "prop.dns.ttl:between"),
            ("usage:exists=yes", "usage:exists"),
            ("usage:exists=true&usage:exists=false", "usage:exists"),
            ("domain=", "domain"),
        ] {
            assert!(detail(query).starts_with(name), "{query}");
        }
    }

    #[test]
    fn a_body_reads_as_parameters() {
        let body = serde_json::json!({
            "valid_at": "2026-01-01T00:00:00Z",
            "page_size": 10,
            "filters": [
                { "field": "domain", "value": "lab" },
                { "field": "prop.policy.env", "op": "eq", "value": ["prod", "test"] },
                { "field": "prop.dns.ttl", "op": "between", "value": [60, null] },
                { "field": "prop.dns.fqdn", "op": "exists", "value": true },
            ],
        });
        let query = search_query(&serde_json::to_vec(&body).unwrap()).unwrap();
        assert_eq!(
            params(&query),
            params(
                "valid_at=2026-01-01T00:00:00Z&page_size=10&domain=lab&prop.policy.env=prod\
                 &prop.policy.env=test&prop.dns.ttl:between=60..&prop.dns.fqdn:exists=true"
            )
        );
        for wrong in [
            r#"{"filters": [{"field": "a:b", "value": 1}]}"#,
            r#"{"filters": [{"field": "a", "op": "between", "value": ["1..2", null]}]}"#,
            r#"{"filters": [{"field": "a", "op": "exists", "value": "true"}]}"#,
            r#"{"filters": [{"field": "a", "value": {}}]}"#,
            r#"{"filters": [{"field": "a", "op": "like", "value": 1}]}"#,
            r#"{"limit": 3}"#,
        ] {
            let problem = search_query(wrong.as_bytes()).unwrap_err();
            assert_eq!(problem.kind(), ProblemKind::BadRequest, "{wrong}");
        }
    }
}
