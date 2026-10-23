//! Objects of the policy: axes, value attributes, flow types, services, rules, patterns,
//! conditions, signals and pedagogical texts (REQ-010-4 to 9, 29, 30, 33, 36, 37; design
//! 010 « Données »).
//!
//! Every object is identified by its key, as the storage adapter reads it at a date; the
//! constructors check what a single object can check, the snapshot (REQ-010-11) and the
//! storage check references between objects.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use kartenn_core::properties::PropertyKey;
use kartenn_core::rules::Violation;
use unicode_normalization::UnicodeNormalization;

use crate::rules::{
    CONDITION_PARAMS_INVALID, KEY_SYNTAX, RULE_CONDITIONAL_DENY, RULE_FLOW_SELECTOR_EMPTY,
    RULE_TERM_EMPTY, RULE_UNKNOWN_AXIS, SERVICE_ENTRIES_EMPTY, SERVICE_ENTRY_INVALID,
    SERVICE_FLOW_TYPE, SERVICE_WIDE_PORTS, SIGNAL_PARAMS_INVALID, TEXT_INVALID, TEXT_MISSING,
};

/// Key of a flow type, service, rule, pattern, condition or signal: 1 to 63 characters
/// `[a-z0-9]` and hyphens, neither doubled nor at an end (REQ-010-5).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(String);

impl Key {
    /// Checks a key (`policy.key.syntax`).
    ///
    /// # Errors
    /// A violation of `policy.key.syntax`.
    pub fn new(text: &str) -> Result<Self, Violation> {
        let valid = (1..=63).contains(&text.len())
            && text
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !text.starts_with('-')
            && !text.ends_with('-')
            && !text.contains("--");
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(Violation::new(
                &KEY_SYNTAX,
                format!("{text:?} n'est pas une clé (1 à 63 caractères [a-z0-9-])"),
            ))
        }
    }

    /// The key text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Key of an axis: a property of the 003 designated as an axis (REQ-010-29).
pub type AxisKey = PropertyKey;

/// A value of an axis: the key of a value of a closed list, or an external reference
/// (N26). Values are compared and ordered by their key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AxisValue(String);

impl AxisValue {
    /// A value from its key, as the storage reads it.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// The value key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AxisValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for AxisValue {
    fn from(key: &str) -> Self {
        Self(key.to_owned())
    }
}

/// Who carries the values of an axis (REQ-010-29, 31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carrier {
    /// The addressing tree (domains, containers, networks, addresses).
    Tree,
    /// The machine, with the value of the tree as inherited default (S27).
    Machine,
}

/// A property designated as an axis of the policy (REQ-010-29).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Axis {
    /// The property.
    pub key: AxisKey,
    /// Its carrier.
    pub carrier: Carrier,
    /// Whether the inference produces expected values for it (REQ-010-40).
    pub inference: bool,
}

/// Attributes of a value of an axis (REQ-010-30): never read by a verdict.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ValueAttributes {
    /// A transverse value (`global`), listed by the audit report.
    pub transverse: bool,
    /// Optional rank, read by conditions and signals only (S30).
    pub rank: Option<i32>,
}

/// A flow type (REQ-010-4); its labels are a row's, served by the dictionary (spec 018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowType {
    /// Key.
    pub key: Key,
}

/// A transport of a service entry (REQ-010-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Transport {
    /// TCP (RFC 9293).
    Tcp,
    /// UDP (RFC 768).
    Udp,
    /// SCTP (RFC 9260).
    Sctp,
    /// DCCP (RFC 4340).
    Dccp,
    /// ICMP (RFC 792).
    Icmp,
    /// `ICMPv6` (RFC 4443).
    Icmpv6,
    /// Any IP protocol by number, without ports.
    Ip,
}

impl Transport {
    /// Parses the transport of a request or a row.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "tcp" => Self::Tcp,
            "udp" => Self::Udp,
            "sctp" => Self::Sctp,
            "dccp" => Self::Dccp,
            "icmp" => Self::Icmp,
            "icmpv6" => Self::Icmpv6,
            "ip" => Self::Ip,
            _ => return None,
        })
    }

    /// Whether the transport has ports (RFC 6335 §6).
    #[must_use]
    pub fn has_ports(self) -> bool {
        matches!(self, Self::Tcp | Self::Udp | Self::Sctp | Self::Dccp)
    }
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
            Self::Sctp => "sctp",
            Self::Dccp => "dccp",
            Self::Icmp => "icmp",
            Self::Icmpv6 => "icmpv6",
            Self::Ip => "ip",
        })
    }
}

