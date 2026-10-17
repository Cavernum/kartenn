//! Objects of the core and their local rules (spec 002, REQ-002-48).
//!
//! The core carries only the attributes on which a core invariant or an RBAC anchor
//! depends (002/A10): owner entity and site are properties declared by the core
//! (`core.owner`, `core.site`, ADR 0014), every descriptive attribute is a property
//! (spec 003). Local rules look at one object, possibly with its network; rules that
//! need the rest of the domain run in the write pipeline.

use std::fmt;
use std::sync::LazyLock;

use uuid::Uuid;

use crate::addr::{Addr, Prefix};
use crate::family::{IPV6, intrinsically_unassignable};
use crate::registry::special_registry;
use crate::rules::{self, Violation};

/// A technical identifier of an entity, site or domain: 1 to 63 characters
/// `[a-z0-9-]`, no leading or trailing hyphen (REQ-002-47). Not a display name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

impl Slug {
    /// Validates a slug.
    ///
    /// # Errors
    ///
    /// A `core.slug.syntax` violation.
    pub fn parse(text: &str) -> Result<Self, Violation> {
        let valid = (1..=63).contains(&text.len())
            && text
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !text.starts_with('-')
            && !text.ends_with('-');
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(Violation::new(
                &rules::SLUG_SYNTAX,
                format!("{text:?} is not a valid slug"),
            ))
        }
    }

    /// The slug text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An entity: owner of addressing objects (group, subsidiary, contractor…), in a tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    /// Identifier.
    pub id: Uuid,
    /// Technical identifier.
    pub slug: Slug,
    /// Parent entity.
    pub parent: Option<Uuid>,
}

/// A site (region, site, building…), in a tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// Identifier.
    pub id: Uuid,
    /// Technical identifier.
    pub slug: Slug,
    /// Parent site.
    pub parent: Option<Uuid>,
}

/// An addressing domain: the space in which an address is unique (D4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Domain {
    /// Identifier.
    pub id: Uuid,
    /// Technical identifier.
    pub slug: Slug,
}

/// Nature of a prefix (002/A2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Nature {
    /// Aggregate or supernet: contains only prefixes.
    Container,
    /// Layer-3 subnet: contains ranges and addresses; a leaf of the prefix tree.
    Network,
}

impl Nature {
    /// Stored form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::Network => "network",
        }
    }
}

/// A prefix of an addressing domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixObject {
    /// Identifier.
    pub id: Uuid,
    /// Domain (immutable, REQ-002-12).
    pub domain: Uuid,
    /// CIDR value.
    pub value: Prefix,
    /// Container or network.
    pub nature: Nature,
    /// Point-to-point network (RFC 3021, RFC 6164).
    pub point_to_point: bool,
    /// Anycast network: may exist in several domains, holds anycast addresses only
    /// (REQ-002-46, 49).
    pub anycast: bool,
}

/// Usage of a range (002/A5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeUsage {
    /// Dynamic pool: no individual address except DHCP reservations.
    Dhcp,
    /// Reserved: no individual address.
    Reserved,
    /// Static: individual addresses only.
    Static,
}

impl RangeUsage {
    /// Stored form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dhcp => "dhcp",
            Self::Reserved => "reserved",
            Self::Static => "static",
        }
    }
}

/// A range of addresses inside a network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpRange {
    /// Identifier.
    pub id: Uuid,
    /// Domain.
    pub domain: Uuid,
    /// First address.
    pub first: Addr,
    /// Last address (inclusive).
    pub last: Addr,
    /// Usage.
    pub usage: RangeUsage,
}

/// Usage of an address (002/A5, A11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressUsage {
    /// Host.
    Host,
    /// Gateway.
    Gateway,
    /// Virtual address shared within a domain.
    Vip,
    /// Anycast address, may appear in several domains (REQ-002-46).
    Anycast,
    /// Reserved (also a DHCP reservation inside a DHCP range).
    Reserved,
}

