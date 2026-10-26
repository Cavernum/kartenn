//! The evaluation of a flow against the policy (spec 010), in the transaction of a changeset
//! (REQ-011-8, 11, 21; design 011 « Réévaluation »): its endpoints developed into their
//! addresses, the values of the axes read on the state the changeset leaves, a verdict per
//! pair of addresses for its service, their summary.
//!
//! It reads the transaction, not the pool: an address created, classified or attached in
//! the same changeset counts. The policy of a committed revision is kept for the instance
//! (task 011-10); a transaction that wrote the policy reads its own at each call, since a
//! changeset may raise a revision it then rolls back (a simulation). A pass over the flows
//! after the writes of a changeset reads it through [`Policies`], which lives as long as
//! the pass, and evaluates its flows by batches.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use kartenn_core::period::Instant;
use kartenn_machines::read::{addresses_of_interfaces, instant};
use kartenn_policy::evaluate::{
    Dossier, Endpoint as Side, Evaluation, Requested, Synthesis, evaluate, synthesis,
    unknown_service,
};
use kartenn_policy::model::{
    AxisKey, AxisValue, Carrier, ConditionKind, DossierField, Key, SignalKind,
};
use kartenn_policy::read::{self, Database};
use kartenn_policy::snapshot::{Parts, PolicySnapshot};
use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::model::{EndpointKind, Flow};

/// The axis whose values are environments, always read (010/S22).
const ENVIRONMENT: &str = "policy.environment";

/// The evaluation of one pair of addresses.
#[derive(Debug, Clone)]
pub struct Pair {
    /// Address (or prefix) of the source.
    pub source: Uuid,
    /// Address (or prefix) of the destination.
    pub destination: Uuid,
    /// Its evaluation for the service of the flow.
    pub evaluation: Evaluation,
}

/// A flow evaluated at a date.
#[derive(Debug, Clone)]
pub struct Evaluated {
    /// The revision of the policy read.
    pub revision: u64,
    /// The addresses of the source.
    pub sources: Vec<Uuid>,
    /// The addresses of the destination.
    pub destinations: Vec<Uuid>,
    /// One evaluation per pair.
    pub pairs: Vec<Pair>,
    /// Their summary; `None` when an endpoint holds no address.
    pub summary: Option<Synthesis>,
}