/// An entry of a service as read from a request or a row, before checks.
#[derive(Debug, Clone, Default)]
pub struct RawEntry<'a> {
    /// Transport name.
    pub transport: &'a str,
    /// Closed range of ports, for the transports that have ports.
    pub ports: Option<(i64, i64)>,
    /// ICMP type.
    pub icmp_type: Option<i64>,
    /// ICMP code (with a type).
    pub icmp_code: Option<i64>,
    /// IP protocol number, for `ip`.
    pub protocol: Option<i64>,
}

/// A checked entry of a service (REQ-010-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ServiceEntry {
    /// A closed range of ports of a transport that has ports.
    Ports {
        /// Transport.
        transport: Transport,
        /// First port.
        first: u16,
        /// Last port (≥ first).
        last: u16,
    },
    /// ICMP or `ICMPv6`, with optional type and code.
    Icmp {
        /// `ICMPv6` rather than ICMP.
        v6: bool,
        /// Type.
        icmp_type: Option<u8>,
        /// Code (only with a type).
        code: Option<u8>,
    },
    /// An IP protocol number, without ports.
    Protocol(u8),
}

fn byte(value: Option<i64>, what: &str) -> Result<Option<u8>, String> {
    value
        .map(|v| u8::try_from(v).map_err(|_| format!("{what} {v} hors de 0–255")))
        .transpose()
}

impl ServiceEntry {
    /// Checks an entry (`policy.service.entry_invalid`).
    ///
    /// # Errors
    /// A violation of `policy.service.entry_invalid`.
    pub fn new(raw: &RawEntry<'_>) -> Result<Self, Violation> {
        Self::check(raw).map_err(|why| Violation::new(&SERVICE_ENTRY_INVALID, why))
    }

    fn check(raw: &RawEntry<'_>) -> Result<Self, String> {
        let transport = Transport::parse(raw.transport)
            .ok_or_else(|| format!("transport {:?} inconnu", raw.transport))?;
        if transport.has_ports() {
            if raw.icmp_type.is_some() || raw.icmp_code.is_some() || raw.protocol.is_some() {
                return Err(format!("{transport} n'a ni type ICMP ni protocole"));
            }
            let (first, last) = raw
                .ports
                .ok_or_else(|| format!("{transport} exige un port ou une plage"))?;
            let port = |p: i64| {
                u16::try_from(p)
                    .ok()
                    .filter(|p| *p >= 1)
                    .ok_or_else(|| format!("{transport}/{p} hors de 1–65535"))
            };
            let (first, last) = (port(first)?, port(last)?);
            if first > last {
                return Err(format!("{transport}/{first}-{last} : plage inversée"));
            }
            return Ok(Self::Ports {
                transport,
                first,
                last,
            });
        }
        if raw.ports.is_some() {
            return Err(format!("{transport} n'a pas de port"));
        }
        match transport {
            Transport::Icmp | Transport::Icmpv6 => {
                if raw.protocol.is_some() {
                    return Err(format!("{transport} n'a pas de numéro de protocole"));
                }
                let icmp_type = byte(raw.icmp_type, "type ICMP")?;
                let code = byte(raw.icmp_code, "code ICMP")?;
                if code.is_some() && icmp_type.is_none() {
                    return Err("un code ICMP exige un type".to_owned());
                }
                Ok(Self::Icmp {
                    v6: transport == Transport::Icmpv6,
                    icmp_type,
                    code,
                })
            }
            _ => {
                if raw.icmp_type.is_some() || raw.icmp_code.is_some() {
                    return Err("ip n'a pas de type ICMP".to_owned());
                }
                let protocol = byte(raw.protocol, "protocole IP")?
                    .ok_or_else(|| "ip exige un numéro de protocole".to_owned())?;
                Ok(Self::Protocol(protocol))
            }
        }
    }

