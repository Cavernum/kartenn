//! Validation rules: stable identifiers, severities, sources (REQ-002-35 to 37).
//!
//! Every rule of the core is declared once in [`CORE_RULES`]; the published catalogue
//! (`docs/spec/regles-noyau.md`) is generated from it and checked by a test, so code and
//! documentation cannot drift.

use std::fmt::{self, Write as _};

use uuid::Uuid;

/// Severity of a rule (002/A4): fixed by the specification, never configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The whole changeset is refused.
    Error,
    /// The changeset is accepted; an anomaly is recorded.
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Error => "erreur",
            Self::Warning => "avertissement",
        })
    }
}

/// A validation rule.
#[derive(Debug, PartialEq, Eq)]
pub struct Rule {
    /// Stable identifier, `<namespace>.<object>.<rule>`; never reused for another meaning.
    pub id: &'static str,
    /// Severity.
    pub severity: Severity,
    /// What the rule guarantees, in one sentence.
    pub summary: &'static str,
    /// Requirements served.
    pub requirements: &'static [&'static str],
    /// Normative source (RFC section or decision).
    pub source: &'static str,
}

/// A rule that fired on some objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The rule.
    pub rule: &'static Rule,
    /// Objects in cause (may be empty for a value that is not stored yet).
    pub subjects: Vec<Uuid>,
    /// Human-readable explanation, citing the values involved.
    pub message: String,
}

impl Violation {
    /// A violation of `rule` with a message.
    #[must_use]
    pub fn new(rule: &'static Rule, message: impl Into<String>) -> Self {
        Self {
            rule,
            subjects: Vec::new(),
            message: message.into(),
        }
    }

    /// Adds objects in cause.
    #[must_use]
    pub fn with_subjects(mut self, subjects: impl IntoIterator<Item = Uuid>) -> Self {
        self.subjects.extend(subjects);
        self
    }

    /// Whether the violation refuses the changeset.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.rule.severity == Severity::Error
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.rule.id, self.message)
    }
}

macro_rules! rules {
    ($($name:ident = $id:literal, $sev:ident, [$($req:literal),*], $source:literal, $summary:literal;)*) => {
        $(
            #[doc = $summary]
            pub static $name: Rule = Rule {
                id: $id,
                severity: Severity::$sev,
                summary: $summary,
                requirements: &[$($req),*],
                source: $source,
            };
        )*
        /// Every rule of the core, in catalogue order.
        pub static CORE_RULES: &[&Rule] = &[$(&$name),*];
    };
}