fn store(e: &sqlx::Error) -> StoreError {
    StoreError(e.to_string())
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

/// The axes the evaluation may read: those designated, `policy.environment`, and those the
/// rules, conditions and signals cite (as the query of compliance reads them, 010-13).
fn axes_read(parts: &Parts) -> BTreeSet<AxisKey> {
    let mut found: BTreeSet<AxisKey> = parts.axes.iter().map(|a| a.key.clone()).collect();
    if let Ok(environment) = AxisKey::parse(ENVIRONMENT) {
        found.insert(environment);
    }
    for rule in &parts.rules {
        found.extend(rule.source.keys().cloned());
        found.extend(rule.destination.keys().cloned());
        found.extend(rule.same.iter().cloned());
    }
    for condition in &parts.conditions {
        if let ConditionKind::DossierRankAtMost(axis) = &condition.kind {
            found.insert(axis.clone());
        }
    }
    for signal in &parts.signals {
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

/// The points each endpoint of `flows` develops into at `at`, in one query per kind of
/// endpoint: an address or a prefix, itself; an interface, its addresses (port of reading
/// of the 009); a range or a machine, nothing (F3, F21). With the width of each and whether they are addresses.
async fn develop(
    conn: &mut PgConnection,
    flows: &[&Flow],
    at: OffsetDateTime,
) -> Result<BTreeMap<(EndpointKind, Uuid), (Vec<(Uuid, u128)>, bool)>, StoreError> {
    let t: Instant = instant(at);
    let mut wanted: BTreeMap<EndpointKind, BTreeSet<Uuid>> = BTreeMap::new();
    for flow in flows {
        for end in [&flow.source, &flow.destination] {
            wanted.entry(end.kind).or_default().insert(end.id);
        }
    }
    let ids = |kind: EndpointKind| -> Vec<Uuid> {
        wanted
            .get(&kind)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    };
    let mut out = BTreeMap::new();
    for id in ids(EndpointKind::Address) {
        out.insert((EndpointKind::Address, id), (vec![(id, 1)], true));
    }
    // Never endpoints (F3, F21): no point, the check of the flow refuses them.
    for kind in [EndpointKind::Range, EndpointKind::Machine] {
        for id in ids(kind) {
            out.insert((kind, id), (Vec::new(), false));
        }
    }
    let prefixes = ids(EndpointKind::Prefix);
    if !prefixes.is_empty() {
        let found: BTreeMap<Uuid, String> = sqlx::query_as(
            "SELECT DISTINCT ON (id) id, prefix::text FROM public.prefix
              WHERE id = ANY($1) AND valid_period @> $2 ORDER BY id",
        )
        .bind(&prefixes)
        .bind(at)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| store(&e))?
        .into_iter()
        .collect();
        for id in prefixes {
            let width = found.get(&id).map_or(1, |p| width_of(p));
            out.insert((EndpointKind::Prefix, id), (vec![(id, width)], false));
        }
    }
    let owners = ids(EndpointKind::Interface);
    if !owners.is_empty() {
        for id in &owners {
            out.insert((EndpointKind::Interface, *id), (Vec::new(), true));
        }
        for (owner, address) in addresses_of_interfaces(conn, &owners, t, None).await? {
            if let Some((points, _)) = out.get_mut(&(EndpointKind::Interface, owner)) {
                points.push((address.address, 1));
            }
        }
    }
    Ok(out)
}

/// The resolved values of `keys` on `objects` at `at`, on the state of the transaction
/// (the origins materialised by the core, ADR 0016): closed-list values by key.
async fn tree_values(
    conn: &mut PgConnection,
    objects: &[Uuid],
    keys: &[String],
    at: OffsetDateTime,
) -> Result<BTreeMap<(Uuid, String), BTreeSet<AxisValue>>, StoreError> {
    if objects.is_empty() || keys.is_empty() {
        return Ok(BTreeMap::new());
    }
    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT o.object_id, d.key, e.key
           FROM public.property_origin o
           JOIN public.property_declaration d
             ON d.id = o.property_id AND d.key = ANY($2) AND d.valid_period @> $3
           JOIN public.property_assignment a
             ON a.object_id = o.origin_object_id AND a.property_id = o.property_id
            AND a.valid_period @> $3 AND a.kind = 'value'
           LEFT JOIN public.property_enum_value e
             ON e.id = a.v_uuid OR e.id = ANY(a.v_uuids)
          WHERE o.object_id = ANY($1) AND o.valid_period @> $3",
    )
    .persistent(false)
    .bind(objects)
    .bind(keys)
    .bind(at)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let mut out: BTreeMap<(Uuid, String), BTreeSet<AxisValue>> = BTreeMap::new();
    for (object, key, value) in rows {
        if let Some(value) = value {
            out.entry((object, key))
                .or_default()
                .insert(AxisValue::new(value));
        }
    }
    Ok(out)
}

/// The values of the policy that `flows` name as the nature of their data, by
/// identifier, at `at`: one query for the batch.
async fn natures(
    conn: &mut PgConnection,
    flows: &[&Flow],
    at: OffsetDateTime,
) -> Result<BTreeMap<Uuid, Vec<(String, String)>>, StoreError> {
    let named: BTreeSet<Uuid> = flows
        .iter()
        .flat_map(|f| f.data_nature.iter().copied())
        .collect();
    let mut out: BTreeMap<Uuid, Vec<(String, String)>> = BTreeMap::new();
    if named.is_empty() {
        return Ok(out);
    }
    let named: Vec<Uuid> = named.into_iter().collect();
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, property_key, value_key FROM policy.policy_value
          WHERE id = ANY($1) AND valid_period @> $2",
    )
    .bind(&named)
    .bind(at)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    for (id, axis, value) in rows {
        out.entry(id).or_default().push((axis, value));
    }
    Ok(out)
}