    /// The transport of the entry.
    #[must_use]
    pub fn transport(self) -> Transport {
        match self {
            Self::Ports { transport, .. } => transport,
            Self::Icmp { v6: false, .. } => Transport::Icmp,
            Self::Icmp { v6: true, .. } => Transport::Icmpv6,
            Self::Protocol(_) => Transport::Ip,
        }
    }

    /// Whether the entry covers a port of a transport (REQ-010-14).
    #[must_use]
    pub fn covers_port(self, transport: Transport, port: u16) -> bool {
        matches!(self, Self::Ports { transport: t, first, last } if t == transport && (first..=last).contains(&port))
    }
}

/// Above this many ports of one transport, a service opens wide (REQ-010-7).
pub const WIDE_PORTS: u32 = 1024;

/// A named service of the catalogue (REQ-010-5, D29).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    /// Key.
    pub key: Key,
    /// Its one flow type.
    pub flow_type: Key,
    /// Its entries, non-empty.
    pub entries: Vec<ServiceEntry>,
}

impl Service {
    /// Checks a service: exactly one flow type, at least one entry (REQ-010-5).
    ///
    /// # Errors
    /// Violations of `policy.service.flow_type` and `policy.service.entries_empty`.
    pub fn new(
        key: Key,
        flow_types: &[Key],
        entries: Vec<ServiceEntry>,
    ) -> Result<Self, Vec<Violation>> {
        let mut errors = Vec::new();
        if flow_types.len() != 1 {
            errors.push(Violation::new(
                &SERVICE_FLOW_TYPE,
                format!(
                    "le service {key} cite {} types de flux, il en faut un",
                    flow_types.len()
                ),
            ));
        }
        if entries.is_empty() {
            errors.push(Violation::new(
                &SERVICE_ENTRIES_EMPTY,
                format!("le service {key} n'a aucun couple transport et ports"),
            ));
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(Self {
            key,
            flow_type: flow_types[0].clone(),
            entries,
        })
    }

    /// Warnings of an accepted service: `policy.service.wide_ports` per transport that
    /// covers more than 1 024 distinct ports (REQ-010-7).
    #[must_use]
    pub fn warnings(&self) -> Vec<Violation> {
        let mut by_transport: BTreeMap<Transport, Vec<(u16, u16)>> = BTreeMap::new();
        for entry in &self.entries {
            if let ServiceEntry::Ports {
                transport,
                first,
                last,
            } = *entry
            {
                by_transport
                    .entry(transport)
                    .or_default()
                    .push((first, last));
            }
        }
        by_transport
            .into_iter()
            .filter_map(|(transport, mut ranges)| {
                ranges.sort_unstable();
                let mut count = 0u32;
                let mut end = 0u32;
                for (first, last) in ranges {
                    let (first, last) = (u32::from(first), u32::from(last));
                    if last > end {
                        count += last - first.max(end + 1) + 1;
                        end = last;
                    }
                }
                (count > WIDE_PORTS).then(|| {
                    Violation::new(
                        &SERVICE_WIDE_PORTS,
                        format!(
                            "le service {} couvre {count} ports {transport}, plus de {WIDE_PORTS}",
                            self.key
                        ),
                    )
                })
            })
            .collect()
    }

    /// Whether one of its entries covers a port of a transport.
    #[must_use]
    pub fn covers_port(&self, transport: Transport, port: u16) -> bool {
        self.entries.iter().any(|e| e.covers_port(transport, port))
    }
}

/// A selector of endpoints: for each cited axis, the values admitted (AND between axes,
/// OR between the values of an axis); empty = any endpoint (REQ-010-8).
pub type Selector = BTreeMap<AxisKey, BTreeSet<AxisValue>>;

/// A selector of flows: flow types or services, never both empty (REQ-010-8, Q19).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowSelector {
    /// Flow types: every service of these types.
    pub flow_types: BTreeSet<Key>,
    /// Services.
    pub services: BTreeSet<Key>,
}

/// Effect of a rule (S23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Effect {
    /// Allow.
    Allow,
    /// Deny: wins over every authorisation (D28).
    Deny,
}

