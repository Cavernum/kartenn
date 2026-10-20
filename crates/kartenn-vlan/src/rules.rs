//! Validation rules of the VLAN module (design 008 « Règles »).

use kartenn_core::rules::{Rule, Severity};

macro_rules! rules {
    ($list:ident; $($name:ident = $id:literal, $sev:ident, [$($req:literal),*], $source:literal, $summary:literal;)*) => {
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
        pub static $list: &[&Rule] = &[$(&$name),*];
    };
}

rules! {
    VLAN_RULES;
    VID_INVALID = "vlan.vid.invalid", Error, ["REQ-008-12", "REQ-008-14"], "IEEE 802.1Q-2022 §9.6",
        "Un VID est un entier de 1 à 4094.";
    VID_OUT_OF_RANGE = "vlan.vid.out_of_range", Error, ["REQ-008-13", "REQ-008-14"], "008/V6",
        "Un VID est dans la plage de son groupe, et la plage d'un groupe garde ses VLAN.";
    VID_DUPLICATE = "vlan.vid.duplicate", Error, ["REQ-008-15"], "008/V2",
        "Un VID est vivant au plus une fois par groupe à tout instant.";
    NAME_INVALID = "vlan.name.invalid", Error, ["REQ-008-14"], "008/V7",
        "Un nom de VLAN a de 1 à 64 caractères, sans caractère de contrôle.";
    NAME_DUPLICATE = "vlan.name.duplicate", Error, ["REQ-008-15"], "008/V7",
        "Un nom de VLAN est vivant au plus une fois par groupe à tout instant.";
    GROUP_ANCHOR = "vlan.group.anchor", Error, ["REQ-008-12"], "008/V3",
        "Un groupe est ancré sur une entité, un site ou un domaine vivant sur toute sa période.";
    PREFIX_NOT_NETWORK = "vlan.prefix.not_network", Error, ["REQ-008-17"], "008/V4",
        "La propriété vlan.vlan ne se pose que sur un réseau.";
    SITE_MISMATCH = "vlan.site.mismatch", Warning, ["REQ-008-18"], "008/V9",
        "Le site d'un réseau et celui du groupe de son VLAN concordent.";
    DOMAIN_MISMATCH = "vlan.domain.mismatch", Warning, ["REQ-008-18"], "008/V9",
        "Le domaine d'un réseau et celui du groupe de son VLAN concordent.";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_namespaced() {
        let mut ids: Vec<_> = VLAN_RULES.iter().map(|r| r.id).collect();
        assert!(ids.iter().all(|id| id.starts_with("vlan.")));
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), VLAN_RULES.len());
    }
}