rules! {
    ADDR_SYNTAX = "core.addr.syntax", Error, ["REQ-002-2", "REQ-002-3"], "RFC 6943 §3.1.1, RFC 4291 §2.2",
        "Une adresse est écrite sous la forme stricte de sa famille.";
    ADDR_ZONE = "core.addr.zone", Error, ["REQ-002-4"], "RFC 4007 §6",
        "Une adresse ou un préfixe ne porte pas d'identifiant de zone.";
    ADDR_IPV4_MAPPED = "core.addr.ipv4_mapped", Error, ["REQ-002-31"], "002/A4",
        "Aucun préfixe ni adresse IPv6 IPv4-mapped n'est stocké ; on utilise la valeur IPv4.";
    PREFIX_SYNTAX = "core.prefix.syntax", Error, ["REQ-002-5"], "RFC 4632 §3.1",
        "Un préfixe s'écrit adresse/longueur, la longueur explicite et dans la largeur de la famille.";
    PREFIX_UNALIGNED = "core.prefix.unaligned", Error, ["REQ-002-5"], "002/design",
        "Un préfixe n'a aucun bit d'hôte à 1.";
    PREFIX_DUPLICATE = "core.prefix.duplicate", Error, ["REQ-002-15"], "D34",
        "Un préfixe existe au plus une fois par domaine sur toute période.";
    PREFIX_NETWORK_NOT_LEAF = "core.prefix.network_not_leaf", Error, ["REQ-002-16", "REQ-002-17"], "002/A2",
        "Un réseau ne contient aucun autre préfixe.";
    PREFIX_NOT_EMPTY_FOR_NATURE = "core.prefix.not_empty_for_nature", Error, ["REQ-002-17"], "002/A2",
        "Un préfixe ne change de nature que si son contenu le permet.";
    PREFIX_P2P_LENGTH = "core.prefix.p2p_length", Error, ["REQ-002-19"], "RFC 3021, RFC 6164",
        "Seuls les réseaux /31 et /32 (IPv4) ou /127 et /128 (IPv6) sont point à point.";
    PREFIX_SITE_ON_CONTAINER = "core.prefix.site_on_container", Error, ["REQ-002-20"], "002/A7",
        "Seuls les réseaux sont rattachés à un site.";
    RANGE_BOUNDS = "core.range.bounds", Error, ["REQ-002-21"], "002/design",
        "Une plage a des bornes d'une même famille, la première au plus égale à la dernière.";
    RANGE_OUTSIDE_NETWORK = "core.range.outside_network", Error, ["REQ-002-21"], "002/A2",
        "Une plage est entièrement contenue dans un seul réseau de son domaine.";
    RANGE_OVERLAP = "core.range.overlap", Error, ["REQ-002-22"], "D34",
        "Les plages d'un réseau ne se chevauchent pas.";
    RANGE_RESERVED_BOUNDS = "core.range.reserved_bounds", Error, ["REQ-002-23"], "RFC 1122 §3.2.1.3, RFC 4291 §2.6.1",
        "Une plage exclut les adresses de réseau et de diffusion (IPv4) et l'IID nul (IPv6).";
    ADDRESS_OUTSIDE_NETWORK = "core.address.outside_network", Error, ["REQ-002-25"], "002/A2",
        "Une adresse appartient à exactement un réseau de son domaine.";
    ADDRESS_DUPLICATE = "core.address.duplicate", Error, ["REQ-002-26"], "D4",
        "Une adresse existe au plus une fois par domaine sur toute période.";
    ADDRESS_RANGE_USAGE = "core.address.range_usage", Error, ["REQ-002-28"], "002/A8",
        "Les adresses respectent l'usage de la plage où elles tombent.";
    ADDRESS_NOT_ASSIGNABLE = "core.address.not_assignable", Error, ["REQ-002-29"], "RFC 1122 §3.2.1.3, RFC 4291 §2.6.1, registres IANA",
        "Les adresses de réseau, de diffusion, d'IID nul, de boucle locale, non spécifiées, réservées et multicast ne sont pas attribuées.";
    PUBLIC_OVERLAP = "core.public.overlap", Error, ["REQ-002-32", "REQ-002-46"], "002/A3, 002/A11",
        "L'espace public est unique entre domaines, sauf adresses anycast partout.";
    DOMAIN_IMMUTABLE = "core.domain.immutable", Error, ["REQ-002-12"], "002/A1",
        "Un objet d'adressage ne change jamais de domaine.";
    ANCHOR_CYCLE = "core.anchor.cycle", Error, ["REQ-002-8", "REQ-002-9"], "002/design",
        "Les arbres des entités et des sites n'ont pas de cycle.";
    ANCHOR_IN_USE = "core.anchor.in_use", Error, ["REQ-002-10"], "D33",
        "Une entité ou un site n'est clos que si rien ne le référence.";
    SLUG_SYNTAX = "core.slug.syntax", Error, ["REQ-002-47"], "002/A9",
        "Un slug compte 1 à 63 caractères [a-z0-9-], sans tiret au début ni à la fin.";
    SLUG_DUPLICATE = "core.slug.duplicate", Error, ["REQ-002-47"], "002/A9",
        "Un slug est unique par type d'objet à tout instant.";
    EXTERNAL_ID_DUPLICATE = "core.external_id.duplicate", Error, ["REQ-002-41"], "002/A9",
        "Un identifiant externe désigne un seul objet par source.";
    ADDR_DOCUMENTATION = "core.addr.documentation", Warning, ["REQ-002-35"], "RFC 5737, RFC 3849, RFC 9637",
        "Les blocs de documentation ne servent pas en production.";
    ADDR_SHARED_SPACE = "core.addr.shared_space", Warning, ["REQ-002-35"], "RFC 6598",
        "L'espace partagé est destiné au NAT d'opérateur (CGN).";
    PREFIX_IPV6_LONG_SUBNET = "core.prefix.ipv6_long_subnet", Warning, ["REQ-002-35"], "RFC 7136 §5",
        "Les réseaux d'hôtes IPv6 sont des /64, sauf point à point /127 et /128.";
    PREFIX_ULA_GLOBAL_ID = "core.prefix.ula_global_id", Warning, ["REQ-002-35"], "RFC 4193 §3.2",
        "Un préfixe unique local utilise un Global ID pseudo-aléatoire.";
    PUBLIC_ANYCAST_SHARED = "core.public.anycast_shared", Warning, ["REQ-002-46"], "002/A11",
        "Une adresse publique anycast figure dans plusieurs domaines.";
}

