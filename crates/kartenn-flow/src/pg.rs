//! The storage of the module in PostgreSQL, in the schema `flow` (design 011 « Données »).

use kartenn_core::period::{Instant, Period};
use kartenn_core::values::ExternalRef;
use kartenn_policy::evaluate::{Decisive, Outcome, SynthesisVerdict, Verdict};
use kartenn_service::port::{StoreError, WriteError};
use serde_json::{Value, json};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use std::collections::{BTreeMap, BTreeSet};

use crate::evaluation::Evaluated;
use crate::model::{Endpoint, EndpointKind, Flow, FlowKind, OnBehalfOf};
use crate::reevaluation::{Row, Segment, State, Summary};

/// The versioned table of the flows.
pub(crate) const FLOW_TABLE: &str = "flow.flow";

/// The versioned tables keyed by `flow_id`, ended with their flow.
const BY_FLOW: &[&str] = &[
    "flow.flow_waiver",
    "flow.flow_endpoint_address",
    "flow.flow_evaluation",
    "flow.flow_summary",
];

/// The derived tables of the evaluation of a flow, rewritten by a new evaluation.
const DERIVED: &[&str] = &[
    "flow.flow_endpoint_address",
    "flow.flow_evaluation",
    "flow.flow_summary",
];

/// An instant as PostgreSQL reads it.
pub(crate) fn time(i: Instant) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).expect("an instant")
}

/// An instant of PostgreSQL as the core counts it, in microseconds.
pub(crate) fn instant(t: OffsetDateTime) -> Instant {
    kartenn_machines::read::instant(t)
}

/// The bounds of `period` to bind; a `None` start is the instant of the transaction.
pub(crate) fn bounds(period: Option<Period>) -> (Option<OffsetDateTime>, Option<OffsetDateTime>) {
    period.map_or((None, None), |p| (Some(time(p.start())), p.end().map(time)))
}

pub(crate) fn store(e: &sqlx::Error) -> StoreError {
    StoreError(e.to_string())
}

/// The name an endpoint kind is stored under.
pub(crate) fn kind_name(kind: EndpointKind) -> &'static str {
    match kind {
        EndpointKind::Address => "address",
        EndpointKind::Prefix => "prefix",
        EndpointKind::Interface => "interface",
        EndpointKind::Machine => "machine",
        EndpointKind::Range => "range",
    }
}

/// The endpoint kind of a stored or requested name.
pub(crate) fn kind_of(name: &str) -> Option<EndpointKind> {
    Some(match name {
        "address" => EndpointKind::Address,
        "prefix" => EndpointKind::Prefix,
        "interface" => EndpointKind::Interface,
        "machine" => EndpointKind::Machine,
        "range" => EndpointKind::Range,
        _ => return None,
    })
}

/// The qualified type of the object an endpoint designates, as the core names it.
pub(crate) fn object_kind(kind: EndpointKind) -> &'static str {
    match kind {
        EndpointKind::Address => "address",
        EndpointKind::Prefix => "prefix",
        EndpointKind::Interface => "machines.interface",
        EndpointKind::Machine => "machines.machine",
        EndpointKind::Range => "range",
    }
}

/// The table an endpoint kind lives in.
fn table_of(kind: EndpointKind) -> &'static str {
    match kind {
        EndpointKind::Address => "public.address",
        EndpointKind::Prefix => "public.prefix",
        EndpointKind::Interface => "machines.interface",
        EndpointKind::Machine => "machines.machine",
        EndpointKind::Range => "public.range",
    }
}

/// The kind of endpoint a reference of the module points at (`address`,
/// `machines.interface`…), as the flow records it; `None` for another target.
fn endpoint_kind(to: &str) -> Option<&'static str> {
    Some(match to {
        "address" => "address",
        "prefix" => "prefix",
        "machines.interface" => "interface",
        "machines.machine" => "machine",
        _ => return None,
    })
}

