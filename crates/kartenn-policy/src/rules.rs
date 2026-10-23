//! Validation rules of the policy module (design 010 « Opérations », table of rules).

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
    POLICY_RULES;
    KEY_SYNTAX = "policy.key.syntax", Error, ["REQ-010-4", "REQ-010-5", "REQ-010-8"], "010/S5",
        "Une clé de la politique a de 1 à 63 caractères [a-z0-9] et des traits d'union non doublés ni en bord.";
    SERVICE_ENTRY_INVALID = "policy.service.entry_invalid", Error, ["REQ-010-5"], "010/S5, RFC 6335 §6",
        "Un couple d'un service a un transport connu, des ports 1–65535 pour tcp, udp, sctp et dccp, un type et un code ICMP 0–255 sans port, un protocole IP 0–255 sans port.";
    SERVICE_FLOW_TYPE = "policy.service.flow_type", Error, ["REQ-010-5"], "010/S5, D29",
        "Un service a exactement un type de flux.";
    SERVICE_ENTRIES_EMPTY = "policy.service.entries_empty", Error, ["REQ-010-5"], "010/S5",
        "Un service a au moins un couple transport et ports.";
    SERVICE_WIDE_PORTS = "policy.service.wide_ports", Warning, ["REQ-010-7"], "010/S5",
        "Un service qui couvre plus de 1 024 ports d'un même transport ouvre large.";
    RULE_FLOW_SELECTOR_EMPTY = "policy.rule.flow_selector_empty", Error, ["REQ-010-8"], "010/S7, Q19",
        "Une règle cite au moins un type de flux ou un service.";
    RULE_UNKNOWN_AXIS = "policy.rule.unknown_axis", Error, ["REQ-010-8", "REQ-010-29"], "010/S22",
        "Une règle ne cite que des propriétés désignées comme axes de la politique.";
    RULE_TERM_EMPTY = "policy.rule.term_empty", Error, ["REQ-010-8"], "010/S23",
        "Un terme d'un sélecteur d'extrémité admet au moins une valeur.";
    RULE_CONDITIONAL_DENY = "policy.rule.conditional_deny", Error, ["REQ-010-8", "REQ-010-32"], "010/S25",
        "Seule une autorisation peut exiger des motifs.";
    MATRIX_STALE = "policy.matrix.stale", Error, ["REQ-010-34"], "010/S35",
        "L'écriture d'une cellule qui porte l'empreinte de la matrice de son type de flux ne s'applique que si la matrice n'a pas changé depuis.";
    TEXT_MISSING = "policy.text.missing", Error, ["REQ-010-33"], "010/S29",
        "Une interdiction, un motif et une condition portent un texte pédagogique dans la langue par défaut.";
    TEXT_INVALID = "policy.text.invalid", Error, ["REQ-010-33"], "010/S29",
        "Un texte pédagogique est non vide, sans caractère de contrôle hors saut de ligne, d'au plus 4 000 caractères.";
    CONDITION_PARAMS_INVALID = "policy.condition.params_invalid", Error, ["REQ-010-36"], "010/S30",
        "Les paramètres d'une condition d'admissibilité respectent le schéma de son genre.";
    SIGNAL_PARAMS_INVALID = "policy.signal.params_invalid", Error, ["REQ-010-37"], "010/S32",
        "Les paramètres d'un signal respectent le schéma de son genre.";
    INFERENCE_REGEX_INVALID = "policy.inference.regex_invalid", Error, ["REQ-010-42"], "010/I3",
        "Une expression régulière d'inférence est ancrée, d'au plus 256 caractères, dans la syntaxe du moteur à temps linéaire.";
    INFERENCE_DEPTH = "policy.inference.depth", Error, ["REQ-010-44"], "010/I5",
        "Les exceptions d'une règle d'inférence s'imbriquent sur 4 niveaux au plus.";
    INFERENCE_CRITERION_INVALID = "policy.inference.criterion_invalid", Error, ["REQ-010-42"], "010/I3",
        "Un critère d'inférence admet au moins une valeur, et une règle au moins une valeur attendue.";
    MACHINE_ENVIRONMENT_MISMATCH = "policy.machine.environment_mismatch", Warning, ["REQ-010-17"], "010/S10, D22",
        "Les adresses d'une machine, hors interfaces d'un rôle exempté, résolvent l'environnement qu'elle déclare, ou, sans déclaration, un même environnement.";
    INFERENCE_CONFLICT = "policy.inference.conflict", Warning, ["REQ-010-45"], "010/I6 adouci",
        "Des règles d'inférence de même priorité, au même nœud, attendent des ensembles différents.";
    INFERENCE_MISMATCH = "policy.inference.mismatch", Warning, ["REQ-010-48"], "010/I9, Q13",
        "Les héritiers d'une valeur déclarée de policy.environment portent la valeur que l'inférence attend ; l'anomalie, sur le nœud d'origine, compte les héritiers en écart.";
    OPERATION_INVALID = "policy.operation.invalid", Error, ["REQ-010-26"], "ADR 0026 clause 6",
        "Une opération de la politique a un corps conforme à son schéma et cite des objets connus.";
    AXIS_INVALID = "policy.axis.invalid", Error, ["REQ-010-29"], "010/S22",
        "Un axe est une propriété vivante de liste fermée ou de référence externe, portée par l'arbre d'adressage ou par la machine selon son porteur ; policy.environment est un axe imposé.";
    VALUE_UNKNOWN = "policy.value.unknown", Error, ["REQ-010-2", "REQ-010-30"], "010/S2",
        "Les attributs d'une valeur décrivent une valeur vivante d'un axe.";
    KEY_DUPLICATE = "policy.key.duplicate", Error, ["REQ-010-4", "REQ-010-5", "REQ-010-8"], "010/design « Données »",
        "La clé d'un objet de la politique est unique sur sa période parmi les objets de son type.";
    REFERENCE_CLOSED = "policy.reference.closed", Error, ["REQ-010-2", "REQ-010-8"], "010/design « Opérations »",
        "Un objet de la politique ne cite que des valeurs, types de flux, services, motifs et règles vivants sur sa période.";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_namespaced() {
        let mut ids: Vec<_> = POLICY_RULES.iter().map(|r| r.id).collect();
        assert!(ids.iter().all(|id| id.starts_with("policy.")));
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), POLICY_RULES.len());
    }
}