/// The rule of `id`, if the core defines it.
#[must_use]
pub fn rule(id: &str) -> Option<&'static Rule> {
    CORE_RULES.iter().copied().find(|r| r.id == id)
}

/// The catalogue of the core rules as Markdown (REQ-002-37).
#[must_use]
pub fn catalogue_markdown() -> String {
    let mut out = String::from(
        "<!-- Généré par kartenn-core (rules::catalogue_markdown) : ne pas modifier à la main. -->\n\n\
         # Catalogue des règles du noyau\n\n\
         Identifiants stables (REQ-002-37) : un identifiant n'est jamais réutilisé pour un autre sens.\n\
         Sévérités fixées par la spécification (002/A4).\n\n\
         | Identifiant | Sévérité | Règle | Exigences | Source |\n\
         |---|---|---|---|---|\n",
    );
    for r in CORE_RULES {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} | {} |",
            r.id,
            r.severity,
            r.summary,
            r.requirements.join(", "),
            r.source
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn identifiers_are_unique_and_well_formed() {
        let ids: BTreeSet<_> = CORE_RULES.iter().map(|r| r.id).collect();
        assert_eq!(ids.len(), CORE_RULES.len(), "duplicate rule identifier");
        for id in ids {
            let parts: Vec<_> = id.split('.').collect();
            assert!(parts.len() == 3 && parts[0] == "core", "{id}");
            assert!(
                id.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'.'
                    || b == b'_'),
                "{id}"
            );
        }
    }

    #[test]
    fn the_design_catalogue_is_complete() {
        assert_eq!(CORE_RULES.len(), 30);
        assert_eq!(
            CORE_RULES
                .iter()
                .filter(|r| r.severity == Severity::Warning)
                .count(),
            5
        );
        assert_eq!(rule("core.prefix.duplicate"), Some(&PREFIX_DUPLICATE));
        assert_eq!(rule("core.nope.nope"), None);
    }

    #[test]
    fn the_published_catalogue_matches_the_code() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/spec/regles-noyau.md"
        );
        let generated = catalogue_markdown();
        if std::env::var_os("KARTENN_BLESS").is_some() {
            std::fs::write(path, &generated).expect("write the catalogue");
        }
        let published = std::fs::read_to_string(path).unwrap_or_default();
        assert_eq!(
            published, generated,
            "docs/spec/regles-noyau.md is stale: run `KARTENN_BLESS=1 cargo test -p kartenn-core rules`"
        );
    }

    #[test]
    fn violations_carry_rule_and_subjects() {
        let id = Uuid::now_v7();
        let v = Violation::new(&PREFIX_DUPLICATE, "10.0.0.0/8 already exists").with_subjects([id]);
        assert!(v.is_error());
        assert_eq!(v.subjects, vec![id]);
        assert_eq!(
            v.to_string(),
            "[core.prefix.duplicate] 10.0.0.0/8 already exists"
        );
    }
}