/// The identifier of the property `key` valid at `at` (now when `None`).
pub(crate) async fn property(
    conn: &mut PgConnection,
    key: &str,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT id FROM public.property_declaration
          WHERE key = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(key)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// `text` read as an instant of PostgreSQL, `None` if it is not one.
pub(crate) async fn parse_instant(
    conn: &mut PgConnection,
    text: &str,
) -> Result<Option<Instant>, StoreError> {
    // A failed cast must not abort the transaction: tested in a savepoint.
    sqlx::query("SAVEPOINT flow_instant")
        .execute(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    let parsed: Result<OffsetDateTime, sqlx::Error> = sqlx::query_scalar("SELECT $1::timestamptz")
        .bind(text)
        .fetch_one(&mut *conn)
        .await;
    let release = if parsed.is_ok() {
        "RELEASE SAVEPOINT flow_instant"
    } else {
        "ROLLBACK TO SAVEPOINT flow_instant"
    };
    sqlx::query(release)
        .execute(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    Ok(parsed.ok().map(instant))
}

/// The flows that name `target`, of the type `to`, as an endpoint and live from `at` on
/// (active or to come, REQ-011-3).
pub(crate) async fn referrers(
    conn: &mut PgConnection,
    to: &str,
    target: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Vec<Uuid>, StoreError> {
    let Some(kind) = endpoint_kind(to) else {
        return Ok(Vec::new());
    };
    sqlx::query_scalar(
        "SELECT DISTINCT id FROM flow.flow
          WHERE ((source_kind = $3 AND source_id = $1)
              OR (destination_kind = $3 AND destination_id = $1))
            AND valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)
          ORDER BY id",
    )
    .bind(target)
    .bind(at)
    .bind(kind)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// Ends from `at` on the rows of the flow `flow` in the tables keyed by `flow_id` (its
/// waiver, its endpoints and evaluations), as the core ends a versioned object: a version
/// that started before is shortened, a later one removed.
pub(crate) async fn close_dependents(
    conn: &mut PgConnection,
    flow: Uuid,
    at: Option<OffsetDateTime>,
    changeset: Uuid,
) -> Result<(), StoreError> {
    end_rows(conn, BY_FLOW, flow, at, changeset).await
}

async fn end_rows(
    conn: &mut PgConnection,
    tables: &[&str],
    flow: Uuid,
    at: Option<OffsetDateTime>,
    changeset: Uuid,
) -> Result<(), StoreError> {
    for table in tables {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE {table}
                SET valid_period = tstzrange(lower(valid_period), coalesce($2::timestamptz, now())),
                    changeset_id = $3
              WHERE flow_id = $1 AND valid_period @> coalesce($2::timestamptz, now())
                AND lower(valid_period) < coalesce($2::timestamptz, now())"
        )))
        .bind(flow)
        .bind(at)
        .bind(changeset)
        .execute(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {table}
              WHERE flow_id = $1 AND lower(valid_period) >= coalesce($2::timestamptz, now())"
        )))
        .bind(flow)
        .bind(at)
        .execute(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    }
    Ok(())
}

fn written(e: &sqlx::Error, id: Uuid) -> WriteError {
    match e {
        sqlx::Error::Database(d) if d.code().as_deref() == Some("23P01") => {
            WriteError::NoVersion(id)
        }
        _ => WriteError::Store(store(e)),
    }
}

/// Inserts the flow `id` as `record`, over [`from`, `to`) (from now when `from` is `None`).
pub(crate) async fn insert_flow(
    conn: &mut PgConnection,
    id: Uuid,
    record: &Value,
    (from, to): (Option<OffsetDateTime>, Option<OffsetDateTime>),
    changeset: Uuid,
) -> Result<(), WriteError> {
    sqlx::query(
        "INSERT INTO flow.flow (id, kind, source_kind, source_id, destination_kind,
                                destination_id, service_id, external_refs, justification,
                                requester_id, on_behalf_of, responsibles, data_nature,
                                personal_data, entity_id, valid_period, changeset_id)
         SELECT $1, r.kind, r.source_kind, r.source_id, r.destination_kind, r.destination_id,
                r.service, r.external_refs, r.justification, r.requester, r.on_behalf_of,
                coalesce(r.responsibles, '[]'), coalesce(r.data_nature, '{}'),
                coalesce(r.personal_data, false), r.entity,
                tstzrange(coalesce($3::timestamptz, now()), $4), $5
           FROM jsonb_to_record($2) AS r(kind text, source_kind text, source_id uuid,
                destination_kind text, destination_id uuid, service uuid, external_refs jsonb,
                justification text, requester uuid, on_behalf_of jsonb, responsibles jsonb,
                data_nature uuid[], personal_data boolean, entity uuid)",
    )
    .bind(id)
    .bind(record)
    .bind(from)
    .bind(to)
    .bind(changeset)
    .execute(conn)
    .await
    .map_err(|e| written(&e, id))?;
    Ok(())
}

