//! The query of compliance (feature `pg`; REQ-010-13, 14, 15, 25, 31, 38; design 010
//! « Évaluation », « Droits »): two endpoints of the addressing tree or of the module
//! Machines, a service, a port or a flow type, an optional file; a verdict per pair of
//! addresses and service, their synthesis, read without any write.
//!
//! The endpoints are developed into their addresses at `valid_at` (an address: itself; a
//! prefix: itself, on its own classifications; an interface or a machine: its addresses,
//! port of reading of the 009). The values of the axes are read for every address in one
//! grouped read (N9) and, for an axis carried by the machine, composed with the values of
//! the machines attached (S27): their union, or the value of the tree when no machine
//! carries one. The verdicts are those of [`crate::evaluate::evaluate`] on the snapshot
//! of the date ([`SnapshotCache`]).
//!
//! Rights (REQ-010-25): `policy.verdict.read` and the reading of both endpoints give the
//! whole explanation; else `policy.flow.request` on the entity of the request gives the
//! verdicts with an explanation reduced to values, rules, patterns and conditions; else the
//! query answers as for an absent object.

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::period::Instant;
use kartenn_machines::read::{addresses_of_interface, addresses_of_machine, instant};
use kartenn_service::authz::{Authorizer, Subject};
use kartenn_service::port::{Reader, StoreError};
use kartenn_service::reads::read_permission;
use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::cache::SnapshotCache;
use crate::evaluate::{
    Dossier, Endpoint, Evaluation, Requested, Synthesis, evaluate, synthesis, unknown_service,
};
use crate::model::{AxisKey, AxisValue, Carrier, ConditionKind, Key, SignalKind, Transport};
use crate::snapshot::PolicySnapshot;

/// Most addresses an endpoint develops into (design 010 « Évaluation »).
pub const MAX_ADDRESSES: usize = 64;

/// Most evaluations of a query, pairs times services.
pub const MAX_EVALUATIONS: usize = 10_000;

/// Permission to query the compliance with the whole explanation (S15).
pub const VERDICT_READ: &str = "policy.verdict.read";

/// Permission to request a flow on an entity, which gives the verdicts of its requests with
/// a reduced explanation (REQ-010-25).
pub const FLOW_REQUEST: &str = "policy.flow.request";

/// The axis whose values are environments, always read (S22).
const ENVIRONMENT: &str = "policy.environment";

/// The kind of an endpoint (REQ-010-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointKind {
    /// An address of the core.
    Address,
    /// A prefix of the core, evaluated on its own classifications.
    Prefix,
    /// An interface of the module Machines.
    Interface,
    /// A machine of the module Machines.
    Machine,
}

impl EndpointKind {
    /// The kind of object a read by identifier names (`kartenn_store_pg::read::object`).
    fn object_kind(self) -> &'static str {
        match self {
            Self::Address => "address",
            Self::Prefix => "prefix",
            Self::Interface => "machines.interface",
            Self::Machine => "machines.machine",
        }
    }
}

/// An endpoint as the request names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointRef {
    /// Its kind.
    pub kind: EndpointKind,
    /// Its identifier.
    pub id: Uuid,
}

/// What the request evaluates (REQ-010-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Object {
    /// A service.
    Service(Key),
    /// A transport and a port: each service that covers it.
    Port(Transport, u16),
    /// A flow type: each of its services.
    FlowType(Key),
}

impl Object {
    fn requested(&self) -> Requested {
        match self {
            Self::Service(_) => Requested::Service,
            Self::Port(..) => Requested::Port,
            Self::FlowType(_) => Requested::FlowType,
        }
    }
}

/// A query of compliance (REQ-010-13, 14, 38).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// The source.
    pub source: EndpointRef,
    /// The destination.
    pub destination: EndpointRef,
    /// What is evaluated.
    pub object: Object,
    /// The file joined, if any (REQ-010-38): without it, the conditions that read it are
    /// not evaluated.
    pub dossier: Option<Dossier>,
    /// The entity of the request, from the file: where `policy.flow.request` is looked for.
    pub entity: Option<Uuid>,
    /// Valid time of the query.
    pub valid_at: OffsetDateTime,
    /// Assertion time (now when `None`).
    pub as_of: Option<OffsetDateTime>,
}

/// How much of the explanation the caller sees (REQ-010-25).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// `policy.verdict.read` and the reading of both endpoints: everything.
    Full,
    /// `policy.flow.request` on the entity of the request: values, rules, patterns and
    /// conditions, without objects of the referential.
    Reduced,
}

