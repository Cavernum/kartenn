//! Properties (spec 006, REQ-006-4; task 006-13a; spec 003): the declarations and the
//! values of the closed lists, which any authenticated caller reads (a schema, not data);
//! for an object, its resolved values, the explanation of one of them and the value its
//! source expects; the deviations between resolved and expected values. All are dated by
//! `valid_at` and `as_of`, but the suggestion, computed on the current state. The value of
//! a sensitive property needs `prop.<key>.read`, or it is masked (REQ-005-21); an object
//! the explanation walks that the caller may not read is shown as `null` (REQ-005-19).

use std::collections::{BTreeMap, BTreeSet};

use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use kartenn_core::audit::rfc3339;
use kartenn_core::period::Period;
use kartenn_core::rules::PROP_EXPECTED_MISMATCH;
use kartenn_service::RequestError;
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::authz::Authorizer;
use kartenn_service::expected::suggest;
use kartenn_service::operations::RawInput;
use kartenn_service::reads::{objects_read, read_object, sensitive_disclosed};
use kartenn_store_pg::anomalies::{self, Anomaly, AnomalyFilter};
use kartenn_store_pg::explain::explain;
use kartenn_store_pg::list::Kind;
use kartenn_store_pg::property_read::{self, DeclarationRead};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::read;
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::cursor::{Page, next_href, respond_page};
use crate::dated::{Dates, ETag, Moment, respond};
use crate::objects::{Api, audited, bad, collection, gate, href, instant, store};
use crate::problem::{Problem, ProblemKind};

/// The routes of the properties.
pub fn routes(api: Api) -> Router {
    let mut router = Router::new()
        .route("/property-declarations", get(declarations))
        .route("/property-declarations/{key}/values", get(values))
        .route("/property-gaps", get(gaps));
    for kind in Kind::ALL {
        let name = collection(kind);
        router = router
            .route(
                &format!("/{name}/{{id}}/properties"),
                get(
                    move |state: State<Api>,
                          context: Extension<RequestContext>,
                          id: Path<Uuid>,
                          dates: Dates,
                          headers: HeaderMap| {
                        resolved(kind, state, context, id, dates, headers)
                    },
                ),
            )
            .route(
                &format!("/{name}/{{id}}/properties/{{key}}/explanation"),
                get(
                    move |state: State<Api>,
                          context: Extension<RequestContext>,
                          path: Path<(Uuid, String)>,
                          dates: Dates,
                          headers: HeaderMap| {
                        explanation(kind, state, context, path, dates, headers)
                    },
                ),
            )
            .route(
                &format!("/{name}/{{id}}/properties/{{key}}/suggestion"),
                get(
                    move |state: State<Api>,
                          context: Extension<RequestContext>,
                          path: Path<(Uuid, String)>,
                          dates: Dates,
                          headers: HeaderMap| {
                        suggestion(kind, state, context, path, dates, headers)
                    },
                ),
            );
    }
    router.with_state(api)
}

/// `body` as a dated read, with the weak tag of its content.
fn respond_json(headers: &HeaderMap, moment: &Moment, body: &Value) -> Response {
    let bytes = serde_json::to_vec(body).expect("JSON values serialise");
    let etag = ETag::page(&bytes);
    let response = ([(CONTENT_TYPE, "application/json")], bytes).into_response();
    respond(headers, moment, &etag, response)
}

async fn moment_of(api: &Api, dates: Dates) -> Result<Moment, Problem> {
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    dates.at(now, api.bound)
}

// ---------------------------------------------------------------- declarations

fn declaration_of(found: &DeclarationRead) -> Value {
    json!({
        "id": found.id,
        "key": found.key,
        "type": found.value_type,
        "params": found.params,
        "cardinality": found.cardinality,
        "carriers": found.carriers,
        "inherit": found.inherit,
        "sensitive": found.sensitive,
        "expected": found.expected.as_ref().map(|(source, mode)| {
            json!({ "source": source, "mode": mode })
        }),
        "links": { "values": format!("/v1/property-declarations/{}/values", found.key) },
    })
}

/// The key after which a page starts, as [`Page::next`] wrote it.
fn after_key(page: &Page) -> Result<Option<String>, Problem> {
    page.after()
        .map(|key| serde_json::from_value::<String>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))
}