/// Rewrites the fields `patch` of the flow `id` over [`from`, `to`).
pub(crate) async fn patch_flow(
    conn: &mut PgConnection,
    id: Uuid,
    patch: &Value,
    (from, to): (Option<OffsetDateTime>, Option<OffsetDateTime>),
    changeset: Uuid,
) -> Result<(), WriteError> {
    let versions: i32 = sqlx::query_scalar(
        "SELECT public.kartenn_portion('flow.flow'::regclass, $1,
                                       coalesce($2::timestamptz, now()), $3, $4::jsonb, $5)",
    )
    .bind(id)
    .bind(from)
    .bind(to)
    .bind(patch)
    .bind(changeset)
    .fetch_one(conn)
    .await
    .map_err(|e| written(&e, id))?;
    if versions == 0 {
        return Err(WriteError::NoVersion(id));
    }
    Ok(())
}

/// Marks `flow` as written by the operation `op` of the changeset of this transaction, for
/// the rules on the final state (design 011 « Opérations et règles »).
pub(crate) async fn mark(conn: &mut PgConnection, flow: Uuid, op: &str) -> Result<(), StoreError> {
    sqlx::query(
        "CREATE TEMP TABLE IF NOT EXISTS flow_written (flow_id uuid NOT NULL, op text NOT NULL)
         ON COMMIT DROP",
    )
    .execute(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    sqlx::query("INSERT INTO pg_temp.flow_written VALUES ($1, $2)")
        .bind(flow)
        .bind(op)
        .execute(conn)
        .await
        .map_err(|e| store(&e))?;
    Ok(())
}

/// The operations of this transaction that wrote `flow`, in their order.
pub(crate) async fn marks(conn: &mut PgConnection, flow: Uuid) -> Result<Vec<String>, StoreError> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('pg_temp.flow_written') IS NOT NULL")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    if !exists {
        return Ok(Vec::new());
    }
    sqlx::query_scalar("SELECT op FROM pg_temp.flow_written WHERE flow_id = $1")
        .bind(flow)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))
}

/// A flow as stored: its last version, with its reviewer.
#[derive(Debug, Clone)]
pub(crate) struct Stored {
    pub flow: Flow,
    pub reviewed_by: Option<Uuid>,
}

fn external_refs(value: &Value) -> Vec<ExternalRef> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some(ExternalRef {
                system: r.get("system")?.as_str()?.parse().ok()?,
                id: r.get("id")?.as_str()?.to_owned(),
                label: r.get("label").and_then(Value::as_str).map(str::to_owned),
            })
        })
        .collect()
}

fn on_behalf_of(value: Option<&Value>) -> Option<OnBehalfOf> {
    let value = value?;
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return Some(OnBehalfOf::Text(text.to_owned()));
    }
    let r = value.get("external")?;
    external_refs(&Value::Array(vec![r.clone()]))
        .pop()
        .map(OnBehalfOf::External)
}