/// Where the values of an address on an axis come from (REQ-010-31).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The addressing tree: the object carrying the assignment and the chain walked, both
    /// empty in a reduced explanation or when nothing applies.
    Tree {
        /// The object carrying the applicable assignment.
        origin: Option<Uuid>,
        /// Objects walked from the address to the origin.
        chain: Vec<Uuid>,
    },
    /// The machines attached to the address, whose values are united; empty in a reduced
    /// explanation.
    Machine {
        /// The machines carrying a value.
        machines: Vec<Uuid>,
    },
}

/// The values of an endpoint address on an axis and their source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisResolution {
    /// The axis.
    pub axis: AxisKey,
    /// The values, empty when not classified.
    pub values: BTreeSet<AxisValue>,
    /// Their source.
    pub source: Source,
}

/// An address of an endpoint, as the explanation shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Point {
    /// The address (or the prefix); `None` in a reduced explanation, which names no object
    /// of the referential.
    pub id: Option<Uuid>,
    /// Its values per axis read.
    pub axes: Vec<AxisResolution>,
}

/// One evaluation of the answer: a pair of points and a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairEvaluation {
    /// Index of the source point in [`Answer::sources`].
    pub source: usize,
    /// Index of the destination point in [`Answer::destinations`].
    pub destination: usize,
    /// The evaluation.
    pub evaluation: Evaluation,
}

/// Why an answer has no evaluation (REQ-010-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Empty {
    /// An endpoint has no address at the date.
    NoAddress,
}

/// The answer of a query of compliance (REQ-010-13, 15, 25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// What the caller sees.
    pub access: Access,
    /// Revision of the policy read.
    pub revision: u64,
    /// The points of the source.
    pub sources: Vec<Point>,
    /// The points of the destination.
    pub destinations: Vec<Point>,
    /// The evaluations, pairs in the order of the points, then services by key.
    pub evaluations: Vec<PairEvaluation>,
    /// Their synthesis; `None` without evaluation.
    pub synthesis: Option<Synthesis>,
    /// Why there is no evaluation, if so.
    pub empty: Option<Empty>,
    /// Addresses of a readable machine or interface the caller may not read one by one:
    /// left out, counted (design 010 « Droits »).
    pub hidden: usize,
}

/// Why a query is not answered.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    /// An endpoint does not exist then, is not of the kind named, or the caller may neither
    /// read the verdict nor request the flow: answered as an absent object (REQ-006-20).
    #[error("not found")]
    NotFound,
    /// An endpoint develops into more than [`MAX_ADDRESSES`] addresses, or the query into
    /// more than [`MAX_EVALUATIONS`] evaluations (`policy.query.too_large`).
    #[error("{0}")]
    TooLarge(String),
    /// Infrastructure failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// An endpoint developed: its addresses (or the prefix), with the width of each.
struct Developed {
    points: Vec<(Uuid, u128)>,
    /// Whether the points are addresses (machine axes apply) or a prefix.
    addresses: bool,
}

fn micros(t: OffsetDateTime) -> Instant {
    instant(t)
}

/// The number of addresses of a prefix `a.b.c.d/n` or `x::/n`.
fn width_of(prefix: &str) -> u128 {
    let Some((address, length)) = prefix.split_once('/') else {
        return 1;
    };
    let bits: u32 = if address.contains(':') { 128 } else { 32 };
    let length: u32 = length.parse().unwrap_or(bits).min(bits);
    1u128.checked_shl(bits - length).unwrap_or(u128::MAX)
}

/// Values of a closed list as the storage resolves them: one key or a list of keys.
fn values_of(value: Option<&Value>) -> BTreeSet<AxisValue> {
    match value {
        Some(Value::String(s)) => BTreeSet::from([AxisValue::new(s.clone())]),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(AxisValue::new))
            .collect(),
        Some(Value::Null) | None => BTreeSet::new(),
        Some(other) => BTreeSet::from([AxisValue::new(other.to_string())]),
    }
}

