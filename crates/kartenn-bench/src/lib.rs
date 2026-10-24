//! Synthetic datasets and benchmarks for kartenn.
//!
//! The generator builds a reproducible addressing dataset shaped like a large merged
//! information system (002/REQ-002-45): several addressing domains, some of them reusing
//! `10.0.0.0/8`, container trees at least six levels deep, networks, DHCP ranges and
//! addresses, in IPv4 and IPv6, with a share of public space. It is independent of
//! `kartenn-core` on purpose: it feeds the risk prototypes before the core exists.

use std::collections::BTreeSet;
use std::fmt;
use std::io::{self, Write};
use std::net::{Ipv4Addr, Ipv6Addr};

use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use uuid::Uuid;

pub mod core_budgets;
pub mod events_budgets;
pub mod flow_budgets;
pub mod http;
pub mod machine_budgets;
pub mod policy_budgets;
pub mod props;
pub mod rbac;
mod stats_budgets;
pub mod telemetry;
pub mod vlan_budgets;

/// Address family of a generated value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// IPv4, 32 bits.
    V4,
    /// IPv6, 128 bits.
    V6,
}

impl Family {
    /// Width of the family in bits.
    #[must_use]
    pub const fn width(self) -> u8 {
        match self {
            Self::V4 => 32,
            Self::V6 => 128,
        }
    }
}

/// An address value, left as raw bits within its family width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Addr {
    /// Family of the address.
    pub family: Family,
    /// Bits of the address (only the low `family.width()` bits are used).
    pub bits: u128,
}

impl fmt::Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.family {
            Family::V4 => {
                let v4 = u32::try_from(self.bits).map_err(|_| fmt::Error)?;
                write!(f, "{}", Ipv4Addr::from(v4))
            }
            Family::V6 => write!(f, "{}", Ipv6Addr::from(self.bits)),
        }
    }
}

/// A CIDR prefix: an aligned address and a length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Prefix {
    /// Network address (host bits are zero).
    pub addr: Addr,
    /// Prefix length in bits.
    pub len: u8,
}

impl Prefix {
    /// Number of host bits of the prefix.
    #[must_use]
    pub const fn host_bits(self) -> u8 {
        self.addr.family.width() - self.len
    }

    /// The ancestor of this prefix at a shorter length.
    #[must_use]
    pub fn truncate(self, len: u8) -> Self {
        let host = self.addr.family.width() - len;
        let bits = if host >= 128 {
            0
        } else {
            (self.addr.bits >> host) << host
        };
        Self {
            addr: Addr {
                family: self.addr.family,
                bits,
            },
            len,
        }
    }
}

impl fmt::Display for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.addr, self.len)
    }
}

/// Nature of a generated prefix (002/A2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nature {
    /// Contains only prefixes.
    Container,
    /// Layer-3 subnet, contains ranges and addresses.
    Network,
}

impl Nature {
    /// Name, as stored.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::Network => "network",
        }
    }
}

/// A generated addressing domain.
#[derive(Debug, Clone)]
pub struct Domain {
    /// Identifier of the domain.
    pub id: Uuid,
    /// Technical slug of the domain.
    pub slug: String,
    /// Whether the domain is numbered in public space.
    pub public: bool,
}

/// A generated prefix.
#[derive(Debug, Clone)]
pub struct PrefixRow {
    /// Identifier.
    pub id: Uuid,
    /// Index of the domain in [`Dataset::domains`].
    pub domain: usize,
    /// Value.
    pub prefix: Prefix,
    /// Container or network.
    pub nature: Nature,
}

/// A generated DHCP range.
#[derive(Debug, Clone)]
pub struct RangeRow {
    /// Identifier.
    pub id: Uuid,
    /// Index of the domain in [`Dataset::domains`].
    pub domain: usize,
    /// First address.
    pub start: Addr,
    /// Last address (inclusive).
    pub end: Addr,
}

/// A generated address.
#[derive(Debug, Clone)]
pub struct AddressRow {
    /// Identifier.
    pub id: Uuid,
    /// Index of the domain in [`Dataset::domains`].
    pub domain: usize,
    /// Value.
    pub addr: Addr,
}

/// Size parameters of a dataset.
#[derive(Debug, Clone, Copy)]
pub struct Scale {
    /// Number of addressing domains (at least 20 for the reference dataset).
    pub domains: usize,
    /// Networks per domain.
    pub networks_per_domain: usize,
    /// Mean addresses per network.
    pub addresses_per_network: usize,
}

