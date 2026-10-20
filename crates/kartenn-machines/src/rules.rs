//! Rules of the `machines` module (009 design, « Règles »). Identifiers and severities are
//! fixed by the specification; local checks live next to the values they guard, rules of
//! the final state in the module handler.

use kartenn_core::rules::{Rule, Severity};

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
        /// Rules of the module, in catalogue order.
        pub static MACHINES_RULES: &[&Rule] = &[$(&$name),*];
    };
}

rules! {
    NAME_INVALID = "machines.name.invalid", Error, ["REQ-009-4"], "RFC 1123 §2.1",
        "Un nom de machine est une suite d'étiquettes LDH de 1 à 63 caractères, 253 au total, conservée en minuscules.";
    NAME_DUPLICATE = "machines.name.duplicate", Error, ["REQ-009-4"], "009/M3",
        "Deux machines vivantes de même propriétaire résolu ne portent pas le même nom sur des périodes qui se chevauchent.";
    CLUSTER_DUPLICATE = "machines.cluster.duplicate", Error, ["REQ-009-2"], "009/M6",
        "Un slug de cluster est unique à tout instant.";
    HOST_INVALID = "machines.host.invalid", Error, ["REQ-009-5"], "009/M5",
        "Une machine n'est hébergée que selon les combinaisons permises, jamais par elle-même ni en cycle, ni par un hébergeur clos.";
    INTERFACE_DUPLICATE = "machines.interface.duplicate", Error, ["REQ-009-6"], "009/M7",
        "Une machine ne porte pas deux interfaces de même nom au même instant.";
    MAC_INVALID = "machines.mac.invalid", Error, ["REQ-009-6"], "RFC 9542 §1.1, §2.1.1",
        "Une adresse MAC d'interface est un EUI-48 unicast écrit en hexadécimal.";
    VLAN_MODE = "machines.vlan.mode", Error, ["REQ-009-12"], "IEEE 802.1Q-2022, 009/M9",
        "Le mode 802.1Q d'une interface et ses VLAN sont cohérents, et ses VLAN vivants sur la période.";
    ADDRESS_TAKEN = "machines.address.taken", Error, ["REQ-009-10"], "009/M8",
        "Une adresse non partagée n'est rattachée qu'à une interface à la fois.";
    ADDRESS_PRIMARY = "machines.address.primary", Error, ["REQ-009-11"], "009/M8",
        "Une interface a au plus une adresse principale par famille.";
    ADDRESS_CLOSED = "machines.address.closed", Error, ["REQ-009-9"], "009/M8",
        "Une adresse rattachée est vivante sur toute la période du rattachement.";
    MAC_DUPLICATE = "machines.mac.duplicate", Warning, ["REQ-009-13"], "009/M11",
        "Une MAC universelle n'est portée que par une interface vivante.";
    ADDRESS_VLAN_MISMATCH = "machines.address.vlan_mismatch", Warning, ["REQ-009-13"], "009/M11",
        "Le réseau d'une adresse rattachée ne porte pas de VLAN étranger à l'interface.";
    HOST_SITE_MISMATCH = "machines.host.site_mismatch", Warning, ["REQ-009-13"], "009/M11",
        "Une machine hébergée et son hébergeur ont le même site résolu.";
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn rule_ids_unique_and_namespaced() {
        let mut seen = HashSet::new();
        for rule in MACHINES_RULES {
            assert!(rule.id.starts_with("machines."), "{}", rule.id);
            assert!(seen.insert(rule.id), "doublon {}", rule.id);
            assert!(
                kartenn_core::rules::rule(rule.id).is_none(),
                "{} collides with the core",
                rule.id
            );
        }
    }
}