/// The axes the evaluation may read: those designated, `policy.environment`, and those
/// the rules, conditions and signals cite.
fn axes_read(snapshot: &PolicySnapshot) -> BTreeSet<AxisKey> {
    let mut found = snapshot.axis_keys();
    if let Ok(environment) = AxisKey::parse(ENVIRONMENT) {
        found.insert(environment);
    }
    for rule in snapshot.rules() {
        found.extend(rule.source.keys().cloned());
        found.extend(rule.destination.keys().cloned());
        found.extend(rule.same.iter().cloned());
    }
    for condition in snapshot.conditions.values() {
        if let ConditionKind::DossierRankAtMost(axis) = &condition.kind {
            found.insert(axis.clone());
        }
    }
    for signal in snapshot.signals.values() {
        match &signal.kind {
            SignalKind::AxisDiffers(axis)
            | SignalKind::DestinationHigherRank(axis)
            | SignalKind::EndpointValue(axis, _)
            | SignalKind::RankAtLeast(axis, _) => {
                found.insert(axis.clone());
            }
            _ => {}
        }
    }
    found
}

/// Develops an endpoint into its points at the date, after checking its kind.
async fn develop<R: Reader>(
    conn: &mut PgConnection,
    reader: &R,
    endpoint: EndpointRef,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Developed, QueryError> {
    let object = reader
        .object(endpoint.id, valid_at, as_of)
        .await?
        .filter(|o| o.kind == endpoint.kind.object_kind())
        .ok_or(QueryError::NotFound)?;
    let points = match endpoint.kind {
        EndpointKind::Address => vec![(endpoint.id, 1)],
        EndpointKind::Prefix => {
            let width = object
                .fields
                .get("prefix")
                .and_then(Value::as_str)
                .map_or(1, width_of);
            vec![(endpoint.id, width)]
        }
        EndpointKind::Interface => addresses_of_interface(conn, endpoint.id, valid_at, as_of)
            .await?
            .into_iter()
            .map(|a| (a.address, 1))
            .collect(),
        EndpointKind::Machine => {
            let mut seen = BTreeSet::new();
            addresses_of_machine(conn, endpoint.id, valid_at, as_of)
                .await?
                .into_iter()
                .filter(|a| seen.insert(a.address))
                .map(|a| (a.address, 1))
                .collect()
        }
    };
    if points.len() > MAX_ADDRESSES {
        return Err(QueryError::TooLarge(format!(
            "une extrémité compte {} adresses, au plus {MAX_ADDRESSES}",
            points.len()
        )));
    }
    Ok(Developed {
        points,
        addresses: endpoint.kind != EndpointKind::Prefix,
    })
}

/// Whether `subject` holds `permission` on every one of `objects`.
async fn holds<A: Authorizer>(
    authorizer: &A,
    subject: &Subject,
    permission: &str,
    objects: &[Uuid],
) -> Result<bool, StoreError> {
    Ok(authorizer
        .authorize_many(subject, permission, objects)
        .await?
        .is_empty())
}

/// What `subject` may see of the query, `None` when it may see nothing (REQ-010-25).
async fn access<A: Authorizer>(
    authorizer: &A,
    subject: &Subject,
    query: &Query,
) -> Result<Option<Access>, StoreError> {
    let ends = [query.source, query.destination];
    let ids = ends.map(|e| e.id);
    let mut full = holds(authorizer, subject, VERDICT_READ, &ids).await?;
    for end in ends {
        if !full {
            break;
        }
        full = holds(
            authorizer,
            subject,
            &read_permission(end.kind.object_kind()),
            &[end.id],
        )
        .await?;
    }
    if full {
        return Ok(Some(Access::Full));
    }
    if let Some(entity) = query.entity
        && holds(authorizer, subject, FLOW_REQUEST, &[entity]).await?
    {
        return Ok(Some(Access::Reduced));
    }
    Ok(None)
}

/// Answers `query` for `subject` (the system when `None`), reading the policy through
/// `cache`, the objects and their values through `reader`, the module Machines on `conn`
/// (REQ-010-13 to 15, 25, 31, 38). Writes nothing.
///
/// # Errors
///
/// [`QueryError::NotFound`] for an absent or invisible endpoint, or a caller without right;
/// [`QueryError::TooLarge`] beyond the bounds; [`QueryError::Store`] on failure.
#[allow(clippy::too_many_lines)] // the steps of design 010 « Évaluation », in their order
pub async fn query<R: Reader, A: Authorizer>(
    conn: &mut PgConnection,
    reader: &R,
    authorizer: &A,
    cache: &SnapshotCache,
    subject: Option<&Subject>,
    query: &Query,
) -> Result<Answer, QueryError> {
    let valid_at = micros(query.valid_at);
    let as_of = query.as_of.map(micros);
    let access = match subject {
        None => Access::Full,
        Some(subject) => access(authorizer, subject, query)
            .await?
            .ok_or(QueryError::NotFound)?,
    };
    let mut source = develop(conn, reader, query.source, valid_at, as_of).await?;
    let mut destination = develop(conn, reader, query.destination, valid_at, as_of).await?;
    // Addresses of a machine or an interface the caller may not read one by one: left out
    // and counted, in a full answer (a reduced one shows no address).
    let mut hidden = 0;
    if access == Access::Full
        && let Some(subject) = subject
    {
        for developed in [&mut source, &mut destination] {
            if !developed.addresses {
                continue;
            }
            let ids: Vec<Uuid> = developed.points.iter().map(|(id, _)| *id).collect();
            let unreadable: BTreeSet<Uuid> = authorizer
                .authorize_many(subject, &read_permission("address"), &ids)
                .await?
                .into_iter()
                .collect();
            hidden += unreadable.len();
            developed.points.retain(|(id, _)| !unreadable.contains(id));
        }
    }
    let snapshot = cache.snapshot(conn, query.valid_at, query.as_of).await?;
    let services = services_of(&snapshot, &query.object);
    let evaluations_count = source.points.len() * destination.points.len() * services.len().max(1);
    if evaluations_count > MAX_EVALUATIONS {
        return Err(QueryError::TooLarge(format!(
            "la requête compte {evaluations_count} évaluations, au plus {MAX_EVALUATIONS}"
        )));
    }
    let axes = axes_read(&snapshot);
    let points = resolve(
        conn,
        reader,
        &snapshot,
        &axes,
        [&source, &destination],
        valid_at,
        as_of,
    )
    .await?;
    let shown = |developed: &Developed| -> Vec<Point> {
        developed
            .points
            .iter()
            .map(|(id, _)| reduce(points[id].1.clone(), *id, access))
            .collect()
    };
    let mut answer = Answer {
        access,
        revision: snapshot.revision(),
        sources: shown(&source),
        destinations: shown(&destination),
        evaluations: Vec::new(),
        synthesis: None,
        empty: None,
        hidden,
    };
    if source.points.is_empty() || destination.points.is_empty() {
        answer.empty = Some(Empty::NoAddress);
        return Ok(answer);
    }
    let requested = query.object.requested();
    for (i, (a, _)) in source.points.iter().enumerate() {
        for (j, (b, _)) in destination.points.iter().enumerate() {
            let (ea, eb) = (&points[a].0, &points[b].0);
            if services.is_empty() {
                answer.evaluations.push(PairEvaluation {
                    source: i,
                    destination: j,
                    evaluation: unknown_service(),
                });
            }
            for service in &services {
                answer.evaluations.push(PairEvaluation {
                    source: i,
                    destination: j,
                    evaluation: evaluate(
                        &snapshot,
                        ea,
                        eb,
                        service,
                        requested,
                        query.dossier.as_ref(),
                    ),
                });
            }
        }
    }
    let evaluations: Vec<Evaluation> = answer
        .evaluations
        .iter()
        .map(|e| e.evaluation.clone())
        .collect();
    answer.synthesis = synthesis(&evaluations);
    Ok(answer)
}

/// The services the object of a query stands for, by key; none for a port no service
/// covers, an unknown service or a flow type without service (each pair is then
/// `unknown_service`, to be reviewed, REQ-010-14).
fn services_of(snapshot: &PolicySnapshot, object: &Object) -> Vec<Key> {
    let mut keys: Vec<Key> = match object {
        Object::Service(key) => snapshot
            .service(key)
            .map(|s| s.key.clone())
            .into_iter()
            .collect(),
        Object::Port(transport, port) => snapshot
            .services_for_port(*transport, *port)
            .into_iter()
            .map(|s| s.key.clone())
            .collect(),
        Object::FlowType(flow_type) => snapshot
            .services_of_type(flow_type)
            .into_iter()
            .map(|s| s.key.clone())
            .collect(),
    };
    keys.sort();
    keys.dedup();
    keys
}

/// The explanation of a point as `access` shows it.
fn reduce(mut axes: Vec<AxisResolution>, id: Uuid, access: Access) -> Point {
    if access == Access::Full {
        return Point { id: Some(id), axes };
    }
    for axis in &mut axes {
        axis.source = match axis.source {
            Source::Tree { .. } => Source::Tree {
                origin: None,
                chain: Vec::new(),
            },
            Source::Machine { .. } => Source::Machine {
                machines: Vec::new(),
            },
        };
    }
    Point { id: None, axes }
}

/// Per object and axis: the values of the tree, their origin and the chain walked.
type TreeValues = BTreeMap<(Uuid, String), (BTreeSet<AxisValue>, Option<Uuid>, Vec<Uuid>)>;

/// The endpoint of the evaluation and the explanation of each point: one grouped read of
/// the values of `axes` for every point (N9), then, for the axes carried by the machine,
/// the values of the machines attached to each address (port of reading of the 009, M10)
/// composed per S27.
async fn resolve<R: Reader>(
    conn: &mut PgConnection,
    reader: &R,
    snapshot: &PolicySnapshot,
    axes: &BTreeSet<AxisKey>,
    sides: [&Developed; 2],
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<BTreeMap<Uuid, (Endpoint, Vec<AxisResolution>)>, QueryError> {
    let mut widths: BTreeMap<Uuid, u128> = BTreeMap::new();
    let mut addresses: BTreeSet<Uuid> = BTreeSet::new();
    for side in sides {
        for (id, width) in &side.points {
            widths.insert(*id, *width);
            if side.addresses {
                addresses.insert(*id);
            }
        }
    }
    let objects: Vec<Uuid> = widths.keys().copied().collect();
    let keys: Vec<String> = axes.iter().map(ToString::to_string).collect();
    let tree: TreeValues = reader
        .resolved_many(&objects, &keys, valid_at, as_of)
        .await?
        .into_iter()
        .map(|r| {
            (
                (r.object, r.property),
                (values_of(r.value.as_ref()), r.origin, r.chain),
            )
        })
        .collect();
    let machine_axes: Vec<String> = axes
        .iter()
        .filter(|a| {
            snapshot
                .axes
                .get(*a)
                .is_some_and(|axis| axis.carrier == Carrier::Machine)
        })
        .map(ToString::to_string)
        .collect();
    // Per address and machine axis: the union of the values of its machines, and those
    // machines.
    let mut machine: BTreeMap<(Uuid, String), (BTreeSet<AxisValue>, Vec<Uuid>)> = BTreeMap::new();
    if !machine_axes.is_empty() && !addresses.is_empty() {
        let list: Vec<Uuid> = addresses.iter().copied().collect();
        let keys: Vec<&str> = machine_axes.iter().map(String::as_str).collect();
        for (address, machines) in
            kartenn_machines::read::machine_values_of_addresses(conn, &list, &keys, valid_at, as_of)
                .await?
        {
            for m in machines {
                for value in m.values {
                    let values = values_of(value.value.as_ref());
                    if values.is_empty() {
                        continue;
                    }
                    let entry = machine.entry((address, value.key)).or_default();
                    entry.0.extend(values);
                    if !entry.1.contains(&m.machine) {
                        entry.1.push(m.machine);
                    }
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    for (id, width) in widths {
        let mut endpoint = Endpoint {
            values: BTreeMap::new(),
            width,
            whole_domain: false,
        };
        let mut explanation = Vec::new();
        for axis in axes {
            let key = axis.to_string();
            let (values, source) = if let Some((values, machines)) = machine.get(&(id, key.clone()))
            {
                (
                    values.clone(),
                    Source::Machine {
                        machines: machines.clone(),
                    },
                )
            } else {
                let (values, origin, chain) = tree.get(&(id, key)).cloned().unwrap_or_default();
                (values, Source::Tree { origin, chain })
            };
            if !values.is_empty() {
                endpoint.values.insert(axis.clone(), values.clone());
            }
            explanation.push(AxisResolution {
                axis: axis.clone(),
                values,
                source,
            });
        }
        out.insert(id, (endpoint, explanation));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_width_of_a_prefix() {
        assert_eq!(width_of("10.1.0.0/24"), 256);
        assert_eq!(width_of("10.1.0.5/32"), 1);
        assert_eq!(width_of("2001:db8::/64"), 1u128 << 64);
        assert_eq!(width_of("::/0"), u128::MAX);
    }

    #[test]
    fn values_of_a_key_or_a_list() {
        assert_eq!(
            values_of(Some(&Value::from("prod"))),
            BTreeSet::from([AxisValue::new("prod")])
        );
        assert_eq!(
            values_of(Some(&serde_json::json!(["a", "b"]))),
            BTreeSet::from([AxisValue::new("a"), AxisValue::new("b")])
        );
        assert!(values_of(None).is_empty());
    }
}