impl AddressUsage {
    /// Stored form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Gateway => "gateway",
            Self::Vip => "vip",
            Self::Anycast => "anycast",
            Self::Reserved => "reserved",
        }
    }
}

/// An address of a network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    /// Identifier.
    pub id: Uuid,
    /// Domain.
    pub domain: Uuid,
    /// Value.
    pub value: Addr,
    /// Usage.
    pub usage: AddressUsage,
}

static IPV4_MAPPED: LazyLock<Prefix> =
    LazyLock::new(|| Prefix::parse(IPV6, "::ffff:0:0/96").expect("literal IPv6 prefix"));
static ULA: LazyLock<Prefix> =
    LazyLock::new(|| Prefix::parse(IPV6, "fd00::/8").expect("literal IPv6 prefix"));

/// IPv4-mapped values are refused, citing the IPv4 equivalent (REQ-002-31).
fn check_ipv4_mapped(value: Prefix, id: Uuid, out: &mut Vec<Violation>) {
    if value.family() == IPV6 && IPV4_MAPPED.contains(value) {
        let v4 = std::net::Ipv6Addr::from_bits(value.network().bits())
            .to_ipv4_mapped()
            .map_or_else(String::new, |v4| format!(" (use {v4})"));
        out.push(
            Violation::new(
                &rules::ADDR_IPV4_MAPPED,
                format!("{value} is IPv4-mapped{v4}"),
            )
            .with_subjects([id]),
        );
    }
}

/// Documentation and shared-space warnings, from the IANA registry names (REQ-002-35).
fn check_special_use(value: Prefix, id: Uuid, out: &mut Vec<Violation>) {
    let Some(entry) = special_registry(value.family()).and_then(|r| r.classify(value)) else {
        return;
    };
    if entry.name.starts_with("Documentation") {
        out.push(
            Violation::new(
                &rules::ADDR_DOCUMENTATION,
                format!("{value} is in a documentation block ({})", entry.rfc),
            )
            .with_subjects([id]),
        );
    } else if entry.name == "Shared Address Space" {
        out.push(
            Violation::new(
                &rules::ADDR_SHARED_SPACE,
                format!("{value} is in the shared address space (RFC 6598)"),
            )
            .with_subjects([id]),
        );
    }
}

/// Whether a ULA Global ID looks hand-made rather than pseudo-random (002/design):
/// zero, at most 0xFFFF, five identical bytes, or bytes in a ±1 sequence.
#[must_use]
pub fn ula_global_id_looks_sequential(bytes: [u8; 5]) -> bool {
    let global_id = bytes.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b));
    let identical = bytes.windows(2).all(|w| w[0] == w[1]);
    let rising = bytes.windows(2).all(|w| w[0].checked_add(1) == Some(w[1]));
    let falling = bytes.windows(2).all(|w| w[0].checked_sub(1) == Some(w[1]));
    global_id <= 0xFFFF || identical || rising || falling
}