/// The file of a flow (REQ-010-38): the fields it carries, and the nature of its data on
/// the axes of the values it names.
fn dossier(flow: &Flow, natures: &BTreeMap<Uuid, Vec<(String, String)>>) -> Dossier {
    let mut fields = BTreeSet::from([DossierField::Requester]);
    if !flow.external_refs.is_empty() {
        fields.insert(DossierField::ExternalRefs);
    }
    if !flow.responsibles.is_empty() {
        fields.insert(DossierField::Owners);
    }
    if flow.period.end().is_some() {
        fields.insert(DossierField::Period);
    }
    let mut values: BTreeMap<AxisKey, BTreeSet<AxisValue>> = BTreeMap::new();
    if !flow.data_nature.is_empty() {
        fields.insert(DossierField::DataNature);
        let named: BTreeSet<Uuid> = flow.data_nature.iter().copied().collect();
        for (axis, value) in named.iter().filter_map(|id| natures.get(id)).flatten() {
            if let Ok(axis) = AxisKey::parse(axis) {
                values
                    .entry(axis)
                    .or_default()
                    .insert(AxisValue::new(value.clone()));
            }
        }
    }
    Dossier { fields, values }
}

/// The policy an evaluation reads at an instant: its snapshot and the axes it may read.
pub(crate) struct Loaded {
    snapshot: PolicySnapshot,
    axes: BTreeSet<AxisKey>,
    machine_axes: Vec<String>,
}

impl Loaded {
    /// The policy valid at `at` as the transaction sees it.
    async fn read(conn: &mut PgConnection, at: OffsetDateTime) -> Result<Self, StoreError> {
        let revision = read::revision(conn).await?;
        let parts = read::parts(conn, at, None, revision).await?;
        let axes = axes_read(&parts);
        let machine_axes = parts
            .axes
            .iter()
            .filter(|axis| axis.carrier == Carrier::Machine)
            .map(|axis| axis.key.to_string())
            .collect();
        Ok(Self {
            snapshot: PolicySnapshot::new(parts),
            axes,
            machine_axes,
        })
    }
}

/// Policies kept by one pass, the oldest dropped first when full.
const KEPT: usize = 16;

/// The policies read by one pass over the flows once every write of a changeset is done:
/// the policy no longer changes in the transaction, and a snapshot read at an instant holds
/// over the span where no version of a policy object starts or ends
/// ([`read::stable_span`]). It lives as long as the pass, never beyond its transaction.
#[derive(Default)]
pub(crate) struct Policies {
    kept: Vec<(Option<OffsetDateTime>, Option<OffsetDateTime>, Arc<Loaded>)>,
}

impl Policies {
    /// The policy valid at `at`, read once per stable span.
    async fn at(
        &mut self,
        conn: &mut PgConnection,
        at: OffsetDateTime,
    ) -> Result<Arc<Loaded>, StoreError> {
        let covers = |from: &Option<OffsetDateTime>, until: &Option<OffsetDateTime>| {
            from.is_none_or(|f| f <= at) && until.is_none_or(|u| at < u)
        };
        if let Some((_, _, hit)) = self.kept.iter().find(|(f, u, _)| covers(f, u)) {
            return Ok(hit.clone());
        }
        let kept = load_at(conn, at).await?;
        let loaded = kept.2.clone();
        if self.kept.len() >= KEPT {
            self.kept.remove(0);
        }
        self.kept.push(kept);
        Ok(loaded)
    }
}

/// A policy with the span of valid time it holds over, start included, end excluded.
type Kept = (Option<OffsetDateTime>, Option<OffsetDateTime>, Arc<Loaded>);

/// The policies of a committed revision of a database, shared by the evaluations of the
/// instance (task 011-10), as the cache of the queries of the 010 keeps its snapshots: a
/// revision committed names one state of the policy of its database, every write of it
/// raising the revision. Two databases served by the process may share a revision.
static COMMITTED: Mutex<Option<(Database, u64, Vec<Kept>)>> = Mutex::new(None);