/// `GET /v1/property-declarations`: the declarations at the dates of the read, by key.
async fn declarations(
    State(api): State<Api>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let name = "property-declarations";
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, name)?;
    page.unfiltered()?;
    let after = after_key(&page)?;
    let moment = moment_of(&api, page.dates).await?;
    let size = usize::from(page.size);
    let mut found = property_read::declarations(
        &api.pool,
        moment.valid_at,
        Some(moment.as_of()),
        after.as_deref(),
        i64::from(page.size) + 1,
    )
    .await
    .map_err(|e| store(&e))?;
    let next = (found.len() > size).then(|| {
        found.truncate(size);
        let cursor = page.next(&api.ring, name, &moment, json!(found[size - 1].key));
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let items: Vec<Value> = found.iter().map(declaration_of).collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// `GET /v1/property-declarations/{key}/values`: the keys of a closed list, sorted; 404
/// for a key not declared at the dates, 409 for a property of another type.
async fn values(
    State(api): State<Api>,
    Path(key): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let name = format!("property-declarations/{key}/values");
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, &name)?;
    page.unfiltered()?;
    let after = after_key(&page)?;
    let moment = moment_of(&api, page.dates).await?;
    let as_of = Some(moment.as_of());
    let declaration = property_read::declaration(&api.pool, &key, moment.valid_at, as_of)
        .await
        .map_err(|e| store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    if declaration.value_type != "enum" {
        return Err(Problem::new(ProblemKind::Conflict).detail(format!(
            "{key} is of type {}, not a closed list",
            declaration.value_type
        )));
    }
    let size = usize::from(page.size);
    let mut keys = property_read::enum_values(
        &api.pool,
        declaration.id,
        moment.valid_at,
        as_of,
        after.as_deref(),
        i64::from(page.size) + 1,
    )
    .await
    .map_err(|e| store(&e))?;
    let next = (keys.len() > size).then(|| {
        keys.truncate(size);
        let cursor = page.next(&api.ring, &name, &moment, json!(keys[size - 1]));
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let items: Vec<Value> = keys.into_iter().map(|key| json!({ "key": key })).collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

// ---------------------------------------------------------------- one object

/// `GET /v1/<collection>/{id}/properties`: the resolved values of the object, those the
/// caller may not see named in `masked`; 404 as the object.
async fn resolved(
    kind: Kind,
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let moment = moment_of(&api, dates).await?;
    let found = read_object(
        &PgStore::new(api.pool.clone()),
        &PgAuthorizer::new(api.pool.clone()),
        &api.journal,
        &context,
        id,
        instant(moment.valid_at),
        moment.as_of.map(instant),
        api.audit,
    )
    .await?
    .filter(|found| found.kind == kind.as_str())
    .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let this = format!("{}/properties", href(kind, id));
    let body = json!({
        "properties": found.properties,
        "masked": found.masked,
        "links": { "self": this, "object": href(kind, id) },
    });
    Ok(respond_json(&headers, &moment, &body))
}

/// 404 unless the object `id` of `kind` exists at the dates of `moment` and the caller
/// may read it now; then the declaration of `key` at those dates, 404 without one.
async fn object_and_declaration(
    api: &Api,
    context: &RequestContext,
    kind: Kind,
    id: Uuid,
    key: &str,
    moment: &Moment,
) -> Result<DeclarationRead, Problem> {
    let kinds = property_read::kinds(&api.pool, &[id], moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?;
    if kinds.get(&id) != Some(&kind) {
        return Err(Problem::new(ProblemKind::NotFound));
    }
    gate(api, context, kind, id).await?;
    property_read::declaration(&api.pool, key, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))
}

/// Whether the caller may see the value of `declaration` on `id`: always for a property
/// that is not sensitive; a disclosure of a sensitive one is audited.
async fn discloses(
    api: &Api,
    context: &RequestContext,
    declaration: &DeclarationRead,
    id: Uuid,
) -> Result<bool, Problem> {
    if !declaration.sensitive {
        return Ok(true);
    }
    let disclosed = sensitive_disclosed(
        &PgAuthorizer::new(api.pool.clone()),
        &api.journal,
        context,
        &declaration.key,
        &[id],
    )
    .await?;
    Ok(disclosed.contains(&id))
}

/// Records the read of the property `key` of `id` (`data.objects.read`).
async fn audit_read(
    api: &Api,
    context: &RequestContext,
    kind: Kind,
    id: Uuid,
    criteria: Value,
) -> Result<(), Problem> {
    let event =
        objects_read(context, kind.as_str(), &[id], criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(())
}

/// The objects among `ids` the caller may read now, as nodes `{kind, id, href}`; the
/// others, and those absent at the dates, are left out.
async fn visible_nodes(
    api: &Api,
    context: &RequestContext,
    ids: &[Uuid],
    moment: &Moment,
) -> Result<BTreeMap<Uuid, Value>, Problem> {
    let kinds = property_read::kinds(&api.pool, ids, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?;
    let mut by_kind: BTreeMap<&'static str, (Kind, Vec<Uuid>)> = BTreeMap::new();
    for (id, kind) in &kinds {
        by_kind
            .entry(kind.as_str())
            .or_insert_with(|| (*kind, Vec::new()))
            .1
            .push(*id);
    }
    let mut denied = BTreeSet::new();
    if let Some(subject) = context.subject() {
        let authorizer = PgAuthorizer::new(api.pool.clone());
        for (name, (_, ids)) in &by_kind {
            denied.extend(
                authorizer
                    .authorize_many(&subject, &format!("core.{name}.read"), ids)
                    .await?,
            );
        }
    }
    Ok(kinds
        .into_iter()
        .filter(|(id, _)| !denied.contains(id))
        .map(|(id, kind)| {
            let node = json!({ "kind": kind.as_str(), "id": id, "href": href(kind, id) });
            (id, node)
        })
        .collect())
}

/// A resolved value written as JSON by the store.
fn value_of(text: Option<&str>) -> Result<Value, Problem> {
    text.map(serde_json::from_str)
        .transpose()
        .map(Option::unwrap_or_default)
        .map_err(|_| Problem::new(ProblemKind::Internal))
}

/// `GET /v1/<collection>/{id}/properties/{key}/explanation`: how the value of `key`
/// resolves on the object at the dates of the read (REQ-003-14, 15).
async fn explanation(
    kind: Kind,
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path((id, key)): Path<(Uuid, String)>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let moment = moment_of(&api, dates).await?;
    let declaration = object_and_declaration(&api, &context, kind, id, &key, &moment).await?;
    let found = explain(&api.pool, id, declaration.id, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let mut walked = found.chain.clone();
    walked.extend(found.origin);
    walked.extend(found.replaces.as_ref().map(|(origin, _)| *origin));
    let nodes = visible_nodes(&api, &context, &walked, &moment).await?;
    let node = |id: &Uuid| nodes.get(id).cloned().unwrap_or(Value::Null);
    let criteria = json!({
        "object": id,
        "property": key,
        "explanation": true,
        "valid_at": rfc3339(instant(moment.valid_at)),
        "as_of": rfc3339(instant(moment.as_of())),
    });
    audit_read(&api, &context, kind, id, criteria).await?;
    let shown = discloses(&api, &context, &declaration, id).await?;
    let value = if shown {
        value_of(found.value.as_deref())?
    } else {
        Value::Null
    };
    let replaces = match &found.replaces {
        Some((origin, replaced)) => json!({
            "origin": node(origin),
            "value": if shown { value_of(replaced.as_deref())? } else { Value::Null },
        }),
        None => Value::Null,
    };
    let body = json!({
        "property": found.property,
        "value": value,
        "masked": !shown,
        "origin": found.origin.as_ref().map_or(Value::Null, node),
        "chain": found.chain.iter().map(node).collect::<Vec<_>>(),
        "provenance": found.provenance,
        "replaces": replaces,
        "links": { "object": href(kind, id) },
    });
    Ok(respond_json(&headers, &moment, &body))
}

fn raw_value(input: &RawInput) -> Value {
    match input {
        RawInput::Bool(b) => json!(b),
        RawInput::Int(i) => json!(i),
        RawInput::Text(t) => json!(t),
    }
}

/// `GET /v1/<collection>/{id}/properties/{key}/suggestion`: the value the source of the
/// property expects for the object at `valid_at`, written nowhere (REQ-003-23); computed
/// on the current state, so `as_of` is refused.
async fn suggestion(
    kind: Kind,
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path((id, key)): Path<(Uuid, String)>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    if dates.as_of.is_some() {
        return Err(bad("as_of: a suggestion is computed on the current state"));
    }
    let moment = moment_of(&api, dates).await?;
    let declaration = object_and_declaration(&api, &context, kind, id, &key, &moment).await?;
    let found = suggest(
        &PgStore::new(api.pool.clone()),
        &api.sources,
        declaration.id,
        id,
        Some(Period::from(instant(moment.valid_at))),
    )
    .await
    .map_err(Problem::from)?;
    let criteria = json!({
        "object": id,
        "property": key,
        "suggestion": true,
        "valid_at": rfc3339(instant(moment.valid_at)),
    });
    audit_read(&api, &context, kind, id, criteria).await?;
    let suggestion = match found {
        Some(found) if discloses(&api, &context, &declaration, id).await? => json!({
            "values": found.values.iter().map(raw_value).collect::<Vec<_>>(),
            "explanation": found.explanation,
        }),
        Some(_) => json!({ "masked": true }),
        None => Value::Null,
    };
    let body = json!({
        "property": declaration.key,
        "suggestion": suggestion,
        "links": { "object": href(kind, id) },
    });
    Ok(respond_json(&headers, &moment, &body))
}

// ---------------------------------------------------------------- deviations

/// The property a deviation is about: its message starts with its key.
fn gap_property(anomaly: &Anomaly) -> Option<&str> {
    (anomaly.rule_id == PROP_EXPECTED_MISMATCH.id)
        .then(|| anomaly.message.split_once(':').map(|(key, _)| key))
        .flatten()
}

/// Replaces the message of each deviation of a sensitive property that the caller may not
/// see on its object: the message cites the resolved value (REQ-005-21). Each message left
/// is a disclosure, audited.
pub(crate) async fn mask_gaps(
    api: &Api,
    context: &RequestContext,
    found: &mut [(Anomaly, Option<Kind>)],
) -> Result<(), Problem> {
    let mut sensitive: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    let keys = property_read::sensitive_keys(&api.pool)
        .await
        .map_err(|e| store(&e))?;
    for (anomaly, _) in found.iter() {
        if let Some(key) = gap_property(anomaly).filter(|k| keys.contains(*k)) {
            sensitive
                .entry(key.to_owned())
                .or_default()
                .push(anomaly.object_id);
        }
    }
    let authorizer = PgAuthorizer::new(api.pool.clone());
    for (key, objects) in sensitive {
        let disclosed =
            sensitive_disclosed(&authorizer, &api.journal, context, &key, &objects).await?;
        for (anomaly, _) in found.iter_mut() {
            if gap_property(anomaly) == Some(key.as_str())
                && !disclosed.contains(&anomaly.object_id)
            {
                anomaly.message = masked_gap(&key);
            }
        }
    }
    Ok(())
}

/// The message of a deviation of sensitive property `key`, without its values.
pub(crate) fn masked_gap(key: &str) -> String {
    format!("{key}: the resolved value differs from the expected one")
}

/// The filter of the deviations: property, domain, entity.
type GapFilter = (Option<String>, Option<Uuid>, Option<Uuid>);

/// `property`, `domain` and `entity`, each at most once; anything else → 400.
fn gap_filter(query: Option<&str>) -> Result<GapFilter, Problem> {
    let (mut property, mut domain, mut entity) = (None, None, None);
    for (name, value) in form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        let slot = match name.as_ref() {
            "property" => &mut property,
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
    Ok((property, id("domain", domain)?, id("entity", entity)?))
}

/// `GET /v1/property-gaps?property=…&domain=…&entity=…`: the open deviations between
/// resolved and expected values (REQ-003-24) whose object the caller may read now.
async fn gaps(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let (property, domain, entity) = gap_filter(query.as_deref())?;
    let moment = moment_of(&api, Dates::default()).await?;
    let scope = PgAuthorizer::new(api.pool.clone())
        .read_scope(&principal.subject(), moment.now)
        .await?;
    let wanted = AnomalyFilter {
        rule: Some(PROP_EXPECTED_MISMATCH.id),
        domain,
        entity,
    };
    let mut found = anomalies::open_within(&api.pool, &wanted, &scope)
        .await
        .map_err(|e| store(&e))?;
    if let Some(property) = &property {
        found.retain(|(a, _)| gap_property(a) == Some(property.as_str()));
    }
    let ids: Vec<Uuid> = found.iter().map(|(a, _)| a.object_id).collect();
    let criteria =
        json!({ "property_gaps": { "property": property, "domain": domain, "entity": entity } });
    let event = objects_read(&context, "object", &ids, criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    mask_gaps(&api, &context, &mut found).await?;
    let items: Vec<Value> = found
        .iter()
        .map(|(anomaly, kind)| {
            json!({
                "id": anomaly.id,
                "property": gap_property(anomaly),
                "object": {
                    "kind": kind.map(Kind::as_str),
                    "id": anomaly.object_id,
                    "href": kind.map(|k| href(k, anomaly.object_id)),
                },
                "message": anomaly.message,
                "opened_at": rfc3339(instant(anomaly.opened_at)),
            })
        })
        .collect();
    Ok(respond_page(&headers, &moment, &items, None))
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    fn anomaly(rule: &str, message: &str) -> Anomaly {
        Anomaly {
            id: Uuid::nil(),
            rule_id: rule.to_owned(),
            object_id: Uuid::nil(),
            message: message.to_owned(),
            opened_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn a_deviation_names_its_property() {
        let gap = anomaly(
            PROP_EXPECTED_MISMATCH.id,
            "dns.ttl: resolved 60, expected …",
        );
        assert_eq!(gap_property(&gap), Some("dns.ttl"));
        assert_eq!(gap_property(&anomaly("core.other", "dns.ttl: …")), None);
    }

    #[test]
    fn a_stored_value_reads_as_json() {
        assert_eq!(value_of(Some("[\"a\",\"b\"]")).unwrap(), json!(["a", "b"]));
        assert_eq!(value_of(None).unwrap(), Value::Null);
    }
}