impl Scale {
    /// Reference dataset of the performance budgets: 20 domains, about 10⁵ networks
    /// (three domains are capped at 2 048 by their `172.16.0.0/12` root) and 10⁶ addresses.
    pub const REFERENCE: Self = Self {
        domains: 20,
        networks_per_domain: 5_500,
        addresses_per_network: 10,
    };

    /// Reduced dataset (`K_BENCH_SCALE=reduced`, the default of the benches): the 20
    /// domains of the reference with a tenth of its networks, about 1.1·10⁴ networks and
    /// 1.1·10⁵ addresses. Its verdicts check the benches, not the budgets (ADR 0032).
    pub const REDUCED: Self = Self {
        domains: 20,
        networks_per_domain: 550,
        addresses_per_network: 10,
    };

    /// Small dataset for tests.
    pub const SMALL: Self = Self {
        domains: 20,
        networks_per_domain: 50,
        addresses_per_network: 4,
    };
}

/// A generated dataset.
#[derive(Debug, Clone, Default)]
pub struct Dataset {
    /// Addressing domains.
    pub domains: Vec<Domain>,
    /// Containers and networks, parents before children within a domain.
    pub prefixes: Vec<PrefixRow>,
    /// DHCP ranges, one per tenth network.
    pub ranges: Vec<RangeRow>,
    /// Addresses.
    pub addresses: Vec<AddressRow>,
}

/// Shape of the tree of one domain: root, container levels, network length.
struct Layout {
    root: Prefix,
    levels: &'static [u8],
    network_len: u8,
    public: bool,
}

const V4_LEVELS_FROM_8: &[u8] = &[11, 14, 17, 20, 22];
const V4_LEVELS_FROM_12: &[u8] = &[14, 16, 18, 20, 22];
const V4_LEVELS_FROM_10: &[u8] = &[12, 14, 16, 18, 20, 22];
const V6_LEVELS_FROM_48: &[u8] = &[50, 52, 54, 56, 60];

fn v4(a: u8, b: u8, len: u8) -> Prefix {
    let bits = u128::from(u32::from(Ipv4Addr::new(a, b, 0, 0)));
    Prefix {
        addr: Addr {
            family: Family::V4,
            bits,
        },
        len,
    }
}

fn v6(bits: u128, len: u8) -> Prefix {
    Prefix {
        addr: Addr {
            family: Family::V6,
            bits,
        },
        len,
    }
}

/// Domain layouts, cycling through the shapes of a merged information system:
/// domains 0–4 reuse `10.0.0.0/8`; then RFC 1918 /12, ULA /48 and public /12 or /48.
fn layout(index: usize, rng: &mut ChaCha8Rng) -> Layout {
    let lane = u8::try_from(index % 16).expect("index % 16 fits in u8");
    match index {
        0..=4 => Layout {
            root: v4(10, 0, 8),
            levels: V4_LEVELS_FROM_8,
            network_len: 24,
            public: false,
        },
        _ => match index % 4 {
            0 => Layout {
                root: v4(172, 16, 12),
                levels: V4_LEVELS_FROM_12,
                network_len: 24,
                public: false,
            },
            1 => {
                // ULA fd00::/8 with a pseudo-random 40-bit Global ID (RFC 4193 §3.2).
                let global_id: u128 = rng.random_range(1..(1u128 << 40));
                let bits = (0xfdu128 << 120) | (global_id << 80);
                Layout {
                    root: v6(bits, 48),
                    levels: V6_LEVELS_FROM_48,
                    network_len: 64,
                    public: false,
                }
            }
            2 => Layout {
                // Public IPv4, a distinct /10 per domain: (20 + lane).0.0.0/10.
                root: v4(20 + lane, 0, 10),
                levels: V4_LEVELS_FROM_10,
                network_len: 24,
                public: true,
            },
            _ => {
                // Public IPv6, a distinct /48 per domain inside 2a00::/16.
                let bits = (0x2a00u128 << 112) | (u128::from(lane) << 80);
                Layout {
                    root: v6(bits, 48),
                    levels: V6_LEVELS_FROM_48,
                    network_len: 64,
                    public: true,
                }
            }
        },
    }
}

fn uuid(rng: &mut ChaCha8Rng) -> Uuid {
    uuid::Builder::from_random_bytes(rng.random()).into_uuid()
}