/// Local rules of a prefix (REQ-002-19, 31, 35).
#[must_use]
pub fn check_prefix(p: &PrefixObject) -> Vec<Violation> {
    let mut out = Vec::new();
    check_ipv4_mapped(p.value, p.id, &mut out);
    let width = p.value.family().width();
    let len = p.value.prefix_len();
    if p.point_to_point && (p.nature != Nature::Network || len + 1 < width) {
        out.push(
            Violation::new(
                &rules::PREFIX_P2P_LENGTH,
                format!(
                    "{} cannot be point-to-point: only /{} and /{width} networks can",
                    p.value,
                    width - 1
                ),
            )
            .with_subjects([p.id]),
        );
    }
    if p.anycast && p.nature != Nature::Network {
        out.push(
            Violation::new(
                &rules::PREFIX_ANYCAST_NETWORK,
                format!("{} is a container and cannot be anycast", p.value),
            )
            .with_subjects([p.id]),
        );
    }
    if p.value.family() == IPV6
        && p.nature == Nature::Network
        && len > 64
        && !(p.point_to_point && len >= 127)
    {
        out.push(
            Violation::new(
                &rules::PREFIX_IPV6_LONG_SUBNET,
                format!("{} is longer than /64 for an IPv6 host network", p.value),
            )
            .with_subjects([p.id]),
        );
    }
    if len >= 48 && ULA.contains(p.value) {
        let octets = p.value.network().bits().to_be_bytes();
        let global_id = [octets[1], octets[2], octets[3], octets[4], octets[5]];
        if ula_global_id_looks_sequential(global_id) {
            out.push(
                Violation::new(
                    &rules::PREFIX_ULA_GLOBAL_ID,
                    format!(
                        "the Global ID of {} does not look pseudo-random (RFC 4193 §3.2)",
                        p.value
                    ),
                )
                .with_subjects([p.id]),
            );
        }
    }
    check_special_use(p.value, p.id, &mut out);
    out
}

/// Local rules of a range (REQ-002-21 bounds, 31, 35).
#[must_use]
pub fn check_range(r: &IpRange) -> Vec<Violation> {
    let mut out = Vec::new();
    if r.first.family() != r.last.family() || r.first > r.last {
        out.push(
            Violation::new(
                &rules::RANGE_BOUNDS,
                format!("{} – {} is not a valid range", r.first, r.last),
            )
            .with_subjects([r.id]),
        );
        return out;
    }
    for bound in [r.first, r.last] {
        check_ipv4_mapped(Prefix::host(bound), r.id, &mut out);
    }
    check_special_use(Prefix::host(r.first), r.id, &mut out);
    out.dedup();
    out
}

/// Local rules of an address (REQ-002-29 intrinsic part, 31, 35).
#[must_use]
pub fn check_address(a: &Address) -> Vec<Violation> {
    let mut out = Vec::new();
    let host = Prefix::host(a.value);
    check_ipv4_mapped(host, a.id, &mut out);
    if let Some(block) = intrinsically_unassignable(a.value.family())
        .iter()
        .find(|b| b.contains_addr(a.value))
    {
        out.push(
            Violation::new(
                &rules::ADDRESS_NOT_ASSIGNABLE,
                format!("{} is in {block}, never assigned to a host", a.value),
            )
            .with_subjects([a.id]),
        );
    }
    check_special_use(host, a.id, &mut out);
    out
}

/// Whether `addr` cannot be assigned in `network` because of its position: network and
/// broadcast addresses of an IPv4 network of length ≤ 30, zero interface identifier of an
/// IPv6 network other than a point-to-point /127 or a /128 (REQ-002-23, 29).
#[must_use]
pub fn reserved_in_network(addr: Addr, network: &PrefixObject) -> bool {
    addr.family()
        .implementation()
        .reserved_in_network(addr, network.value, network.point_to_point)
}

/// Rules of an address against its network: positional reservations (REQ-002-29) and
/// anycast usage (REQ-002-49).
#[must_use]
pub fn check_address_in_network(a: &Address, network: &PrefixObject) -> Vec<Violation> {
    let mut out = Vec::new();
    if reserved_in_network(a.value, network) {
        out.push(
            Violation::new(
                &rules::ADDRESS_NOT_ASSIGNABLE,
                format!(
                    "{} is a reserved position of {} (network, broadcast or zero identifier)",
                    a.value, network.value
                ),
            )
            .with_subjects([a.id, network.id]),
        );
    }
    if network.anycast && a.usage != AddressUsage::Anycast {
        out.push(
            Violation::new(
                &rules::ANYCAST_ADDRESS_USAGE,
                format!(
                    "{} is in the anycast network {} and must be of usage anycast",
                    a.value, network.value
                ),
            )
            .with_subjects([a.id, network.id]),
        );
    }
    out
}