/// The last version of the flow `id` in the transaction.
pub(crate) async fn load(conn: &mut PgConnection, id: Uuid) -> Result<Option<Stored>, StoreError> {
    let row: Option<(Value, OffsetDateTime, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT to_jsonb(f) - 'valid_period', lower(valid_period), upper(valid_period)
           FROM flow.flow f WHERE id = $1 ORDER BY lower(valid_period) DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))?;
    let Some((r, from, to)) = row else {
        return Ok(None);
    };
    let Ok(period) = Period::new(instant(from), to.map(instant)) else {
        return Ok(None);
    };
    let text = |key: &str| r.get(key).and_then(Value::as_str).unwrap_or_default();
    let uuid = |key: &str| {
        r.get(key)
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<Uuid>().ok())
    };
    let uuids = |key: &str| -> Vec<Uuid> {
        r.get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str()?.parse().ok())
            .collect()
    };
    let endpoint = |kind: &str, id: &str| Endpoint {
        kind: kind_of(text(kind)).unwrap_or(EndpointKind::Range),
        id: uuid(id).unwrap_or_default(),
    };
    let flow = Flow {
        kind: if text("kind") == "observed" {
            FlowKind::Observed
        } else {
            FlowKind::Requested
        },
        source: endpoint("source_kind", "source_id"),
        destination: endpoint("destination_kind", "destination_id"),
        service: uuid("service_id").unwrap_or_default(),
        external_refs: external_refs(r.get("external_refs").unwrap_or(&Value::Null)),
        justification: r
            .get("justification")
            .and_then(Value::as_str)
            .map(str::to_owned),
        requester: uuid("requester_id").unwrap_or_default(),
        on_behalf_of: on_behalf_of(r.get("on_behalf_of").filter(|v| !v.is_null())),
        responsibles: uuids("responsibles"),
        data_nature: uuids("data_nature"),
        personal_data: r
            .get("personal_data")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        entity: uuid("entity_id").unwrap_or_default(),
        period,
    };
    Ok(Some(Stored {
        flow,
        reviewed_by: uuid("reviewed_by"),
    }))
}

/// Whether the object `id` of `kind` exists over the whole `period`.
pub(crate) async fn exists_over(
    conn: &mut PgConnection,
    kind: EndpointKind,
    id: Uuid,
    period: Period,
) -> Result<bool, StoreError> {
    covered(conn, table_of(kind), id, period).await
}

/// Whether the service `id` is active over the whole `period`.
pub(crate) async fn service_over(
    conn: &mut PgConnection,
    id: Uuid,
    period: Period,
) -> Result<bool, StoreError> {
    covered(conn, "policy.policy_service", id, period).await
}

async fn covered(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    period: Period,
) -> Result<bool, StoreError> {
    let (from, to) = bounds(Some(period));
    // A schema absent from the build (no module Machines): never covered.
    let present: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(table)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    if !present {
        return Ok(false);
    }
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT coalesce(range_agg(valid_period) @> tstzrange($2, $3), false)
           FROM {table} WHERE id = $1"
    )))
    .bind(id)
    .bind(from)
    .bind(to)
    .fetch_one(conn)
    .await
    .map_err(|e| store(&e))
}