/// Generates a dataset reproducibly from a seed.
///
/// A domain gets at most half of the networks its root can hold, so that random
/// placement stays fast; the reference scale stays well below that bound.
#[must_use]
pub fn generate(seed: u64, scale: Scale) -> Dataset {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut data = Dataset::default();
    for d in 0..scale.domains {
        let layout = layout(d, &mut rng);
        data.domains.push(Domain {
            id: uuid(&mut rng),
            slug: format!("domain-{d:02}"),
            public: layout.public,
        });
        generate_domain(&mut rng, &mut data, d, &layout, scale);
    }
    data
}

fn generate_domain(
    rng: &mut ChaCha8Rng,
    data: &mut Dataset,
    d: usize,
    layout: &Layout,
    scale: Scale,
) {
    let slots_bits = layout.network_len - layout.root.len;
    let slots: u128 = 1u128 << slots_bits;
    // At most half the slots, so that rejection sampling of free slots stays cheap.
    let wanted = u128::try_from(scale.networks_per_domain)
        .expect("usize fits in u128")
        .min(slots / 2);
    let host_bits = layout.root.addr.family.width() - layout.network_len;
    let mut networks = BTreeSet::new();
    while u128::try_from(networks.len()).expect("usize fits in u128") < wanted {
        let slot: u128 = rng.random_range(0..slots);
        let bits = layout.root.addr.bits | (slot << host_bits);
        networks.insert(Prefix {
            addr: Addr {
                family: layout.root.addr.family,
                bits,
            },
            len: layout.network_len,
        });
    }
    let mut containers = BTreeSet::from([layout.root]);
    for net in &networks {
        for &len in layout.levels {
            containers.insert(net.truncate(len));
        }
    }
    let mut by_len: Vec<Prefix> = containers.into_iter().collect();
    by_len.sort_by_key(|p| (p.len, p.addr));
    for prefix in by_len {
        data.prefixes.push(PrefixRow {
            id: uuid(rng),
            domain: d,
            prefix,
            nature: Nature::Container,
        });
    }
    for (i, net) in networks.into_iter().enumerate() {
        data.prefixes.push(PrefixRow {
            id: uuid(rng),
            domain: d,
            prefix: net,
            nature: Nature::Network,
        });
        generate_network(rng, data, d, net, i, scale);
    }
}

fn generate_network(
    rng: &mut ChaCha8Rng,
    data: &mut Dataset,
    d: usize,
    net: Prefix,
    index: usize,
    scale: Scale,
) {
    let family = net.addr.family;
    // Host offsets 1..=200 avoid the network address, the IPv4 broadcast (offset 255
    // of a /24) and the IPv6 Subnet-Router anycast (IID 0).
    let dhcp = index.is_multiple_of(10);
    let max_offset: u128 = if dhcp { 99 } else { 200 };
    if dhcp {
        data.ranges.push(RangeRow {
            id: uuid(rng),
            domain: d,
            start: Addr {
                family,
                bits: net.addr.bits + 100,
            },
            end: Addr {
                family,
                bits: net.addr.bits + 199,
            },
        });
    }
    let mean = scale.addresses_per_network.max(1);
    let count = rng.random_range(mean / 2..=mean + mean / 2).max(1);
    let mut offsets = BTreeSet::new();
    while offsets.len() < count {
        offsets.insert(rng.random_range(1..=max_offset));
    }
    for offset in offsets {
        data.addresses.push(AddressRow {
            id: uuid(rng),
            domain: d,
            addr: Addr {
                family,
                bits: net.addr.bits + offset,
            },
        });
    }
}

impl Dataset {
    /// Writes the domains as PostgreSQL `COPY … FROM STDIN` text: `id, slug, public`.
    ///
    /// # Errors
    ///
    /// Returns any I/O error of the writer.
    pub fn write_domains(&self, out: &mut impl Write) -> io::Result<()> {
        for d in &self.domains {
            writeln!(out, "{}\t{}\t{}", d.id, d.slug, d.public)?;
        }
        Ok(())
    }

    /// Writes the prefixes as `COPY` text: `id, domain_id, prefix, nature`.
    ///
    /// # Errors
    ///
    /// Returns any I/O error of the writer.
    pub fn write_prefixes(&self, out: &mut impl Write) -> io::Result<()> {
        for p in &self.prefixes {
            let domain = self.domains[p.domain].id;
            writeln!(
                out,
                "{}\t{domain}\t{}\t{}",
                p.id,
                p.prefix,
                p.nature.as_str()
            )?;
        }
        Ok(())
    }

