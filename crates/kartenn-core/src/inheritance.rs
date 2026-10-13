//! Inheritance trees and the reference resolver (spec 003, REQ-003-11 to 14).
//!
//! The naive resolver walks the ancestors of an object at one instant. It is the oracle
//! against which the materialised resolution of the storage adapter is tested
//! (003/design, test strategy): simple enough to be obviously right.

use std::collections::HashMap;

use uuid::Uuid;

use crate::objects::{Address, IpRange, Nature, PrefixObject};
use crate::values::Value;

/// A tree along which heritable properties propagate (003/P3): one parent per node.
pub trait InheritanceTree {
    /// Ancestors of `object`, nearest first, up to the root; empty for a root or an
    /// object unknown to this tree.
    fn ancestors(&self, object: Uuid) -> Vec<Uuid>;
}

/// A tree with stored parents: entities or sites.
#[derive(Debug, Default, Clone)]
pub struct ParentTree {
    parents: HashMap<Uuid, Uuid>,
}

impl ParentTree {
    /// A tree from `(child, parent)` pairs. Cycles are refused upstream
    /// (`core.anchor.cycle`); a cycle here stops the walk after one turn.
    #[must_use]
    pub fn new(pairs: impl IntoIterator<Item = (Uuid, Uuid)>) -> Self {
        Self {
            parents: pairs.into_iter().collect(),
        }
    }
}

impl InheritanceTree for ParentTree {
    fn ancestors(&self, object: Uuid) -> Vec<Uuid> {
        let mut out = Vec::new();
        let mut current = object;
        while let Some(&parent) = self.parents.get(&current) {
            if parent == object || out.contains(&parent) {
                break;
            }
            out.push(parent);
            current = parent;
        }
        out
    }
}

/// The addressing tree of one instant (D34): the parent of a prefix is the smallest
/// prefix of its domain that strictly contains it; ranges and addresses hang off their
/// network, never off a range; the domain is the root.
#[derive(Debug, Default, Clone)]
pub struct AddressingTree {
    prefixes: Vec<PrefixObject>,
    leaves: HashMap<Uuid, (Uuid, crate::addr::Prefix)>,
    prefix_index: HashMap<Uuid, usize>,
}

impl AddressingTree {
    /// Builds the tree from the objects valid at one instant.
    #[must_use]
    pub fn new(prefixes: Vec<PrefixObject>, ranges: &[IpRange], addresses: &[Address]) -> Self {
        let prefix_index = prefixes
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id, i))
            .collect();
        let mut leaves = HashMap::new();
        for r in ranges {
            leaves.insert(r.id, (r.domain, crate::addr::Prefix::host(r.first)));
        }
        for a in addresses {
            leaves.insert(a.id, (a.domain, crate::addr::Prefix::host(a.value)));
        }
        Self {
            prefixes,
            leaves,
            prefix_index,
        }
    }

    /// Prefixes of `domain` containing `value` (strictly if `strict`), nearest first.
    fn containing(
        &self,
        domain: Uuid,
        value: crate::addr::Prefix,
        strict: bool,
    ) -> Vec<&PrefixObject> {
        let mut found: Vec<&PrefixObject> = self
            .prefixes
            .iter()
            .filter(|p| {
                p.domain == domain
                    && if strict {
                        p.value.strictly_contains(value)
                    } else {
                        p.value.contains(value)
                    }
            })
            .collect();
        found.sort_by_key(|p| std::cmp::Reverse(p.value.prefix_len()));
        found
    }
}

impl InheritanceTree for AddressingTree {
    fn ancestors(&self, object: Uuid) -> Vec<Uuid> {
        let chain: Vec<&PrefixObject>;
        let domain;
        if let Some(&i) = self.prefix_index.get(&object) {
            let p = &self.prefixes[i];
            domain = p.domain;
            chain = self.containing(p.domain, p.value, true);
        } else if let Some(&(d, value)) = self.leaves.get(&object) {
            domain = d;
            // A range or an address starts at its network: skip any prefix nested below the
            // network (none exist, since a network is a leaf of the prefix tree).
            let all = self.containing(d, value, false);
            let start = all
                .iter()
                .position(|p| p.nature == Nature::Network)
                .unwrap_or(0);
            chain = all[start..].to_vec();
        } else {
            return Vec::new();
        }
        chain
            .into_iter()
            .map(|p| p.id)
            .chain(std::iter::once(domain))
            .collect()
    }
}

/// An assignment as seen by the resolver: a value or an explicit "no value" (REQ-003-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Assigned {
    /// A value.
    Value(Value),
    /// Explicit "no value", which replaces the inherited value like any assignment.
    Cleared,
}

/// Explained resolution of one property for one object (REQ-003-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// Resolved value; `None` when absent or cleared.
    pub value: Option<Value>,
    /// Object carrying the applicable assignment (the object itself or an ancestor).
    pub origin: Option<Uuid>,
    /// Objects walked from the object to the origin, both included.
    pub chain: Vec<Uuid>,
}