/// The policy valid at `at` and the span it holds over: kept from a committed revision
/// when the transaction did not write the policy, read otherwise.
async fn load_at(conn: &mut PgConnection, at: OffsetDateTime) -> Result<Kept, StoreError> {
    let committed = || COMMITTED.lock().unwrap_or_else(PoisonError::into_inner);
    let (database, revision) = read::committed(conn).await?;
    if let Some(revision) = revision {
        let hit = committed()
            .as_ref()
            .filter(|(d, r, _)| *d == database && *r == revision)
            .and_then(|(_, _, kept)| {
                kept.iter()
                    .find(|(f, u, _)| f.is_none_or(|f| f <= at) && u.is_none_or(|u| at < u))
                    .cloned()
            });
        if let Some(hit) = hit {
            return Ok(hit);
        }
    }
    let (from, until) = read::stable_span(conn, at, None).await?;
    let loaded = Arc::new(Loaded::read(conn, at).await?);
    // Kept only when no write of the policy committed meanwhile.
    if let Some(revision) = revision
        && loaded.snapshot.revision() == revision
        && read::committed(conn).await? == (database, Some(revision))
    {
        let mut shared = committed();
        if shared
            .as_ref()
            .is_none_or(|(d, r, _)| *d != database || *r != revision)
        {
            *shared = Some((database, revision, Vec::new()));
        }
        if let Some((_, _, kept)) = shared.as_mut() {
            if kept.len() >= KEPT {
                kept.remove(0);
            }
            kept.push((from, until, loaded.clone()));
        }
    }
    Ok((from, until, loaded))
}

/// Evaluates `flow` at `at` on the state of the transaction (REQ-011-8, 11, 21).
///
/// # Errors
///
/// A failure of the database.
pub async fn evaluate_flow(
    conn: &mut PgConnection,
    flow: &Flow,
    at: OffsetDateTime,
) -> Result<Evaluated, StoreError> {
    let (_, _, policy) = load_at(conn, at).await?;
    one(evaluate_with(conn, &policy, &[flow], at).await?)
}

/// [`evaluate_flow`] in a pass over the flows after the writes of a changeset, the policy
/// read through `policies`.
///
/// # Errors
///
/// A failure of the database.
pub(crate) async fn evaluate_in(
    conn: &mut PgConnection,
    policies: &mut Policies,
    flow: &Flow,
    at: OffsetDateTime,
) -> Result<Evaluated, StoreError> {
    one(evaluate_many(conn, policies, &[flow], at).await?)
}

/// [`evaluate_in`] for a batch of flows at the same instant, in their order: each read is
/// one query for the batch, the values of the axes by lots of [`LOT`] addresses (N9), the
/// evaluation in memory (design 011 « Réévaluation », step 2).
///
/// # Errors
///
/// A failure of the database.
pub(crate) async fn evaluate_many(
    conn: &mut PgConnection,
    policies: &mut Policies,
    flows: &[&Flow],
    at: OffsetDateTime,
) -> Result<Vec<Evaluated>, StoreError> {
    if flows.is_empty() {
        return Ok(Vec::new());
    }
    let policy = policies.at(conn, at).await?;
    evaluate_with(conn, &policy, flows, at).await
}

fn one(mut found: Vec<Evaluated>) -> Result<Evaluated, StoreError> {
    found
        .pop()
        .ok_or_else(|| StoreError("a flow evaluated to nothing".into()))
}

/// The addresses whose values one query reads (N9).
pub(crate) const LOT: usize = 512;

