//! Capacity, occupancy and next free value, computed from sorted lists (REQ-002-43, 44).
//!
//! The storage adapter feeds these functions with the direct content of a prefix; the
//! same functions serve as the reference for the SQL implementation (task 002-24).

use crate::addr::{Addr, Prefix};
use crate::objects::{IpRange, PrefixObject, RangeUsage, reserved_in_network};

/// Number of assignable addresses of a network (REQ-002-43): its size minus the
/// positions that are never assigned (network and broadcast for IPv4 up to /30, zero
/// interface identifier for IPv6 except /128 and point-to-point /127).
///
/// For an IPv6 `/0` network (2¹²⁸ addresses, not representable) the size saturates at
/// `u128::MAX` and the result is one below the exact value; no real network is that large.
#[must_use]
pub fn capacity(network: &PrefixObject) -> u128 {
    let net = network.value;
    let size = net.size().unwrap_or(u128::MAX);
    let reserved = [net.network(), net.last()]
        .into_iter()
        .enumerate()
        .filter(|&(i, a)| (i == 0 || a != net.network()) && reserved_in_network(a, network))
        .count();
    size.saturating_sub(u128::try_from(reserved).unwrap_or(2))
}

/// Occupancy of a prefix: used addresses (network) or covered addresses (container),
/// against the capacity or size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Occupancy {
    /// Addresses in use (network) or covered by direct children (container).
    pub used: u128,
    /// Assignable addresses (network) or size (container, saturating for an IPv6 `/0`).
    pub total: u128,
}

fn range_len(r: &IpRange) -> u128 {
    r.last.bits() - r.first.bits() + 1
}

/// Occupancy of a network: declared addresses plus DHCP and reserved ranges counted in
/// full, without counting twice an address inside such a range (REQ-002-43).
///
/// `ranges` and `addresses` are the content of the network, in any order.
#[must_use]
pub fn network_occupancy(
    network: &PrefixObject,
    ranges: &[IpRange],
    addresses: &[Addr],
) -> Occupancy {
    let blocking: Vec<&IpRange> = ranges
        .iter()
        .filter(|r| r.usage != RangeUsage::Static)
        .collect();
    let in_blocking = |a: Addr| blocking.iter().any(|r| r.first <= a && a <= r.last);
    let ranges_used: u128 = blocking.iter().map(|r| range_len(r)).sum();
    let addresses_used = addresses.iter().filter(|&&a| !in_blocking(a)).count();
    Occupancy {
        used: ranges_used + u128::try_from(addresses_used).unwrap_or(u128::MAX),
        total: capacity(network),
    }
}

/// Occupancy of a container: share of its space covered by its direct children
/// (REQ-002-43). Direct children never overlap (D34).
#[must_use]
pub fn container_occupancy(container: Prefix, children: &[Prefix]) -> Occupancy {
    let used = children.iter().fold(0u128, |acc, c| {
        acc.saturating_add(c.size().unwrap_or(u128::MAX))
    });
    Occupancy {
        used,
        total: container.size().unwrap_or(u128::MAX),
    }
}

/// First assignable address of `network` that is neither declared, nor inside a DHCP or
/// reserved range, nor a reserved position; `None` if the network is full (REQ-002-44).
#[must_use]
pub fn next_free_address(
    network: &PrefixObject,
    ranges: &[IpRange],
    addresses: &[Addr],
) -> Option<Addr> {
    let net = network.value;
    // Occupied intervals, as inclusive bit bounds.
    let mut taken: Vec<(u128, u128)> = ranges
        .iter()
        .filter(|r| r.usage != RangeUsage::Static)
        .map(|r| (r.first.bits(), r.last.bits()))
        .chain(addresses.iter().map(|a| (a.bits(), a.bits())))
        .chain(
            [net.network(), net.last()]
                .into_iter()
                .filter(|&a| reserved_in_network(a, network))
                .map(|a| (a.bits(), a.bits())),
        )
        .collect();
    taken.sort_unstable();
    let mut candidate = net.network().bits();
    for (first, last) in taken {
        if candidate < first {
            break;
        }
        candidate = candidate.max(last.checked_add(1)?);
    }
    (candidate <= net.last().bits())
        .then(|| Addr::new(net.family(), candidate).ok())
        .flatten()
}