/// Reference resolution (REQ-003-11 to 13): the object's own assignment, otherwise, if
/// the property is heritable, the assignment of the nearest ancestor that has one; an
/// assignment replaces the inherited value entirely, sets included (D26).
#[must_use]
pub fn resolve<S: std::hash::BuildHasher>(
    tree: &dyn InheritanceTree,
    assignments: &HashMap<Uuid, Assigned, S>,
    object: Uuid,
    heritable: bool,
) -> Resolution {
    let candidates: Vec<Uuid> = if heritable {
        std::iter::once(object)
            .chain(tree.ancestors(object))
            .collect()
    } else {
        vec![object]
    };
    for (depth, candidate) in candidates.iter().enumerate() {
        if let Some(assigned) = assignments.get(candidate) {
            let value = match assigned {
                Assigned::Value(v) => Some(v.clone()),
                Assigned::Cleared => None,
            };
            return Resolution {
                value,
                origin: Some(*candidate),
                chain: candidates[..=depth].to_vec(),
            };
        }
    }
    Resolution {
        value: None,
        origin: None,
        chain: vec![object],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addr::{Addr, Prefix};
    use crate::family::ALL;
    use crate::objects::{AddressUsage, RangeUsage};
    use crate::values::Scalar;
    use std::collections::BTreeSet;

    fn pfx(domain: Uuid, t: &str, nature: Nature) -> PrefixObject {
        PrefixObject {
            id: Uuid::now_v7(),
            domain,
            value: Prefix::parse_any(&ALL, t).unwrap(),
            nature,
            point_to_point: false,
        }
    }

    fn set(items: &[&str]) -> Assigned {
        let scalars = items
            .iter()
            .map(|s| Scalar::Text((*s).to_owned()))
            .collect::<BTreeSet<_>>();
        Assigned::Value(Value::Set(scalars))
    }

    struct World {
        domain: Uuid,
        root: PrefixObject,
        net: PrefixObject,
        range: IpRange,
        address: Address,
        tree: AddressingTree,
    }

    fn world() -> World {
        let domain = Uuid::now_v7();
        let root = pfx(domain, "10.0.0.0/8", Nature::Container);
        let mid = pfx(domain, "10.1.0.0/16", Nature::Container);
        let net = pfx(domain, "10.1.2.0/24", Nature::Network);
        let other_domain_net = pfx(Uuid::now_v7(), "10.1.2.0/24", Nature::Network);
        let addr = Addr::parse_any(&ALL, "10.1.2.150").unwrap();
        let range = IpRange {
            id: Uuid::now_v7(),
            domain,
            first: Addr::parse_any(&ALL, "10.1.2.100").unwrap(),
            last: Addr::parse_any(&ALL, "10.1.2.199").unwrap(),
            usage: RangeUsage::Dhcp,
        };
        let address = Address {
            id: Uuid::now_v7(),
            domain,
            value: addr,
            usage: AddressUsage::Reserved,
        };
        let tree = AddressingTree::new(
            vec![root.clone(), mid, net.clone(), other_domain_net],
            std::slice::from_ref(&range),
            std::slice::from_ref(&address),
        );
        World {
            domain,
            root,
            net,
            range,
            address,
            tree,
        }
    }

    #[test]
    fn addressing_ancestors_follow_cidr_inclusion_within_the_domain() {
        let w = world();
        let chain = w.tree.ancestors(w.address.id);
        assert_eq!(chain.len(), 4, "network, /16, /8, domain");
        assert_eq!(chain[0], w.net.id);
        assert_eq!(chain[2], w.root.id);
        assert_eq!(chain[3], w.domain);
        assert_eq!(w.tree.ancestors(w.root.id), vec![w.domain]);
        assert_eq!(
            w.tree.ancestors(w.range.id)[0],
            w.net.id,
            "a range hangs off its network"
        );
    }

    #[test]
    fn inheritance_replacement_and_explanation() {
        // REQ-003-11, 12, 14 acceptance.
        let w = world();
        let mut a = HashMap::new();
        a.insert(w.root.id, set(&["prod", "global"]));
        let r = resolve(&w.tree, &a, w.address.id, true);
        assert_eq!(
            r.value,
            Some(match set(&["global", "prod"]) {
                Assigned::Value(v) => v,
                Assigned::Cleared => unreachable!(),
            })
        );
        assert_eq!(r.origin, Some(w.root.id));
        assert_eq!(r.chain.first(), Some(&w.address.id));
        assert_eq!(r.chain.last(), Some(&w.root.id));
        a.insert(w.net.id, set(&["rec"]));
        let r = resolve(&w.tree, &a, w.address.id, true);
        assert_eq!(
            r.origin,
            Some(w.net.id),
            "the network replaces the inherited set"
        );
        assert_eq!(r.chain, vec![w.address.id, w.net.id]);
    }

    #[test]
    fn explicit_clearing_and_non_heritable_properties() {
        // REQ-003-13 acceptance.
        let w = world();
        let mut a = HashMap::new();
        a.insert(w.root.id, set(&["prod"]));
        a.insert(w.net.id, Assigned::Cleared);
        let r = resolve(&w.tree, &a, w.address.id, true);
        assert_eq!((r.value, r.origin), (None, Some(w.net.id)));
        let r = resolve(&w.tree, &a, w.address.id, false);
        assert_eq!(
            (r.value, r.origin),
            (None, None),
            "not heritable: only own assignment"
        );
    }

    #[test]
    fn an_address_never_inherits_from_a_range() {
        let w = world();
        let mut a = HashMap::new();
        a.insert(w.range.id, set(&["dhcp-only"]));
        assert_eq!(resolve(&w.tree, &a, w.address.id, true).origin, None);
    }

    #[test]
    fn parent_trees() {
        let (group, sub, team) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let tree = ParentTree::new([(sub, group), (team, sub)]);
        assert_eq!(tree.ancestors(team), vec![sub, group]);
        assert!(tree.ancestors(group).is_empty());
        let looped = ParentTree::new([(group, sub), (sub, group)]);
        assert_eq!(looped.ancestors(group), vec![sub], "a cycle stops the walk");
    }
}