pub use kartenn_core::labels::Locale;

/// Checks the language tag of a pedagogical text (`policy.text.invalid`): BCP 47, in any
/// case, kept in lower case.
///
/// # Errors
/// A violation of `policy.text.invalid`.
pub fn locale(tag: &str) -> Result<Locale, Violation> {
    Locale::parse(tag).ok_or_else(|| {
        Violation::new(
            &TEXT_INVALID,
            format!("{tag:?} n'est pas une étiquette de langue"),
        )
    })
}

/// Longest body of a pedagogical text, in characters.
pub const TEXT_MAX_CHARS: usize = 4000;

/// A pedagogical text in one language: body and optional documentation link (S29).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    /// Body: plain text, line breaks allowed.
    pub body: String,
    /// Link to a documentation (`https://` or `http://`).
    pub url: Option<String>,
}

impl Text {
    /// Normalises (NFC) and checks a text (`policy.text.invalid`).
    ///
    /// # Errors
    /// A violation of `policy.text.invalid`.
    pub fn new(body: &str, url: Option<&str>) -> Result<Self, Violation> {
        let body: String = body.nfc().collect();
        let len = body.chars().count();
        let bad = len == 0
            || len > TEXT_MAX_CHARS
            || body.chars().any(|c| c.is_control() && c != '\n')
            || url.is_some_and(|u| {
                !(u.starts_with("https://") || u.starts_with("http://"))
                    || u.chars().any(char::is_whitespace)
            });
        if bad {
            return Err(Violation::new(
                &TEXT_INVALID,
                format!("texte de {len} caractères ou lien invalide"),
            ));
        }
        Ok(Self {
            body,
            url: url.map(str::to_owned),
        })
    }
}

/// The pedagogical texts of an object, by language (S29).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Texts(pub BTreeMap<Locale, Text>);

impl Texts {
    /// The text in `locale`, else in `default`, with the language actually rendered
    /// (REQ-010-33).
    #[must_use]
    pub fn get<'a>(&'a self, locale: &Locale, default: &Locale) -> Option<(&'a Locale, &'a Text)> {
        self.0
            .get_key_value(locale)
            .or_else(|| self.0.get_key_value(default))
    }

    /// Checks that a text exists in the default language (`policy.text.missing`).
    ///
    /// # Errors
    /// A violation of `policy.text.missing` naming `what`.
    pub fn require(&self, default: &Locale, what: &str) -> Result<(), Violation> {
        if self.0.contains_key(default) {
            Ok(())
        } else {
            Err(Violation::new(
                &TEXT_MISSING,
                format!("{what} sans texte pédagogique en {}", default.as_str()),
            ))
        }
    }
}

/// A rule of the policy (REQ-010-8, S23). Rules have no order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// Key.
    pub key: Key,
    /// Selector of the source.
    pub source: Selector,
    /// Selector of the destination.
    pub destination: Selector,
    /// Axes on which source and destination share at least one value.
    pub same: BTreeSet<AxisKey>,
    /// Selector of flows, non-empty.
    pub flows: FlowSelector,
    /// Effect.
    pub effect: Effect,
    /// Patterns of which one must match (allow only, S25).
    pub required_patterns: BTreeSet<Key>,
    /// The pattern the rule belongs to, if any (S24).
    pub pattern: Option<Key>,
    /// Pedagogical texts (required on a deny).
    pub texts: Texts,
}