/// The waiver of `flow` that covers `at`: reason, accepted risk, grantor, start and end.
pub(crate) async fn waiver_at(
    conn: &mut PgConnection,
    flow: Uuid,
    at: OffsetDateTime,
) -> Result<Option<(String, String, Uuid, OffsetDateTime, Option<OffsetDateTime>)>, StoreError> {
    sqlx::query_as(
        "SELECT reason, accepted_risk, granted_by, lower(valid_period), upper(valid_period)
           FROM flow.flow_waiver WHERE flow_id = $1 AND valid_period @> $2",
    )
    .bind(flow)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// The last waiver of `flow` written by this changeset: reason, accepted risk, grantor,
/// start and end.
pub(crate) async fn waiver_written(
    conn: &mut PgConnection,
    flow: Uuid,
    changeset: Uuid,
) -> Result<Option<(String, String, Uuid, OffsetDateTime, Option<OffsetDateTime>)>, StoreError> {
    sqlx::query_as(
        "SELECT reason, accepted_risk, granted_by, lower(valid_period), upper(valid_period)
           FROM flow.flow_waiver WHERE flow_id = $1 AND changeset_id = $2
          ORDER BY lower(valid_period) DESC LIMIT 1",
    )
    .bind(flow)
    .bind(changeset)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// Writes a waiver of `flow` over [`from`, `end`).
pub(crate) async fn insert_waiver(
    conn: &mut PgConnection,
    flow: Uuid,
    data: &Value,
    from: Option<OffsetDateTime>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    // A waiver replaces, from its start, the waiver in force (a recertification).
    end_rows(conn, &["flow.flow_waiver"], flow, from, changeset).await?;
    sqlx::query(
        "INSERT INTO flow.flow_waiver (flow_id, reason, accepted_risk, granted_by,
                                       valid_period, changeset_id)
         VALUES ($1, $2, $3, $4,
                 tstzrange(coalesce($5::timestamptz, now()), to_timestamp($6::double precision / 1000000)),
                 $7)",
    )
    .bind(flow)
    .bind(data["reason"].as_str().unwrap_or_default())
    .bind(data["accepted_risk"].as_str().unwrap_or_default())
    .bind(data["granted_by"].as_str().and_then(|s| s.parse::<Uuid>().ok()))
    .bind(from)
    .bind(data["end"].as_i64())
    .bind(changeset)
    .execute(conn)
    .await
    .map_err(|e| written(&e, flow))?;
    Ok(())
}

fn verdict_name(v: Verdict) -> &'static str {
    match v {
        Verdict::Denied => "denied",
        Verdict::Allowed | Verdict::AllowedPending => "allowed",
    }
}

pub(crate) fn outcome_name(o: Outcome) -> &'static str {
    match o {
        Outcome::Automatic => "automatic",
        Outcome::Review => "review",
        Outcome::Noncompliant => "noncompliant",
    }
}

pub(crate) fn summary_name(v: SynthesisVerdict) -> &'static str {
    match v {
        SynthesisVerdict::Allowed => "allowed",
        SynthesisVerdict::Partial => "partial",
        SynthesisVerdict::Denied => "denied",
    }
}

/// The rule a decisive element names, if any.
pub(crate) fn decisive_rule(d: &Decisive) -> Option<String> {
    match d {
        Decisive::Rule(rule) | Decisive::Pattern { rule, .. } | Decisive::Pending { rule, .. } => {
            Some(rule.to_string())
        }
        _ => None,
    }
}

/// The decisive element, as written in an evaluation.
pub(crate) fn decisive_text(d: &Decisive) -> String {
    match d {
        Decisive::Unclassified { side, axis } => format!("unclassified:{side:?}:{axis}"),
        Decisive::Rule(rule) => format!("rule:{rule}"),
        Decisive::Pattern { pattern, rule } => format!("pattern:{pattern}:{rule}"),
        Decisive::Pending { rule, .. } => format!("pending:{rule}"),
        Decisive::NoRule => "no_rule".into(),
        Decisive::UnknownService => "unknown_service".into(),
    }
    .to_lowercase()
}

/// The identifier of the rule `key` valid at `at`.
pub(crate) async fn rule_id(
    conn: &mut PgConnection,
    key: &str,
    at: OffsetDateTime,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar("SELECT id FROM policy.policy_rule WHERE key = $1 AND valid_period @> $2")
        .bind(key)
        .bind(at)
        .fetch_optional(conn)
        .await
        .map_err(|e| store(&e))
}