/// Rules of a range against its network (REQ-002-23).
#[must_use]
pub fn check_range_in_network(r: &IpRange, network: &PrefixObject) -> Vec<Violation> {
    let net = network.value;
    let reserved = [net.network(), net.last()]
        .into_iter()
        .filter(|&a| reserved_in_network(a, network))
        .find(|&a| r.first <= a && a <= r.last);
    reserved.map_or_else(Vec::new, |a| {
        vec![
            Violation::new(
                &rules::RANGE_RESERVED_BOUNDS,
                format!(
                    "{} – {} includes {a}, a reserved position of {net}",
                    r.first, r.last
                ),
            )
            .with_subjects([r.id, network.id]),
        ]
    })
}

/// Rules of an address against a range that contains it (REQ-002-28, 002/A8): only DHCP
/// reservations in a DHCP range, nothing in a reserved range, anything in a static one.
#[must_use]
pub fn check_address_in_range(a: &Address, r: &IpRange) -> Vec<Violation> {
    if a.value < r.first || r.last < a.value {
        return Vec::new();
    }
    let refused = match r.usage {
        RangeUsage::Dhcp => a.usage != AddressUsage::Reserved,
        RangeUsage::Reserved => true,
        RangeUsage::Static => false,
    };
    if refused {
        vec![
            Violation::new(
                &rules::ADDRESS_RANGE_USAGE,
                format!(
                    "{} ({}) is inside the {} range {} – {}",
                    a.value,
                    a.usage.as_str(),
                    r.usage.as_str(),
                    r.first,
                    r.last
                ),
            )
            .with_subjects([a.id, r.id]),
        ]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::family::ALL;

    fn addr(t: &str) -> Addr {
        Addr::parse_any(&ALL, t).unwrap()
    }

    fn prefix(t: &str, nature: Nature, p2p: bool) -> PrefixObject {
        PrefixObject {
            id: Uuid::now_v7(),
            domain: Uuid::nil(),
            value: Prefix::parse_any(&ALL, t).unwrap(),
            nature,
            point_to_point: p2p,
            anycast: false,
        }
    }

    fn net(t: &str) -> PrefixObject {
        prefix(t, Nature::Network, false)
    }

    fn address(t: &str) -> Address {
        Address {
            id: Uuid::now_v7(),
            domain: Uuid::nil(),
            value: addr(t),
            usage: AddressUsage::Host,
        }
    }

    fn range(first: &str, last: &str) -> IpRange {
        IpRange {
            id: Uuid::now_v7(),
            domain: Uuid::nil(),
            first: addr(first),
            last: addr(last),
            usage: RangeUsage::Dhcp,
        }
    }

    fn ids(v: &[Violation]) -> Vec<&'static str> {
        v.iter().map(|v| v.rule.id).collect()
    }

    #[test]
    fn slugs() {
        // REQ-002-47 acceptance.
        assert!(Slug::parse("siege-paris").is_ok());
        assert!(Slug::parse("a").is_ok());
        assert!(Slug::parse(&"a".repeat(63)).is_ok());
        for bad in ["Siège", "-x", "x-", "a_b", "", "UPPER", &"a".repeat(64)] {
            assert_eq!(
                Slug::parse(bad).unwrap_err().rule.id,
                "core.slug.syntax",
                "{bad:?}"
            );
        }
    }

    #[test]
    fn point_to_point_lengths() {
        // REQ-002-19 acceptance.
        assert!(
            check_prefix(&prefix("192.0.2.0/31", Nature::Network, true))
                .iter()
                .all(|v| v.rule.id != "core.prefix.p2p_length")
        );
        assert!(
            ids(&check_prefix(&prefix("10.0.0.0/30", Nature::Network, true)))
                .contains(&"core.prefix.p2p_length")
        );
        assert!(
            ids(&check_prefix(&prefix(
                "10.0.0.0/31",
                Nature::Container,
                true
            )))
            .contains(&"core.prefix.p2p_length")
        );
        assert!(
            !ids(&check_prefix(&prefix(
                "2001:db8:1::/127",
                Nature::Network,
                true
            )))
            .contains(&"core.prefix.p2p_length")
        );
    }

    #[test]
    fn only_networks_are_anycast_and_hold_anycast_addresses() {
        // REQ-002-49 acceptance.
        let anycast = |nature| PrefixObject {
            anycast: true,
            ..prefix("45.0.0.0/24", nature, false)
        };
        assert!(check_prefix(&anycast(Nature::Network)).is_empty());
        assert!(
            ids(&check_prefix(&anycast(Nature::Container)))
                .contains(&"core.prefix.anycast_network")
        );
        let host = address("45.0.0.1");
        let any = Address {
            usage: AddressUsage::Anycast,
            ..address("45.0.0.2")
        };
        assert_eq!(
            ids(&check_address_in_network(&host, &anycast(Nature::Network))),
            vec!["core.anycast.address_usage"]
        );
        assert!(check_address_in_network(&any, &anycast(Nature::Network)).is_empty());
        assert!(
            check_address_in_network(&any, &net("45.0.0.0/24")).is_empty(),
            "an anycast address in a plain network stays accepted (A11)"
        );
    }

    #[test]
    fn ipv6_long_subnets_warn_unless_point_to_point() {
        assert_eq!(
            ids(&check_prefix(&net("2a00:1::/80"))),
            vec!["core.prefix.ipv6_long_subnet"]
        );
        assert!(check_prefix(&prefix("2a00:1::/127", Nature::Network, true)).is_empty());
        assert!(check_prefix(&net("2a00:1::/64")).is_empty());
        assert!(check_prefix(&prefix("2a00:1::/80", Nature::Container, false)).is_empty());
    }

    #[test]
    fn ula_global_id_heuristic() {
        assert_eq!(
            ids(&check_prefix(&net("fd00::/64"))),
            vec!["core.prefix.ula_global_id"]
        );
        assert_eq!(
            ids(&check_prefix(&net("fd00:0:1::/64"))),
            vec!["core.prefix.ula_global_id"]
        );
        assert_eq!(
            ids(&check_prefix(&net("fd11:1111:1111::/64"))),
            vec!["core.prefix.ula_global_id"]
        );
        assert_eq!(
            ids(&check_prefix(&net("fd01:0203:0405::/64"))),
            vec!["core.prefix.ula_global_id"]
        );
        assert!(check_prefix(&net("fd8b:3e19:a4c2::/64")).is_empty());
        assert!(
            check_prefix(&prefix("fd00::/8", Nature::Container, false)).is_empty(),
            "no Global ID below /48"
        );
    }

    #[test]
    fn documentation_and_shared_space_warn() {
        assert_eq!(
            ids(&check_prefix(&net("192.0.2.0/24"))),
            vec!["core.addr.documentation"]
        );
        assert_eq!(
            ids(&check_prefix(&net("3fff:1::/64"))),
            vec!["core.addr.documentation"]
        );
        assert_eq!(
            ids(&check_prefix(&net("100.64.1.0/24"))),
            vec!["core.addr.shared_space"]
        );
        assert!(check_prefix(&net("10.1.2.0/24")).is_empty());
    }

    #[test]
    fn ipv4_mapped_is_refused_with_the_ipv4_value() {
        let v = check_address(&address("::ffff:192.0.2.1"));
        assert_eq!(v[0].rule.id, "core.addr.ipv4_mapped");
        assert!(v[0].message.contains("use 192.0.2.1"));
    }

    #[test]
    fn intrinsically_unassignable_addresses() {
        // REQ-002-29 acceptance, intrinsic part.
        for t in [
            "0.1.2.3",
            "127.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
            "224.0.0.5",
            "::",
            "::1",
            "ff02::1",
        ] {
            assert_eq!(
                ids(&check_address(&address(t))),
                vec!["core.address.not_assignable"],
                "{t}"
            );
        }
        assert!(check_address(&address("10.1.2.3")).is_empty());
        assert!(
            check_address(&address("169.254.1.1")).is_empty(),
            "link-local is assignable"
        );
    }

    #[test]
    fn positional_reservations() {
        // REQ-002-29 acceptance, positional part.
        let n = net("10.1.2.0/24");
        assert_eq!(
            ids(&check_address_in_network(&address("10.1.2.0"), &n)),
            vec!["core.address.not_assignable"]
        );
        assert_eq!(
            ids(&check_address_in_network(&address("10.1.2.255"), &n)),
            vec!["core.address.not_assignable"]
        );
        assert!(check_address_in_network(&address("10.1.2.1"), &n).is_empty());
        let p31 = prefix("192.0.2.0/31", Nature::Network, true);
        assert!(check_address_in_network(&address("192.0.2.0"), &p31).is_empty());
        assert!(check_address_in_network(&address("192.0.2.1"), &p31).is_empty());
        assert!(check_address_in_network(&address("10.9.9.9"), &net("10.9.9.9/32")).is_empty());
        let v6 = net("2001:db8:1::/64");
        assert_eq!(
            ids(&check_address_in_network(&address("2001:db8:1::"), &v6)),
            vec!["core.address.not_assignable"]
        );
        let v6p2p = prefix("2001:db8:1::/127", Nature::Network, true);
        assert!(check_address_in_network(&address("2001:db8:1::"), &v6p2p).is_empty());
        assert!(
            !check_address_in_network(&address("2001:db8:1::"), &net("2001:db8:1::/127"))
                .is_empty()
        );
    }

    #[test]
    fn range_bounds_and_reserved_positions() {
        // REQ-002-21 bounds, REQ-002-23 acceptance.
        assert_eq!(
            ids(&check_range(&range("10.1.2.50", "10.1.2.10"))),
            vec!["core.range.bounds"]
        );
        assert_eq!(
            ids(&check_range(&range("10.1.2.1", "::1"))),
            vec!["core.range.bounds"]
        );
        let n = net("10.1.2.0/24");
        assert_eq!(
            ids(&check_range_in_network(&range("10.1.2.0", "10.1.2.20"), &n)),
            vec!["core.range.reserved_bounds"]
        );
        assert_eq!(
            ids(&check_range_in_network(
                &range("10.1.2.200", "10.1.2.255"),
                &n
            )),
            vec!["core.range.reserved_bounds"]
        );
        assert!(check_range_in_network(&range("10.1.2.10", "10.1.2.50"), &n).is_empty());
    }

    #[test]
    fn addresses_respect_the_usage_of_their_range() {
        // REQ-002-28 acceptance.
        let with = |usage| Address {
            usage,
            ..address("10.1.2.150")
        };
        let dhcp = range("10.1.2.100", "10.1.2.199");
        assert_eq!(
            ids(&check_address_in_range(&with(AddressUsage::Host), &dhcp)),
            vec!["core.address.range_usage"]
        );
        assert!(check_address_in_range(&with(AddressUsage::Reserved), &dhcp).is_empty());
        let reserved = IpRange {
            usage: RangeUsage::Reserved,
            ..range("10.1.2.100", "10.1.2.199")
        };
        assert_eq!(
            ids(&check_address_in_range(
                &with(AddressUsage::Reserved),
                &reserved
            )),
            vec!["core.address.range_usage"]
        );
        let static_range = IpRange {
            usage: RangeUsage::Static,
            ..range("10.1.2.100", "10.1.2.199")
        };
        assert!(check_address_in_range(&with(AddressUsage::Host), &static_range).is_empty());
        assert!(
            check_address_in_range(&address("10.1.2.7"), &dhcp).is_empty(),
            "outside the range"
        );
    }
}