impl Rule {
    /// Checks a rule against the designated axes and the default language
    /// (REQ-010-8, 29, 32, 33).
    ///
    /// # Errors
    /// Violations of `policy.rule.flow_selector_empty`, `policy.rule.unknown_axis`,
    /// `policy.rule.term_empty`, `policy.rule.conditional_deny` and `policy.text.missing`.
    pub fn check(&self, axes: &BTreeSet<AxisKey>, default: &Locale) -> Result<(), Vec<Violation>> {
        let mut errors = Vec::new();
        let key = &self.key;
        if self.flows.flow_types.is_empty() && self.flows.services.is_empty() {
            errors.push(Violation::new(
                &RULE_FLOW_SELECTOR_EMPTY,
                format!("la règle {key} ne cite ni type de flux ni service"),
            ));
        }
        for (side, selector) in [("source", &self.source), ("destination", &self.destination)] {
            for (axis, values) in selector {
                if !axes.contains(axis) {
                    errors.push(Violation::new(
                        &RULE_UNKNOWN_AXIS,
                        format!("la règle {key} cite {axis} ({side}), qui n'est pas un axe"),
                    ));
                }
                if values.is_empty() {
                    errors.push(Violation::new(
                        &RULE_TERM_EMPTY,
                        format!("la règle {key} cite {axis} ({side}) sans valeur"),
                    ));
                }
            }
        }
        for axis in &self.same {
            if !axes.contains(axis) {
                errors.push(Violation::new(
                    &RULE_UNKNOWN_AXIS,
                    format!("la règle {key} exige la même valeur de {axis}, qui n'est pas un axe"),
                ));
            }
        }
        if self.effect == Effect::Deny {
            if !self.required_patterns.is_empty() {
                errors.push(Violation::new(
                    &RULE_CONDITIONAL_DENY,
                    format!("la règle {key} interdit et exige des motifs"),
                ));
            }
            if let Err(v) = self
                .texts
                .require(default, &format!("l'interdiction {key}"))
            {
                errors.push(v);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Number of criteria: terms of both selectors and « same value » constraints (S23).
    #[must_use]
    pub fn criteria(&self) -> usize {
        self.source.len() + self.destination.len() + self.same.len()
    }

    /// Number of values admitted by the terms: fewer admitted values, more specific.
    #[must_use]
    pub fn admitted(&self) -> usize {
        self.source
            .values()
            .chain(self.destination.values())
            .map(BTreeSet::len)
            .sum()
    }

    /// Every axis the rule cites.
    pub fn axes(&self) -> impl Iterator<Item = &AxisKey> {
        self.source
            .keys()
            .chain(self.destination.keys())
            .chain(self.same.iter())
    }
}

/// An architecture pattern: a named group of rules (REQ-010-9, S24).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// Key.
    pub key: Key,
    /// Pedagogical texts (required).
    pub texts: Texts,
}

impl Pattern {
    /// Checks the text of a pattern (REQ-010-33).
    ///
    /// # Errors
    /// A violation of `policy.text.missing`.
    pub fn check(&self, default: &Locale) -> Result<(), Violation> {
        self.texts
            .require(default, &format!("le motif {}", self.key))
    }
}

/// A field of the file joined to a request (REQ-010-38, Q3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DossierField {
    /// External references (tickets, changes).
    ExternalRefs,
    /// Requester, or on behalf of whom.
    Requester,
    /// Owners of the endpoints.
    Owners,
    /// Nature of the data.
    DataNature,
    /// Period of the flow.
    Period,
}

/// Kind of an admissibility condition, a closed list extended by code (REQ-010-36).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionKind {
    /// A field of the file is present.
    DossierField(DossierField),
    /// Each endpoint holds at most this many addresses.
    MaxWidth(u128),
    /// No endpoint is a whole domain.
    NotWholeDomain,
    /// The request names a service rather than a raw port.
    NamedService,
    /// The highest rank of the file's values on the axis is at most the highest rank of
    /// the destination's values on that axis.
    DossierRankAtMost(AxisKey),
}

/// An admissibility condition (REQ-010-36, S30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    /// Key.
    pub key: Key,
    /// Kind and parameters.
    pub kind: ConditionKind,
    /// Applies when the decisive rule is one of these…
    pub rules: BTreeSet<Key>,
    /// … or the service is of one of these types; both empty = every evaluation.
    pub flow_types: BTreeSet<Key>,
    /// Pedagogical texts (required).
    pub texts: Texts,
}