/// The evaluation of `flow` at `at` as stored (design 011 « Données »): the rules and
/// conditions it names resolved to their identifiers at `at`.
pub(crate) async fn state_of(
    conn: &mut PgConnection,
    evaluated: &Evaluated,
    at: OffsetDateTime,
) -> Result<State, StoreError> {
    let mut rows: BTreeMap<(Uuid, Uuid), Row> = BTreeMap::new();
    for pair in &evaluated.pairs {
        if rows.contains_key(&(pair.source, pair.destination)) {
            continue;
        }
        let e = &pair.evaluation;
        let decisive = match decisive_rule(&e.decisive) {
            Some(key) => rule_id(conn, &key, at).await?,
            None => None,
        };
        let combination: serde_json::Map<String, Value> = e
            .combination
            .iter()
            .map(|((side, axis), value)| {
                (
                    format!("{}:{axis}", format!("{side:?}").to_lowercase()),
                    Value::from(value.as_str()),
                )
            })
            .collect();
        let unmet: Vec<String> = e
            .conditions
            .iter()
            .filter(|c| c.state == kartenn_policy::evaluate::ConditionState::Unmet)
            .map(|c| c.condition.to_string())
            .collect();
        let unmet: Vec<Uuid> = if unmet.is_empty() {
            Vec::new()
        } else {
            sqlx::query_scalar(
                "SELECT id FROM policy.policy_condition
                  WHERE key = ANY($1) AND valid_period @> $2 ORDER BY id",
            )
            .bind(&unmet)
            .bind(at)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| store(&e))?
        };
        rows.insert(
            (pair.source, pair.destination),
            Row {
                source: pair.source,
                destination: pair.destination,
                verdict: verdict_name(e.verdict).to_owned(),
                pending: e.verdict == Verdict::AllowedPending,
                outcome: outcome_name(e.outcome).to_owned(),
                reason: decisive_text(&e.decisive),
                decisive,
                combination: Value::Object(combination),
                unmet,
                score: Some(i32::try_from(e.score).unwrap_or(i32::MAX)),
            },
        );
    }
    let sorted = |addresses: &[Uuid]| -> Vec<Uuid> {
        let set: BTreeSet<Uuid> = addresses.iter().copied().collect();
        set.into_iter().collect()
    };
    Ok(State {
        sources: sorted(&evaluated.sources),
        destinations: sorted(&evaluated.destinations),
        rows: rows.into_values().collect(),
        summary: evaluated.summary.map(|s| Summary {
            verdict: summary_name(s.verdict).to_owned(),
            outcome: outcome_name(s.outcome).to_owned(),
            allowed: i32::try_from(s.allowed).unwrap_or(i32::MAX),
            denied: i32::try_from(s.denied).unwrap_or(i32::MAX),
        }),
    })
}