/// Evaluates `flows` at `at` against `policy`, in their order.
#[allow(clippy::too_many_lines)] // the steps of design 010 « Évaluation », in their order
async fn evaluate_with(
    conn: &mut PgConnection,
    policy: &Loaded,
    flows: &[&Flow],
    at: OffsetDateTime,
) -> Result<Vec<Evaluated>, StoreError> {
    let Loaded {
        snapshot,
        axes,
        machine_axes,
    } = policy;
    let ids: BTreeSet<Uuid> = flows.iter().map(|f| f.service).collect();
    let ids: Vec<Uuid> = ids.into_iter().collect();
    let services: BTreeMap<Uuid, Key> = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, key FROM policy.policy_service WHERE id = ANY($1) AND valid_period @> $2",
    )
    .bind(&ids)
    .bind(at)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?
    .into_iter()
    .filter_map(|(id, key)| Some((id, Key::new(&key).ok()?)))
    .collect();
    let developed = develop(conn, flows, at).await?;
    let keys: Vec<String> = axes.iter().map(ToString::to_string).collect();
    let mut widths: BTreeMap<Uuid, u128> = BTreeMap::new();
    let mut addresses: BTreeSet<Uuid> = BTreeSet::new();
    for (points, are_addresses) in developed.values() {
        for (id, width) in points {
            widths.insert(*id, *width);
            if *are_addresses {
                addresses.insert(*id);
            }
        }
    }
    let objects: Vec<Uuid> = widths.keys().copied().collect();
    let mut tree = BTreeMap::new();
    for lot in objects.chunks(LOT) {
        tree.extend(tree_values(conn, lot, &keys, at).await?);
    }
    // An axis carried by the machine: the union of the values of the machines holding the
    // address, else the value of the tree (S27).
    let mut machine: BTreeMap<(Uuid, String), BTreeSet<AxisValue>> = BTreeMap::new();
    let list: Vec<Uuid> = addresses.iter().copied().collect();
    let names: Vec<&str> = machine_axes.iter().map(String::as_str).collect();
    for lot in list.chunks(LOT).filter(|_| !names.is_empty()) {
        for (address, machines) in kartenn_machines::read::machine_values_of_addresses(
            conn,
            lot,
            &names,
            instant(at),
            None,
        )
        .await?
        {
            for m in machines {
                for value in m.values {
                    let found: BTreeSet<AxisValue> = match value.value.as_ref() {
                        Some(serde_json::Value::String(s)) => {
                            BTreeSet::from([AxisValue::new(s.clone())])
                        }
                        Some(serde_json::Value::Array(items)) => items
                            .iter()
                            .filter_map(|v| v.as_str().map(AxisValue::new))
                            .collect(),
                        _ => BTreeSet::new(),
                    };
                    if !found.is_empty() {
                        machine
                            .entry((address, value.key))
                            .or_default()
                            .extend(found);
                    }
                }
            }
        }
    }
    let side = |id: Uuid| -> Side {
        let mut values = BTreeMap::new();
        for axis in axes {
            let key = axis.to_string();
            let found = machine
                .get(&(id, key.clone()))
                .or_else(|| tree.get(&(id, key)))
                .cloned()
                .unwrap_or_default();
            if !found.is_empty() {
                values.insert(axis.clone(), found);
            }
        }
        Side {
            values,
            width: widths.get(&id).copied().unwrap_or(1),
            whole_domain: false,
        }
    };
    let sides: BTreeMap<Uuid, Side> = objects.iter().map(|id| (*id, side(*id))).collect();
    let natures = natures(conn, flows, at).await?;
    let none = (Vec::new(), false);
    let mut out = Vec::with_capacity(flows.len());
    for flow in flows {
        let points =
            |end: &crate::model::Endpoint| &developed.get(&(end.kind, end.id)).unwrap_or(&none).0;
        let (source, destination) = (points(&flow.source), points(&flow.destination));
        let service = services.get(&flow.service);
        let dossier = dossier(flow, &natures);
        let mut pairs = Vec::with_capacity(source.len() * destination.len());
        for (a, _) in source {
            for (b, _) in destination {
                let evaluation = match (service, sides.get(a), sides.get(b)) {
                    (Some(service), Some(a), Some(b)) => {
                        evaluate(snapshot, a, b, service, Requested::Service, Some(&dossier))
                    }
                    _ => unknown_service(),
                };
                pairs.push(Pair {
                    source: *a,
                    destination: *b,
                    evaluation,
                });
            }
        }
        let evaluations: Vec<Evaluation> = pairs.iter().map(|p| p.evaluation.clone()).collect();
        out.push(Evaluated {
            revision: snapshot.revision(),
            sources: source.iter().map(|(id, _)| *id).collect(),
            destinations: destination.iter().map(|(id, _)| *id).collect(),
            summary: synthesis(&evaluations),
            pairs,
        });
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
    }
}