impl Condition {
    /// Checks the parameters and the text of a condition (REQ-010-33, 36).
    ///
    /// # Errors
    /// Violations of `policy.condition.params_invalid` and `policy.text.missing`.
    pub fn check(
        &self,
        designated: &BTreeSet<AxisKey>,
        default: &Locale,
    ) -> Result<(), Vec<Violation>> {
        let mut errors = Vec::new();
        match &self.kind {
            ConditionKind::MaxWidth(0) => errors.push(Violation::new(
                &CONDITION_PARAMS_INVALID,
                format!("la condition {} borne les extrémités à 0 adresse", self.key),
            )),
            ConditionKind::DossierRankAtMost(axis) if !designated.contains(axis) => {
                errors.push(Violation::new(
                    &CONDITION_PARAMS_INVALID,
                    format!(
                        "la condition {} compare sur {axis}, qui n'est pas un axe",
                        self.key
                    ),
                ));
            }
            _ => {}
        }
        if let Err(v) = self
            .texts
            .require(default, &format!("la condition {}", self.key))
        {
            errors.push(v);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Kind of a signal, a closed list extended by code (REQ-010-37, Q20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalKind {
    /// An endpoint holds at least this many addresses.
    WideEndpoint(u128),
    /// The service is of one of these flow types.
    FlowType(BTreeSet<Key>),
    /// The service is one of these (services in clear, for instance).
    Service(BTreeSet<Key>),
    /// Source and destination carry different sets on the axis.
    AxisDiffers(AxisKey),
    /// The destination ranks higher than the source on the axis.
    DestinationHigherRank(AxisKey),
    /// An endpoint carries one of these values on the axis (exposed to internet…).
    EndpointValue(AxisKey, BTreeSet<AxisValue>),
    /// The authorisation only holds through a transverse value.
    TransverseOnly,
    /// An endpoint carries a value of at least this rank on the axis.
    RankAtLeast(AxisKey, i32),
}

/// A signal: a named, weighted predicate; the sum of the weights is the score (S32).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    /// Key.
    pub key: Key,
    /// Kind and parameters.
    pub kind: SignalKind,
    /// Weight.
    pub weight: i32,
}

impl Signal {
    /// Checks the parameters of a signal (REQ-010-37).
    ///
    /// # Errors
    /// A violation of `policy.signal.params_invalid`.
    pub fn check(&self, designated: &BTreeSet<AxisKey>) -> Result<(), Violation> {
        let cited = match &self.kind {
            SignalKind::AxisDiffers(a)
            | SignalKind::DestinationHigherRank(a)
            | SignalKind::RankAtLeast(a, _) => Some(a),
            SignalKind::EndpointValue(a, values) => {
                if values.is_empty() {
                    return Err(self.invalid("aucune valeur"));
                }
                Some(a)
            }
            SignalKind::FlowType(set) | SignalKind::Service(set) if set.is_empty() => {
                return Err(self.invalid("ensemble vide"));
            }
            SignalKind::WideEndpoint(0) => return Err(self.invalid("seuil nul")),
            _ => None,
        };
        match cited {
            Some(a) if !designated.contains(a) => {
                Err(self.invalid(&format!("{a} n'est pas un axe")))
            }
            _ => Ok(()),
        }
    }

    fn invalid(&self, why: &str) -> Violation {
        Violation::new(
            &SIGNAL_PARAMS_INVALID,
            format!("le signal {} : {why}", self.key),
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn key(text: &str) -> Key {
        Key::new(text).unwrap()
    }

    pub(crate) fn axis(text: &str) -> AxisKey {
        PropertyKey::parse(text).unwrap()
    }

    pub(crate) fn fr() -> Locale {
        Locale::parse("fr").unwrap()
    }

    fn entry(transport: &str, ports: Option<(i64, i64)>) -> Result<ServiceEntry, Violation> {
        ServiceEntry::new(&RawEntry {
            transport,
            ports,
            ..RawEntry::default()
        })
    }

    #[test]
    fn keys_follow_the_identifier_syntax() {
        for good in ["ssh", "https-admin", "a", "x1"] {
            assert!(Key::new(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            "-ssh",
            "ssh-",
            "ss--h",
            "SSH",
            "ssh_admin",
            &"a".repeat(64),
        ] {
            assert_eq!(
                Key::new(bad).unwrap_err().rule.id,
                "policy.key.syntax",
                "{bad}"
            );
        }
    }

    #[test]
    fn services_of_req_010_5_are_accepted() {
        let ssh = Service::new(
            key("ssh"),
            &[key("admin")],
            vec![entry("tcp", Some((22, 22))).unwrap()],
        )
        .unwrap();
        assert!(ssh.covers_port(Transport::Tcp, 22));
        assert!(!ssh.covers_port(Transport::Udp, 22));
        let rdp = vec![
            entry("tcp", Some((3389, 3389))).unwrap(),
            entry("udp", Some((3389, 3389))).unwrap(),
        ];
        assert!(Service::new(key("rdp"), &[key("admin")], rdp).is_ok());
    }

    #[test]
    fn each_refused_case_of_req_010_5() {
        let rule = |r: Result<ServiceEntry, Violation>| r.unwrap_err().rule.id;
        assert_eq!(
            rule(entry("tcp", Some((0, 0)))),
            "policy.service.entry_invalid"
        );
        assert_eq!(
            rule(entry("tcp", Some((70_000, 70_000)))),
            "policy.service.entry_invalid"
        );
        assert_eq!(
            rule(entry("tcp", Some((80, 22)))),
            "policy.service.entry_invalid"
        );
        assert_eq!(rule(entry("tcp", None)), "policy.service.entry_invalid");
        assert_eq!(
            rule(entry("icmp", Some((1, 1)))),
            "policy.service.entry_invalid"
        );
        assert_eq!(rule(entry("gre", None)), "policy.service.entry_invalid");
        let code_without_type = ServiceEntry::new(&RawEntry {
            transport: "icmp",
            icmp_code: Some(0),
            ..RawEntry::default()
        });
        assert_eq!(rule(code_without_type), "policy.service.entry_invalid");
        let type_256 = ServiceEntry::new(&RawEntry {
            transport: "icmpv6",
            icmp_type: Some(256),
            ..RawEntry::default()
        });
        assert_eq!(rule(type_256), "policy.service.entry_invalid");
        let ip_without_protocol = ServiceEntry::new(&RawEntry {
            transport: "ip",
            ..RawEntry::default()
        });
        assert_eq!(rule(ip_without_protocol), "policy.service.entry_invalid");
        assert_eq!(
            ServiceEntry::new(&RawEntry {
                transport: "ip",
                protocol: Some(47),
                ..RawEntry::default()
            }),
            Ok(ServiceEntry::Protocol(47))
        );
        let two_types = Service::new(
            key("x"),
            &[key("admin"), key("application")],
            vec![entry("tcp", Some((1, 1))).unwrap()],
        )
        .unwrap_err();
        assert_eq!(two_types[0].rule.id, "policy.service.flow_type");
        let no_entry = Service::new(key("x"), &[key("admin")], vec![]).unwrap_err();
        assert_eq!(no_entry[0].rule.id, "policy.service.entries_empty");
    }

    #[test]
    fn a_service_over_1024_ports_warns_once_per_transport() {
        let wide = Service::new(
            key("wide"),
            &[key("application")],
            vec![
                entry("tcp", Some((1000, 1600))).unwrap(),
                entry("tcp", Some((1500, 2100))).unwrap(),
                entry("udp", Some((1, 1024))).unwrap(),
            ],
        )
        .unwrap();
        let warnings = wide.warnings();
        assert_eq!(warnings.len(), 1, "tcp: 1101 ports, udp: exactly 1024");
        assert_eq!(warnings[0].rule.id, "policy.service.wide_ports");
        assert!(warnings[0].message.contains("1101"));
    }

    fn rule(effect: Effect) -> Rule {
        Rule {
            key: key("r"),
            source: Selector::from([(axis("policy.environment"), BTreeSet::from(["dev".into()]))]),
            destination: Selector::new(),
            same: BTreeSet::new(),
            flows: FlowSelector {
                flow_types: BTreeSet::from([key("admin")]),
                services: BTreeSet::new(),
            },
            effect,
            required_patterns: BTreeSet::new(),
            pattern: None,
            texts: Texts::default(),
        }
    }

    #[test]
    fn each_refused_case_of_req_010_8() {
        let axes = BTreeSet::from([axis("policy.environment")]);
        let ids = |r: Rule| -> Vec<&'static str> {
            r.check(&axes, &fr())
                .unwrap_err()
                .iter()
                .map(|v| v.rule.id)
                .collect()
        };
        assert!(rule(Effect::Allow).check(&axes, &fr()).is_ok());

        let mut empty_flows = rule(Effect::Allow);
        empty_flows.flows = FlowSelector::default();
        assert_eq!(ids(empty_flows), ["policy.rule.flow_selector_empty"]);

        let mut unknown = rule(Effect::Allow);
        unknown
            .destination
            .insert(axis("custom.colour"), BTreeSet::from(["red".into()]));
        assert_eq!(ids(unknown), ["policy.rule.unknown_axis"]);

        let mut same_unknown = rule(Effect::Allow);
        same_unknown.same.insert(axis("custom.colour"));
        assert_eq!(ids(same_unknown), ["policy.rule.unknown_axis"]);

        let mut empty_term = rule(Effect::Allow);
        empty_term
            .source
            .insert(axis("policy.environment"), BTreeSet::new());
        assert_eq!(ids(empty_term), ["policy.rule.term_empty"]);

        let mut conditional_deny = rule(Effect::Deny);
        conditional_deny.required_patterns.insert(key("relay"));
        conditional_deny
            .texts
            .0
            .insert(fr(), Text::new("Non.", None).unwrap());
        assert_eq!(ids(conditional_deny), ["policy.rule.conditional_deny"]);
    }

    #[test]
    fn a_deny_a_pattern_and_a_condition_need_a_text() {
        let axes = BTreeSet::from([axis("policy.environment")]);
        let deny = rule(Effect::Deny).check(&axes, &fr()).unwrap_err();
        assert_eq!(deny[0].rule.id, "policy.text.missing");
        let mut with_text = rule(Effect::Deny);
        with_text
            .texts
            .0
            .insert(fr(), Text::new("Passer par le bastion.", None).unwrap());
        assert!(with_text.check(&axes, &fr()).is_ok());
        let pattern = Pattern {
            key: key("relay"),
            texts: Texts::default(),
        };
        assert_eq!(
            pattern.check(&fr()).unwrap_err().rule.id,
            "policy.text.missing"
        );
        let condition = Condition {
            key: key("max-width"),
            kind: ConditionKind::MaxWidth(256),
            rules: BTreeSet::new(),
            flow_types: BTreeSet::new(),
            texts: Texts::default(),
        };
        assert_eq!(
            condition.check(&axes, &fr()).unwrap_err()[0].rule.id,
            "policy.text.missing"
        );
    }

    #[test]
    fn texts_fall_back_to_the_default_language() {
        let mut texts = Texts::default();
        texts.0.insert(
            fr(),
            Text::new("Bonne pratique.", Some("https://doc.invalid/x")).unwrap(),
        );
        let en = Locale::parse("en").unwrap();
        let (lang, text) = texts.get(&en, &fr()).unwrap();
        assert_eq!(lang, &fr());
        assert_eq!(text.body, "Bonne pratique.");
        assert!(Text::new("", None).is_err());
        assert!(Text::new("x", Some("ftp://x")).is_err());
        assert!(locale("fr-FR").is_ok() && locale("fr_FR").is_err());
    }

    #[test]
    fn condition_and_signal_parameters_are_checked() {
        let axes = BTreeSet::from([axis("policy.environment")]);
        let mut texts = Texts::default();
        texts
            .0
            .insert(fr(), Text::new("Restreindre.", None).unwrap());
        let zero = Condition {
            key: key("max-width"),
            kind: ConditionKind::MaxWidth(0),
            rules: BTreeSet::new(),
            flow_types: BTreeSet::new(),
            texts,
        };
        assert_eq!(
            zero.check(&axes, &fr()).unwrap_err()[0].rule.id,
            "policy.condition.params_invalid"
        );
        let signal = Signal {
            key: key("differs"),
            kind: SignalKind::AxisDiffers(axis("custom.colour")),
            weight: 2,
        };
        assert_eq!(
            signal.check(&axes).unwrap_err().rule.id,
            "policy.signal.params_invalid"
        );
        let empty = Signal {
            key: key("types"),
            kind: SignalKind::FlowType(BTreeSet::new()),
            weight: 1,
        };
        assert!(empty.check(&axes).is_err());
    }
}