/// First free prefix of length `len` inside `container` that overlaps none of its
/// `children`; `None` if there is none or `len` is shorter than the container
/// (REQ-002-44).
#[must_use]
pub fn next_free_prefix(container: Prefix, children: &[Prefix], len: u8) -> Option<Prefix> {
    if len < container.prefix_len() || len > container.family().width() {
        return None;
    }
    let step_bits = container.family().width() - len;
    let mut sorted: Vec<Prefix> = children
        .iter()
        .copied()
        .filter(|c| container.contains(*c))
        .collect();
    sorted.sort_unstable();
    let mut candidate = container.network().bits();
    let end = container.last().bits();
    for child in sorted {
        let candidate_last = candidate | ones(step_bits);
        if candidate_last < child.network().bits() {
            break;
        }
        if child.last().bits() >= candidate {
            // Next aligned start after this child.
            let after = child.last().bits().checked_add(1)?;
            candidate = align_up(after, step_bits)?;
        }
    }
    if candidate > end {
        return None;
    }
    let addr = Addr::new(container.family(), candidate).ok()?;
    Prefix::new(addr, len).ok()
}

const fn ones(n: u8) -> u128 {
    if n >= 128 {
        u128::MAX
    } else {
        (1u128 << n) - 1
    }
}

fn align_up(bits: u128, step_bits: u8) -> Option<u128> {
    let mask = ones(step_bits);
    if bits & mask == 0 {
        Some(bits)
    } else {
        (bits | mask).checked_add(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::family::ALL;
    use crate::objects::Nature;
    use proptest::prelude::*;
    use uuid::Uuid;

    fn addr(t: &str) -> Addr {
        Addr::parse_any(&ALL, t).unwrap()
    }

    fn pfx(t: &str) -> Prefix {
        Prefix::parse_any(&ALL, t).unwrap()
    }

    fn net(t: &str, p2p: bool) -> PrefixObject {
        PrefixObject {
            id: Uuid::nil(),
            domain: Uuid::nil(),
            value: pfx(t),
            nature: Nature::Network,
            point_to_point: p2p,
            anycast: false,
        }
    }

    fn range(first: &str, last: &str, usage: RangeUsage) -> IpRange {
        IpRange {
            id: Uuid::nil(),
            domain: Uuid::nil(),
            first: addr(first),
            last: addr(last),
            usage,
        }
    }

    #[test]
    fn capacities() {
        // REQ-002-43 acceptance.
        assert_eq!(capacity(&net("10.1.2.0/24", false)), 254);
        assert_eq!(capacity(&net("192.0.2.0/31", true)), 2);
        assert_eq!(capacity(&net("192.0.2.0/31", false)), 2);
        assert_eq!(capacity(&net("10.9.9.9/32", false)), 1);
        assert_eq!(capacity(&net("10.0.0.0/30", false)), 2);
        assert_eq!(capacity(&net("2001:db8:1::/64", false)), (1u128 << 64) - 1);
        assert_eq!(capacity(&net("2001:db8:1::/127", true)), 2);
        assert_eq!(capacity(&net("2001:db8:1::/127", false)), 1);
        assert_eq!(capacity(&net("2001:db8:1::1/128", false)), 1);
        assert_eq!(capacity(&net("::/0", false)), u128::MAX - 1);
    }

    #[test]
    fn network_occupancy_counts_blocking_ranges_once() {
        let n = net("10.1.2.0/24", false);
        let ranges = [
            range("10.1.2.100", "10.1.2.199", RangeUsage::Dhcp),
            range("10.1.2.10", "10.1.2.19", RangeUsage::Static),
        ];
        let addresses = [addr("10.1.2.1"), addr("10.1.2.12"), addr("10.1.2.150")];
        // 100 (DHCP) + .1 + .12 (static range does not block); .150 is inside DHCP.
        assert_eq!(
            network_occupancy(&n, &ranges, &addresses),
            Occupancy {
                used: 102,
                total: 254
            }
        );
    }

    #[test]
    fn container_occupancy_covers_children() {
        // REQ-002-43 acceptance: a /16 with two /24.
        let o = container_occupancy(
            pfx("10.1.0.0/16"),
            &[pfx("10.1.2.0/24"), pfx("10.1.7.0/24")],
        );
        assert_eq!(
            o,
            Occupancy {
                used: 512,
                total: 65_536
            }
        );
        assert_eq!(o.used * 256 / o.total, 2, "2/256 covered");
    }

    #[test]
    fn next_free_address_skips_reserved_and_taken() {
        let n = net("10.1.2.0/24", false);
        let ranges = [range("10.1.2.1", "10.1.2.9", RangeUsage::Reserved)];
        assert_eq!(
            next_free_address(&n, &ranges, &[addr("10.1.2.10")]),
            Some(addr("10.1.2.11"))
        );
        assert_eq!(next_free_address(&n, &[], &[]), Some(addr("10.1.2.1")));
        let full = net("192.0.2.0/31", true);
        assert_eq!(
            next_free_address(&full, &[], &[addr("192.0.2.0"), addr("192.0.2.1")]),
            None
        );
        assert_eq!(
            next_free_address(&net("2001:db8::/64", false), &[], &[]),
            Some(addr("2001:db8::1"))
        );
    }

    #[test]
    fn next_free_prefix_respects_alignment_and_children() {
        let c = pfx("10.1.0.0/16");
        assert_eq!(next_free_prefix(c, &[], 24), Some(pfx("10.1.0.0/24")));
        assert_eq!(
            next_free_prefix(c, &[pfx("10.1.0.0/24")], 24),
            Some(pfx("10.1.1.0/24"))
        );
        assert_eq!(
            next_free_prefix(c, &[pfx("10.1.0.0/25")], 23),
            Some(pfx("10.1.2.0/23"))
        );
        assert_eq!(
            next_free_prefix(c, &[pfx("10.1.0.0/17"), pfx("10.1.128.0/17")], 24),
            None
        );
        assert_eq!(
            next_free_prefix(c, &[], 15),
            None,
            "shorter than the container"
        );
        assert_eq!(
            next_free_prefix(pfx("::/0"), &[pfx("::/1")], 1),
            Some(pfx("8000::/1"))
        );
    }

    fn arb_children() -> impl Strategy<Value = Vec<Prefix>> {
        // Disjoint children of 10.0.0.0/24: pick lengths 26..=30 at random aligned slots,
        // keep those not overlapping the ones already kept.
        prop::collection::vec((26u8..=30, 0u32..256), 0..20).prop_map(|picks| {
            let base = pfx("10.0.0.0/24");
            let mut kept: Vec<Prefix> = Vec::new();
            for (len, slot) in picks {
                let a = Addr::new(base.family(), base.network().bits() + u128::from(slot)).unwrap();
                let p = Prefix::containing(a, len).unwrap();
                if kept.iter().all(|k| !k.overlaps(p)) {
                    kept.push(p);
                }
            }
            kept
        })
    }

    proptest! {
        #[test]
        fn next_free_prefix_is_free_aligned_and_minimal(children in arb_children(), len in 24u8..=32) {
            let c = pfx("10.0.0.0/24");
            let found = next_free_prefix(c, &children, len);
            // Brute force: every aligned block of length `len`, in order.
            let step = 1u128 << (32 - len);
            let expected = (0..(256 / step)).map(|i| {
                Prefix::new(Addr::new(c.family(), c.network().bits() + i * step).unwrap(), len).unwrap()
            }).find(|p| children.iter().all(|k| !k.overlaps(*p)));
            prop_assert_eq!(found, expected);
        }

        #[test]
        fn next_free_address_is_assignable_and_minimal(taken in prop::collection::btree_set(0u32..256, 0..200)) {
            let n = net("10.0.0.0/24", false);
            let addresses: Vec<Addr> = taken.iter().map(|&o| Addr::new(n.value.family(), n.value.network().bits() + u128::from(o)).unwrap()).collect();
            let found = next_free_address(&n, &[], &addresses);
            let expected = (1u32..255).find(|o| !taken.contains(o))
                .map(|o| Addr::new(n.value.family(), n.value.network().bits() + u128::from(o)).unwrap());
            prop_assert_eq!(found, expected);
            if let Some(a) = found {
                prop_assert!(network_occupancy(&n, &[], &addresses).used < capacity(&n));
                prop_assert!(!reserved_in_network(a, &n));
            }
        }

        #[test]
        fn occupancy_never_exceeds_capacity(taken in prop::collection::btree_set(1u32..255, 0..254)) {
            let n = net("10.0.0.0/24", false);
            let addresses: Vec<Addr> = taken.iter().map(|&o| Addr::new(n.value.family(), n.value.network().bits() + u128::from(o)).unwrap()).collect();
            let o = network_occupancy(&n, &[], &addresses);
            prop_assert!(o.used <= o.total);
        }
    }
}
