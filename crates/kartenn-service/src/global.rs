//! Global addressing rules, checked on the final state inside the changeset transaction
//! (spec 002, REQ-002-16, 21 to 23, 25, 28, 29, 32, 46; tasks 002-18 to 21).

use std::collections::BTreeSet;

use kartenn_core::addr::Prefix;
use kartenn_core::objects::{self, AddressUsage, Nature};
use kartenn_core::period::Period;
use kartenn_core::rules::{self, Violation};
use uuid::Uuid;

use crate::operations::ObjectKind;
use crate::pipeline::Planned;
use crate::port::{StoreError, WriteTx};

/// Runs the global rules for every planned operation, appending violations.
pub(crate) async fn validate<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let mut addresses_to_recheck: BTreeSet<Uuid> = BTreeSet::new();
    let mut ranges_to_recheck: BTreeSet<Uuid> = BTreeSet::new();
    for p in planned {
        match p {
            Planned::Prefix { object, public } => {
                check_prefix(tx, object, *public, period, violations).await?;
                // A new network may leave addresses inside a former network uncovered only
                // through closure; creation never removes coverage.
            }
            Planned::Range { object, .. } => {
                check_range(tx, object, period, violations).await?;
            }
            Planned::Address { object, public } => {
                check_address(tx, object, *public, period, violations).await?;
            }
            Planned::Close {
                kind: ObjectKind::Prefix,
                id,
                ..
            } => {
                if let Some((domain, value)) = tx.prefix_of(*id).await? {
                    addresses_to_recheck.extend(tx.addresses_inside(domain, value, None).await?);
                    ranges_to_recheck.extend(tx.ranges_inside(domain, value, None).await?);
                }
            }
            Planned::Domain { .. } | Planned::Close { .. } => {}
        }
    }
    for address in addresses_to_recheck {
        if !tx.address_covered(address).await? {
            violations.push(
                Violation::new(
                    &rules::ADDRESS_OUTSIDE_NETWORK,
                    format!("address {address} would no longer be in a network"),
                )
                .with_subjects([address]),
            );
        }
    }
    for range in ranges_to_recheck {
        if !tx.range_covered(range).await? {
            violations.push(
                Violation::new(
                    &rules::RANGE_OUTSIDE_NETWORK,
                    format!("range {range} would no longer be in a network"),
                )
                .with_subjects([range]),
            );
        }
    }
    Ok(())
}

async fn check_prefix<T: WriteTx>(
    tx: &mut T,
    object: &objects::PrefixObject,
    public: bool,
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    // REQ-002-16: nothing inside a network, no prefix inside a network.
    for network in tx
        .networks_containing(object.domain, object.value, false, period)
        .await?
    {
        violations.push(
            Violation::new(
                &rules::PREFIX_NETWORK_NOT_LEAF,
                format!(
                    "{} would be inside the network {}",
                    object.value, network.value
                ),
            )
            .with_subjects([object.id, network.id]),
        );
    }
    if object.nature == Nature::Network {
        let inside = tx
            .prefixes_inside(object.domain, object.value, period)
            .await?;
        if !inside.is_empty() {
            violations.push(
                Violation::new(
                    &rules::PREFIX_NETWORK_NOT_LEAF,
                    format!(
                        "the network {} would contain {} prefix(es)",
                        object.value,
                        inside.len()
                    ),
                )
                .with_subjects(std::iter::once(object.id).chain(inside)),
            );
        }
        // REQ-002-32: public networks are unique across domains.
        if public {
            for (other, other_domain) in tx
                .public_networks_overlapping(object.domain, object.value, period)
                .await?
            {
                violations.push(
                    Violation::new(
                        &rules::PUBLIC_OVERLAP,
                        format!(
                            "{} overlaps a public network of domain {other_domain}",
                            object.value
                        ),
                    )
                    .with_subjects([object.id, other]),
                );
            }
        }
    }
    Ok(())
}

/// A range lies in a network of its domain, avoids its reserved positions and respects
/// the addresses it covers (REQ-002-21, 23, 28). Overlap between ranges is enforced by the
/// database (REQ-002-22). Public space needs no check of its own: the network holding the
/// range is already unique across domains (REQ-002-32).
async fn check_range<T: WriteTx>(
    tx: &mut T,
    object: &objects::IpRange,
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    if !tx.range_covered(object.id).await? {
        violations.push(
            Violation::new(
                &rules::RANGE_OUTSIDE_NETWORK,
                format!(
                    "{} – {} is not inside a single network of its domain",
                    object.first, object.last
                ),
            )
            .with_subjects([object.id]),
        );
    }
    for network in tx
        .networks_containing(object.domain, Prefix::host(object.first), true, period)
        .await?
    {
        if network.value.contains_addr(object.last) {
            violations.extend(objects::check_range_in_network(object, &network));
        }
    }
    for address in tx
        .addresses_between(object.domain, object.first, object.last, period)
        .await?
    {
        violations.extend(objects::check_address_in_range(&address, object));
    }
    Ok(())
}

async fn check_address<T: WriteTx>(
    tx: &mut T,
    object: &objects::Address,
    public: bool,
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    // REQ-002-25: in a network over its whole period.
    if !tx.address_covered(object.id).await? {
        violations.push(
            Violation::new(
                &rules::ADDRESS_OUTSIDE_NETWORK,
                format!("{} is not inside a network of its domain", object.value),
            )
            .with_subjects([object.id]),
        );
    }
    // REQ-002-29: positional reservations against its network(s).
    for network in tx
        .networks_containing(object.domain, Prefix::host(object.value), true, period)
        .await?
    {
        violations.extend(objects::check_address_in_network(object, &network));
    }
    // REQ-002-28: usage of the range it falls in.
    for range in tx
        .ranges_overlapping(object.domain, object.value, object.value, period)
        .await?
    {
        violations.extend(objects::check_address_in_range(object, &range));
    }
    // REQ-002-32, 46: public space across domains, anycast exception.
    if public {
        let others = tx
            .public_addresses_equal(object.domain, object.value, period)
            .await?;
        let anycast_here = object.usage == AddressUsage::Anycast;
        let clash: Vec<Uuid> = others
            .iter()
            .filter(|(_, usage)| !anycast_here || *usage != AddressUsage::Anycast)
            .map(|(id, _)| *id)
            .collect();
        if !clash.is_empty() {
            violations.push(
                Violation::new(
                    &rules::PUBLIC_OVERLAP,
                    format!(
                        "{} is already used in another domain (anycast only if anycast everywhere)",
                        object.value
                    ),
                )
                .with_subjects(std::iter::once(object.id).chain(clash)),
            );
        } else if anycast_here && !others.is_empty() {
            violations.push(
                Violation::new(
                    &rules::PUBLIC_ANYCAST_SHARED,
                    format!(
                        "{} is anycast in {} other domain(s)",
                        object.value,
                        others.len()
                    ),
                )
                .with_subjects(std::iter::once(object.id).chain(others.iter().map(|(id, _)| *id))),
            );
        }
    }
    Ok(())
}