    /// Writes the ranges as `COPY` text: `id, domain_id, start, end`.
    ///
    /// # Errors
    ///
    /// Returns any I/O error of the writer.
    pub fn write_ranges(&self, out: &mut impl Write) -> io::Result<()> {
        for r in &self.ranges {
            let domain = self.domains[r.domain].id;
            writeln!(out, "{}\t{domain}\t{}\t{}", r.id, r.start, r.end)?;
        }
        Ok(())
    }

    /// Writes the addresses as `COPY` text: `id, domain_id, address`.
    ///
    /// # Errors
    ///
    /// Returns any I/O error of the writer.
    pub fn write_addresses(&self, out: &mut impl Write) -> io::Result<()> {
        for a in &self.addresses {
            let domain = self.domains[a.domain].id;
            writeln!(out, "{}\t{domain}\t{}", a.id, a.addr)?;
        }
        Ok(())
    }

    /// Depth of the deepest container chain of the dataset, network excluded.
    #[must_use]
    pub fn max_container_depth(&self) -> usize {
        let mut depth_by_domain = vec![BTreeSet::new(); self.domains.len()];
        for p in self
            .prefixes
            .iter()
            .filter(|p| p.nature == Nature::Container)
        {
            depth_by_domain[p.domain].insert(p.prefix.len);
        }
        depth_by_domain.iter().map(BTreeSet::len).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump(data: &Dataset) -> Vec<u8> {
        let mut out = Vec::new();
        data.write_domains(&mut out).unwrap();
        data.write_prefixes(&mut out).unwrap();
        data.write_ranges(&mut out).unwrap();
        data.write_addresses(&mut out).unwrap();
        out
    }

    #[test]
    fn same_seed_same_dataset() {
        assert_eq!(
            dump(&generate(7, Scale::SMALL)),
            dump(&generate(7, Scale::SMALL))
        );
        assert_ne!(
            dump(&generate(7, Scale::SMALL)),
            dump(&generate(8, Scale::SMALL))
        );
    }

    #[test]
    fn shape_matches_the_reference_requirements() {
        let data = generate(42, Scale::SMALL);
        assert_eq!(data.domains.len(), 20);
        let ten_slash_eight = v4(10, 0, 8);
        let reusing = data
            .prefixes
            .iter()
            .filter(|p| p.prefix == ten_slash_eight)
            .count();
        assert_eq!(reusing, 5, "five domains reuse 10.0.0.0/8");
        assert!(
            data.max_container_depth() >= 6,
            "containers span at least six levels"
        );
        let families: BTreeSet<_> = data.prefixes.iter().map(|p| p.prefix.addr.family).collect();
        assert_eq!(families.len(), 2, "IPv4 and IPv6");
        assert!(data.domains.iter().any(|d| d.public));
        assert_eq!(
            data.prefixes
                .iter()
                .filter(|p| p.nature == Nature::Network)
                .count(),
            20 * 50
        );
    }

    #[test]
    fn every_address_is_inside_a_network_and_assignable() {
        let data = generate(3, Scale::SMALL);
        let networks: BTreeSet<_> = data
            .prefixes
            .iter()
            .filter(|p| p.nature == Nature::Network)
            .map(|p| (p.domain, p.prefix))
            .collect();
        for a in &data.addresses {
            let len = if a.addr.family == Family::V4 { 24 } else { 64 };
            let net = Prefix { addr: a.addr, len }.truncate(len);
            assert!(
                networks.contains(&(a.domain, net)),
                "{} has no network",
                a.addr
            );
            let offset = a.addr.bits - net.addr.bits;
            assert!((1..=200).contains(&offset), "{} is not assignable", a.addr);
        }
    }

    #[test]
    fn public_domains_do_not_overlap() {
        let data = generate(5, Scale::SMALL);
        let mut roots = BTreeSet::new();
        for p in data
            .prefixes
            .iter()
            .filter(|p| data.domains[p.domain].public && p.prefix.len <= 48)
        {
            if p.prefix.len == 10 || p.prefix.len == 48 {
                assert!(roots.insert(p.prefix), "public root {} reused", p.prefix);
            }
        }
    }
}