/// The stored evaluation of `flow` from `from` on, cut where any of its rows starts or
/// ends: each piece with the state that holds over it.
#[allow(clippy::too_many_lines)] // three tables read back into one state per piece
pub(crate) async fn read_states(
    conn: &mut PgConnection,
    flow: Uuid,
    from: OffsetDateTime,
) -> Result<Vec<Segment>, StoreError> {
    type Span = (OffsetDateTime, Option<OffsetDateTime>);
    let endpoints: Vec<(String, Uuid, OffsetDateTime, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT side, address_id, lower(valid_period), upper(valid_period)
           FROM flow.flow_endpoint_address
          WHERE flow_id = $1 AND valid_period && tstzrange($2, NULL)",
    )
    .bind(flow)
    .bind(from)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    #[allow(clippy::type_complexity)] // the columns of an evaluation
    let evaluations: Vec<(
        Uuid,
        Uuid,
        String,
        bool,
        String,
        String,
        Option<Uuid>,
        Value,
        Vec<Uuid>,
        Option<i32>,
        OffsetDateTime,
        Option<OffsetDateTime>,
    )> = sqlx::query_as(
        "SELECT source_address_id, destination_address_id, verdict, pending, outcome, reason,
                decisive_id, combination, unmet_conditions, score,
                lower(valid_period), upper(valid_period)
           FROM flow.flow_evaluation
          WHERE flow_id = $1 AND valid_period && tstzrange($2, NULL)",
    )
    .bind(flow)
    .bind(from)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let summaries: Vec<(
        String,
        String,
        i32,
        i32,
        OffsetDateTime,
        Option<OffsetDateTime>,
    )> = sqlx::query_as(
        "SELECT summary, outcome, allowed, denied, lower(valid_period), upper(valid_period)
               FROM flow.flow_summary
              WHERE flow_id = $1 AND valid_period && tstzrange($2, NULL)",
    )
    .bind(flow)
    .bind(from)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let mut cuts: BTreeSet<OffsetDateTime> = BTreeSet::from([from]);
    let spans = endpoints
        .iter()
        .map(|e| (e.2, e.3))
        .chain(evaluations.iter().map(|e| (e.10, e.11)))
        .chain(summaries.iter().map(|s| (s.4, s.5)));
    for (lower, upper) in spans {
        cuts.extend(
            [lower, upper.unwrap_or(from)]
                .into_iter()
                .filter(|t| *t > from),
        );
    }
    let covers =
        |(lower, upper): Span, at: OffsetDateTime| lower <= at && upper.is_none_or(|u| u > at);
    let cuts: Vec<OffsetDateTime> = cuts.into_iter().collect();
    let mut out = Vec::new();
    for (i, at) in cuts.iter().enumerate() {
        let mut state = State::default();
        for (side, address, lower, upper) in &endpoints {
            if covers((*lower, *upper), *at) {
                if side == "source" {
                    state.sources.push(*address);
                } else {
                    state.destinations.push(*address);
                }
            }
        }
        state.sources.sort_unstable();
        state.destinations.sort_unstable();
        for e in &evaluations {
            if covers((e.10, e.11), *at) {
                let mut unmet = e.8.clone();
                unmet.sort_unstable();
                state.rows.push(Row {
                    source: e.0,
                    destination: e.1,
                    verdict: e.2.clone(),
                    pending: e.3,
                    outcome: e.4.clone(),
                    reason: e.5.clone(),
                    decisive: e.6,
                    combination: e.7.clone(),
                    unmet,
                    score: e.9,
                });
            }
        }
        state.rows.sort_by_key(|r| (r.source, r.destination));
        state.summary = summaries
            .iter()
            .find(|s| covers((s.4, s.5), *at))
            .map(|s| Summary {
                verdict: s.0.clone(),
                outcome: s.1.clone(),
                allowed: s.2,
                denied: s.3,
            });
        out.push(Segment {
            from: *at,
            to: cuts.get(i + 1).copied(),
            state,
            revision: 0,
        });
    }
    Ok(out)
}

/// Rewrites the evaluation of `flow` from `from` on with `segments` (design 011
/// « Réévaluation », step 3): the rows from `from` on are ended or removed, then each
/// segment written with its addresses, one row per pair and its summary.
pub(crate) async fn write_states(
    conn: &mut PgConnection,
    flow: Uuid,
    from: OffsetDateTime,
    segments: &[Segment],
    changeset: Uuid,
) -> Result<(), StoreError> {
    end_rows(conn, DERIVED, flow, Some(from), changeset).await?;
    for segment in segments.iter().filter(|s| !s.state.is_empty()) {
        write_state(conn, flow, segment, changeset).await?;
    }
    Ok(())
}

async fn write_state(
    conn: &mut PgConnection,
    flow: Uuid,
    segment: &Segment,
    changeset: Uuid,
) -> Result<(), StoreError> {
    let (from, to, state) = (segment.from, segment.to, &segment.state);
    for (side, addresses) in [
        ("source", &state.sources),
        ("destination", &state.destinations),
    ] {
        for address in addresses {
            sqlx::query(
                "INSERT INTO flow.flow_endpoint_address (flow_id, side, address_id, valid_period,
                                                         changeset_id)
                 VALUES ($1, $2, $3, tstzrange($4, $5), $6)",
            )
            .bind(flow)
            .bind(side)
            .bind(address)
            .bind(from)
            .bind(to)
            .bind(changeset)
            .execute(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
        }
    }
    for row in &state.rows {
        sqlx::query(
            "INSERT INTO flow.flow_evaluation (flow_id, source_address_id, destination_address_id,
                    verdict, pending, outcome, reason, decisive_id, combination, unmet_conditions,
                    score, policy_revision, valid_period, changeset_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, tstzrange($13, $14), $15)",
        )
        .bind(flow)
        .bind(row.source)
        .bind(row.destination)
        .bind(&row.verdict)
        .bind(row.pending)
        .bind(&row.outcome)
        .bind(&row.reason)
        .bind(row.decisive)
        .bind(&row.combination)
        .bind(&row.unmet)
        .bind(row.score)
        .bind(segment.revision)
        .bind(from)
        .bind(to)
        .bind(changeset)
        .execute(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    }
    if let Some(s) = &state.summary {
        sqlx::query(
            "INSERT INTO flow.flow_summary (flow_id, summary, outcome, allowed, denied,
                                            valid_period, changeset_id)
             VALUES ($1, $2, $3, $4, $5, tstzrange($6, $7), $8)",
        )
        .bind(flow)
        .bind(&s.verdict)
        .bind(&s.outcome)
        .bind(s.allowed)
        .bind(s.denied)
        .bind(from)
        .bind(to)
        .bind(changeset)
        .execute(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    }
    Ok(())
}

