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
use crate::port::{Change, StoreError, WriteTx};

/// Runs the global rules for every planned operation, appending violations.
/// `former` gives the domain and value of the prefixes resized, before the changeset.
pub(crate) async fn validate<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
    periods: &[Option<Period>],
    former: &[(Uuid, Prefix)],
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let mut addresses_to_recheck: BTreeSet<Uuid> = BTreeSet::new();
    let mut ranges_to_recheck: BTreeSet<Uuid> = BTreeSet::new();
    for (p, &period) in planned.iter().zip(periods) {
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
            Planned::Modify {
                kind, id, change, ..
            } => {
                check_modified(tx, *kind, *id, change, former, period, violations).await?;
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
            Planned::Close {
                kind: kind @ (ObjectKind::Property | ObjectKind::EnumValue),
                id,
                ..
            } => {
                crate::declarations::check_closure(tx, *kind, *id, period, violations).await?;
            }
            Planned::Anchor { kind, id, .. } => {
                check_anchor(tx, *kind, *id, period, violations).await?;
            }
            Planned::Restore {
                kind,
                id,
                property: None,
                ..
            } => check_restored(tx, *kind, *id, former, period, violations).await?,
            Planned::Close {
                kind: kind @ (ObjectKind::Entity | ObjectKind::Site),
                id,
                ..
            } => check_anchor_closure(tx, *kind, *id, period, violations).await?,
            Planned::Domain { .. }
            | Planned::Close { .. }
            | Planned::Expect { .. }
            | Planned::Declare { .. }
            | Planned::UpdateDeclaration { .. }
            | Planned::EnumValue { .. }
            | Planned::Assign { .. }
            | Planned::Unassign { .. }
            // An assignment restored is checked with the properties.
            | Planned::Restore { .. }
            | Planned::BindExternal { .. }
            | Planned::Rbac(_)
            | Planned::Policy(_)
            // Checked by its module, once the core's rules pass.
            | Planned::Module { .. } => {}
        }
    }
    check_sites(tx, planned, periods, violations).await?;
    check_references(tx, planned, violations).await?;
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
        // REQ-002-49: an anycast network holds anycast addresses only, whichever comes first.
        if object.anycast {
            let (first, last) = (object.value.network(), object.value.last());
            for address in tx
                .addresses_between(object.domain, first, last, period)
                .await?
            {
                violations.extend(objects::check_address_in_network(&address, object));
            }
        }
        // REQ-002-32, 46: public networks are unique across domains, except occurrences of
        // the same network marked anycast everywhere (A13). Containers stay out (A3).
        if public {
            let others = tx
                .public_networks_overlapping(object.domain, object.value, period)
                .await?;
            let shared =
                |o: &objects::PrefixObject| object.anycast && o.anycast && o.value == object.value;
            for other in others.iter().filter(|o| !shared(o)) {
                violations.push(
                    Violation::new(
                        &rules::PUBLIC_OVERLAP,
                        format!(
                            "{} overlaps the public network {} of domain {} (anycast only if the same network is anycast everywhere)",
                            object.value, other.value, other.domain
                        ),
                    )
                    .with_subjects([object.id, other.id]),
                );
            }
            if !others.is_empty() && others.iter().all(shared) {
                let domains: BTreeSet<Uuid> = others.iter().map(|o| o.domain).collect();
                violations.push(
                    Violation::new(
                        &rules::PUBLIC_ANYCAST_SHARED,
                        format!(
                            "{} is anycast in {} other domain(s)",
                            object.value,
                            domains.len()
                        ),
                    )
                    .with_subjects(std::iter::once(object.id).chain(others.iter().map(|o| o.id))),
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

/// A modified object (004-8): every final version over the period passes the local and
/// global rules of a creation (REQ-004-8); a prefix whose nature changes keeps only the
/// content its nature allows (REQ-002-17); a resized prefix has the addresses and ranges
/// of its former and new extent checked again (REQ-002-18).
async fn check_modified<T: WriteTx>(
    tx: &mut T,
    kind: ObjectKind,
    id: Uuid,
    change: &Change,
    former: &[(Uuid, Prefix)],
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    match change {
        Change::Prefix { value, nature, .. } => {
            let checks = PrefixChecks {
                nature: nature.is_some(),
                content: value.is_some(),
            };
            check_prefix_versions(tx, id, checks, former, period, violations).await?;
        }
        Change::Address(_) => check_address_versions(tx, id, period, violations).await?,
        Change::Parent(_) => check_anchor(tx, kind, id, period, violations).await?,
        Change::Range(_) => check_range_versions(tx, id, period, violations).await?,
        // A slug is unique among the current slugs of its kind: the constraint of the
        // table (`core.slug.duplicate`). No rule on the sensitivity flag.
        Change::Slug(_) | Change::Sensitive(_) | Change::Obligation { .. } => {}
    }
    Ok(())
}

/// REQ-002-17: a container holds no address nor range, a network no prefix.
async fn check_nature<T: WriteTx>(
    tx: &mut T,
    object: &objects::PrefixObject,
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let inside: Vec<Uuid> = match object.nature {
        Nature::Container => {
            let mut inside = tx
                .addresses_inside(object.domain, object.value, period)
                .await?;
            inside.extend(
                tx.ranges_inside(object.domain, object.value, period)
                    .await?,
            );
            inside
        }
        Nature::Network => {
            tx.prefixes_inside(object.domain, object.value, period)
                .await?
        }
    };
    if !inside.is_empty() {
        violations.push(
            Violation::new(
                &rules::PREFIX_NOT_EMPTY_FOR_NATURE,
                format!(
                    "{} cannot be a {}: it holds {} object(s) its nature does not allow",
                    object.value,
                    object.nature.as_str(),
                    inside.len()
                ),
            )
            .with_subjects(std::iter::once(object.id).chain(inside)),
        );
    }
    Ok(())
}

/// REQ-002-18: the addresses and ranges of the extents of a resized prefix, checked as at
/// their creation (their network may have changed or gone).
async fn check_content<T: WriteTx>(
    tx: &mut T,
    extents: &BTreeSet<(Uuid, Prefix)>,
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let mut seen: BTreeSet<Uuid> = BTreeSet::new();
    for (domain, value) in extents {
        let (first, last) = (value.network(), value.last());
        for address in tx.addresses_between(*domain, first, last, period).await? {
            if seen.insert(address.id) {
                // Public space does not depend on the network: not checked again.
                check_address(tx, &address, false, period, violations).await?;
            }
        }
        for range in tx.ranges_overlapping(*domain, first, last, period).await? {
            if seen.insert(range.id) {
                check_range(tx, &range, period, violations).await?;
            }
        }
    }
    Ok(())
}

/// An entity or site whose parent was set: the parent exists whenever the child does, and
/// the tree has no cycle (REQ-002-8, 9).
async fn check_anchor<T: WriteTx>(
    tx: &mut T,
    kind: ObjectKind,
    id: Uuid,
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    for parent in tx.missing_parents(kind, id, period).await? {
        violations.push(
            Violation::new(
                &rules::CHANGESET_NO_VERSION,
                format!(
                    "the parent {parent} of {} {id} does not exist over its whole period",
                    kind.name()
                ),
            )
            .with_subjects([id, parent]),
        );
    }
    if tx.in_cycle(kind, id, period).await? {
        violations.push(
            Violation::new(
                &rules::ANCHOR_CYCLE,
                format!("{} {id} would be among its own ancestors", kind.name()),
            )
            .with_subjects([id]),
        );
    }
    Ok(())
}

/// REQ-003-10: a closed object leaves no reference assignment without its target, on the
/// final state (a reference removed in the same changeset no longer counts).
async fn check_references<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    for p in planned {
        if let Planned::Close { kind, id, .. } = p
            && (kind.is_addressing() || matches!(kind, ObjectKind::Module(_)))
        {
            let referring = tx.dangling_references(*id).await?;
            if !referring.is_empty() {
                violations.push(
                    Violation::new(
                        &rules::PROP_DANGLING_REFERENCE,
                        format!(
                            "{id} is still referenced by {} object(s) after its closure",
                            referring.len()
                        ),
                    )
                    .with_subjects(std::iter::once(*id).chain(referring)),
                );
            }
        }
    }
    Ok(())
}

/// REQ-002-20: no site on a container, whichever of the two comes first.
async fn check_sites<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
    periods: &[Option<Period>],
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let prefixes: Vec<(Uuid, Option<Period>)> = planned
        .iter()
        .zip(periods)
        .filter_map(|(p, &period)| match p {
            Planned::Assign {
                object,
                kind: ObjectKind::Prefix,
                ..
            } => Some((*object, period)),
            Planned::Modify {
                kind: ObjectKind::Prefix,
                id,
                change: Change::Prefix {
                    nature: Some(_), ..
                },
                ..
            } => Some((*id, period)),
            _ => None,
        })
        .collect();
    for (prefix, period) in prefixes {
        for prefix in tx
            .containers_carrying("core.site", &[prefix], period)
            .await?
        {
            violations.push(
                Violation::new(
                    &rules::PREFIX_SITE_ON_CONTAINER,
                    format!("the container {prefix} cannot be attached to a site"),
                )
                .with_subjects([prefix]),
            );
        }
    }
    Ok(())
}

/// REQ-002-10: an entity or a site leaves no child nor reference without it.
async fn check_anchor_closure<T: WriteTx>(
    tx: &mut T,
    kind: ObjectKind,
    id: Uuid,
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let mut users = tx.children(kind, id, period).await?;
    users.extend(tx.dangling_references(id).await?);
    if !users.is_empty() {
        violations.push(
            Violation::new(
                &rules::ANCHOR_IN_USE,
                format!(
                    "the {} {id} is still used by {} object(s) after its closure",
                    kind.name(),
                    users.len()
                ),
            )
            .with_subjects(std::iter::once(id).chain(users)),
        );
    }
    Ok(())
}

/// What a prefix modification needs checked besides the rules of a creation.
#[derive(Debug, Clone, Copy)]
struct PrefixChecks {
    /// Its content fits its nature (REQ-002-17).
    nature: bool,
    /// The addresses and ranges of its former and new extents (REQ-002-18).
    content: bool,
}

async fn check_prefix_versions<T: WriteTx>(
    tx: &mut T,
    id: Uuid,
    checks: PrefixChecks,
    former: &[(Uuid, Prefix)],
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let versions = tx.prefix_versions(id, period).await?;
    let mut extents: BTreeSet<(Uuid, Prefix)> = BTreeSet::new();
    for (object, public) in &versions {
        violations.extend(objects::check_prefix(object));
        check_prefix(tx, object, *public, period, violations).await?;
        if checks.nature {
            check_nature(tx, object, period, violations).await?;
        }
        extents.insert((object.domain, object.value));
    }
    if checks.content {
        extents.extend(former.iter().copied());
        check_content(tx, &extents, period, violations).await?;
    }
    Ok(())
}

async fn check_address_versions<T: WriteTx>(
    tx: &mut T,
    id: Uuid,
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    for (object, public) in tx.address_versions(id, period).await? {
        violations.extend(objects::check_address(&object));
        check_address(tx, &object, public, period, violations).await?;
    }
    Ok(())
}

async fn check_range_versions<T: WriteTx>(
    tx: &mut T,
    id: Uuid,
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    for object in tx.range_versions(id, period).await? {
        violations.extend(objects::check_range(&object));
        check_range(tx, &object, period, violations).await?;
    }
    Ok(())
}

/// An object restored as before a changeset (004-20): checked as a modification of
/// everything it carries.
async fn check_restored<T: WriteTx>(
    tx: &mut T,
    kind: ObjectKind,
    id: Uuid,
    former: &[(Uuid, Prefix)],
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let all = PrefixChecks {
        nature: true,
        content: true,
    };
    match kind {
        ObjectKind::Prefix => check_prefix_versions(tx, id, all, former, period, violations).await,
        ObjectKind::Address => check_address_versions(tx, id, period, violations).await,
        ObjectKind::Range => check_range_versions(tx, id, period, violations).await,
        ObjectKind::Entity | ObjectKind::Site => {
            check_anchor(tx, kind, id, period, violations).await
        }
        // An object of a module is checked by its module (`ModuleHandler::check`).
        ObjectKind::Domain
        | ObjectKind::Property
        | ObjectKind::EnumValue
        | ObjectKind::Module(_) => Ok(()),
    }
}
