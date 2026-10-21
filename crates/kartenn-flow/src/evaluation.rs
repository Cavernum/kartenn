//! The evaluation of a flow against the policy (spec 010), in the transaction of a changeset
//! (REQ-011-8, 11, 21; design 011 « Réévaluation »): its endpoints developed into their
//! addresses, the values of the axes read on the state the changeset leaves, a verdict per
//! pair of addresses for its service, their summary.
//!
//! It reads the transaction, not the pool: an address created, classified or attached in
//! the same changeset counts. The policy is loaded at each call, never from the cache of
//! the queries, since a changeset may raise a revision it then rolls back (a simulation).

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::period::Instant;
use kartenn_machines::read::{addresses_of_interface, addresses_of_machine, instant};
use kartenn_policy::evaluate::{
    Dossier, Endpoint as Side, Evaluation, Requested, Synthesis, evaluate, synthesis,
    unknown_service,
};
use kartenn_policy::model::{
    AxisKey, AxisValue, Carrier, ConditionKind, DossierField, Key, SignalKind,
};
use kartenn_policy::read;
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

/// The points an endpoint develops into at `at`: an address or a prefix, itself; an
/// interface or a machine, its addresses (port of reading of the 009). With the width of
/// each and whether they are addresses.
async fn develop(
    conn: &mut PgConnection,
    kind: EndpointKind,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<(Vec<(Uuid, u128)>, bool), StoreError> {
    let t: Instant = instant(at);
    Ok(match kind {
        EndpointKind::Address => (vec![(id, 1)], true),
        EndpointKind::Prefix => {
            let prefix: Option<String> = sqlx::query_scalar(
                "SELECT prefix::text FROM public.prefix WHERE id = $1 AND valid_period @> $2",
            )
            .bind(id)
            .bind(at)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
            (vec![(id, prefix.as_deref().map_or(1, width_of))], false)
        }
        EndpointKind::Interface => (
            addresses_of_interface(conn, id, t, None)
                .await?
                .into_iter()
                .map(|a| (a.address, 1))
                .collect(),
            true,
        ),
        EndpointKind::Machine => {
            let mut seen = BTreeSet::new();
            (
                addresses_of_machine(conn, id, t, None)
                    .await?
                    .into_iter()
                    .filter(|a| seen.insert(a.address))
                    .map(|a| (a.address, 1))
                    .collect(),
                true,
            )
        }
        EndpointKind::Range => (Vec::new(), false),
    })
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

/// The file of a flow (REQ-010-38): the fields it carries, and the nature of its data on
/// the axes of the values it names.
async fn dossier(
    conn: &mut PgConnection,
    flow: &Flow,
    at: OffsetDateTime,
) -> Result<Dossier, StoreError> {
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
        let named: Vec<(String, String)> = sqlx::query_as(
            "SELECT property_key, value_key FROM policy.policy_value
              WHERE id = ANY($1) AND valid_period @> $2",
        )
        .bind(&flow.data_nature)
        .bind(at)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
        for (axis, value) in named {
            if let Ok(axis) = AxisKey::parse(&axis) {
                values
                    .entry(axis)
                    .or_default()
                    .insert(AxisValue::new(value));
            }
        }
    }
    Ok(Dossier { fields, values })
}

/// Evaluates `flow` at `at` on the state of the transaction (REQ-011-8, 11, 21).
///
/// # Errors
///
/// A failure of the database.
#[allow(clippy::too_many_lines)] // the steps of design 010 « Évaluation », in their order
pub async fn evaluate_flow(
    conn: &mut PgConnection,
    flow: &Flow,
    at: OffsetDateTime,
) -> Result<Evaluated, StoreError> {
    let revision = read::revision(conn).await?;
    let parts = read::parts(conn, at, None, revision).await?;
    let axes = axes_read(&parts);
    let machine_axes: Vec<String> = parts
        .axes
        .iter()
        .filter(|axis| axis.carrier == Carrier::Machine)
        .map(|axis| axis.key.to_string())
        .collect();
    let snapshot = PolicySnapshot::new(parts);
    let service: Option<String> = sqlx::query_scalar(
        "SELECT key FROM policy.policy_service WHERE id = $1 AND valid_period @> $2",
    )
    .bind(flow.service)
    .bind(at)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let service = service.and_then(|k| Key::new(&k).ok());
    let (source, source_addresses) = develop(conn, flow.source.kind, flow.source.id, at).await?;
    let (destination, destination_addresses) =
        develop(conn, flow.destination.kind, flow.destination.id, at).await?;
    let keys: Vec<String> = axes.iter().map(ToString::to_string).collect();
    let mut widths: BTreeMap<Uuid, u128> = BTreeMap::new();
    let mut addresses: BTreeSet<Uuid> = BTreeSet::new();
    for (points, are_addresses) in [
        (&source, source_addresses),
        (&destination, destination_addresses),
    ] {
        for (id, width) in points {
            widths.insert(*id, *width);
            if are_addresses {
                addresses.insert(*id);
            }
        }
    }
    let objects: Vec<Uuid> = widths.keys().copied().collect();
    let tree = tree_values(conn, &objects, &keys, at).await?;
    // An axis carried by the machine: the union of the values of the machines holding the
    // address, else the value of the tree (S27).
    let mut machine: BTreeMap<(Uuid, String), BTreeSet<AxisValue>> = BTreeMap::new();
    if !machine_axes.is_empty() && !addresses.is_empty() {
        let list: Vec<Uuid> = addresses.iter().copied().collect();
        let names: Vec<&str> = machine_axes.iter().map(String::as_str).collect();
        for (address, machines) in kartenn_machines::read::machine_values_of_addresses(
            conn,
            &list,
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
        for axis in &axes {
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
    let dossier = dossier(conn, flow, at).await?;
    let mut pairs = Vec::new();
    for (a, _) in &source {
        for (b, _) in &destination {
            let evaluation = match &service {
                Some(service) => evaluate(
                    &snapshot,
                    &side(*a),
                    &side(*b),
                    service,
                    Requested::Service,
                    Some(&dossier),
                ),
                None => unknown_service(),
            };
            pairs.push(Pair {
                source: *a,
                destination: *b,
                evaluation,
            });
        }
    }
    let evaluations: Vec<Evaluation> = pairs.iter().map(|p| p.evaluation.clone()).collect();
    Ok(Evaluated {
        revision: snapshot.revision(),
        sources: source.into_iter().map(|(id, _)| id).collect(),
        destinations: destination.into_iter().map(|(id, _)| id).collect(),
        summary: synthesis(&evaluations),
        pairs,
    })
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