/// Freezes the evaluation of `flow` at its registration (REQ-011-23): written once.
pub(crate) async fn register_evaluation(
    conn: &mut PgConnection,
    flow: Uuid,
    evaluated: &Evaluated,
    reviewed_by: Option<Uuid>,
    at: OffsetDateTime,
    changeset: Uuid,
) -> Result<(), StoreError> {
    let Some(s) = &evaluated.summary else {
        return Ok(());
    };
    let decisive = match evaluated
        .pairs
        .iter()
        .find(|p| p.evaluation.outcome == s.outcome)
        .and_then(|p| decisive_rule(&p.evaluation.decisive))
    {
        Some(key) => rule_id(conn, &key, at).await?,
        None => None,
    };
    sqlx::query(
        "INSERT INTO flow.flow_registration (flow_id, summary, outcome, decisive_id,
                                             policy_revision, reviewed_by, changeset_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (flow_id) DO NOTHING",
    )
    .bind(flow)
    .bind(summary_name(s.verdict))
    .bind(outcome_name(s.outcome))
    .bind(decisive)
    .bind(i64::try_from(evaluated.revision).unwrap_or(i64::MAX))
    .bind(reviewed_by)
    .bind(changeset)
    .execute(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(())
}

/// Records the rejection of a request at review (REQ-011-22, 25).
#[allow(clippy::too_many_arguments)] // the columns of a non-conformity
pub(crate) async fn reject(
    conn: &mut PgConnection,
    changeset: Uuid,
    request: &Value,
    evaluation: &Value,
    reason: &str,
    reviewed_by: Uuid,
    requester: Uuid,
    entity: Uuid,
) -> Result<(), StoreError> {
    let rule = match evaluation.get("rule").and_then(Value::as_str) {
        Some(key) => {
            let now: OffsetDateTime = sqlx::query_scalar("SELECT now()")
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| store(&e))?;
            rule_id(conn, key, now).await?
        }
        None => None,
    };
    sqlx::query(
        "INSERT INTO flow.flow_rejection (changeset_id, request, evaluation, reason, rule_id,
                                          reviewed_by, requester_id, entity_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (changeset_id) DO NOTHING",
    )
    .bind(changeset)
    .bind(request)
    .bind(evaluation)
    .bind(reason)
    .bind(rule)
    .bind(reviewed_by)
    .bind(requester)
    .bind(entity)
    .execute(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(())
}

/// The evaluation of a triage, as recorded in the request (REQ-011-21).
pub(crate) fn triage_json(evaluated: &Evaluated) -> Value {
    let worst = evaluated.summary.as_ref().and_then(|s| {
        evaluated
            .pairs
            .iter()
            .find(|p| p.evaluation.outcome == s.outcome)
    });
    json!({
        "summary": evaluated.summary.map(|s| summary_name(s.verdict)),
        "outcome": evaluated.summary.map_or("review", |s| outcome_name(s.outcome)),
        "revision": evaluated.revision,
        "decisive": worst.map(|p| decisive_text(&p.evaluation.decisive)),
        "rule": worst.and_then(|p| decisive_rule(&p.evaluation.decisive)),
        "pairs": evaluated.pairs.len(),
    })
}
