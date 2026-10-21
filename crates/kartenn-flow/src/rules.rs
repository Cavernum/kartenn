//! Validation rules of the flow registry (design 011 « Opérations et règles »).

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
    FLOW_RULES;
    ENDPOINT_INVALID = "flow.endpoint.invalid", Error, ["REQ-011-2"], "011/F3",
        "Une extrémité est une adresse, un préfixe ou, avec le module Machines, une interface ou une machine, valide sur toute la période du flux ; jamais une plage.";
    SERVICE_CLOSED = "flow.service.closed", Error, ["REQ-011-1"], "011/F2",
        "Le service d'un flux est actif sur toute sa période.";
    REFERENCE_MISSING = "flow.reference.missing", Error, ["REQ-011-1"], "011/F2, Q3",
        "Un flux porte au moins une référence externe.";
    FIELD_INVALID = "flow.field.invalid", Error, ["REQ-011-1"], "011/F2",
        "Une justification a de 1 à 4 000 caractères ; une référence externe a un identifiant de 1 à 256 caractères ; sans caractère de contrôle.";
    KIND_RESERVED = "flow.kind.reserved", Error, ["REQ-011-20"], "011/F16, Q18",
        "La sorte « constaté » est réservée à la v0.3.";
    UPDATE_IDENTITY = "flow.update.identity", Error, ["REQ-011-4"], "011/F2",
        "Changer une extrémité ou le service d'un flux en crée un nouveau : la modification est refusée.";
    REVIEW_REQUIRED = "flow.review.required", Error, ["REQ-011-8", "REQ-011-21"], "011/F5, F6",
        "Un flux dont l'issue n'est pas automatique ne s'applique qu'après instruction.";
    REVIEW_END_REQUIRED = "flow.review.end_required", Error, ["REQ-011-22"], "011/F17",
        "Un flux instruit a une date de fin, au plus au plafond de l'instruction.";
    WAIVER_REQUIRED = "flow.waiver.required", Error, ["REQ-011-8", "REQ-011-9"], "011/F6",
        "Un flux refusé par la politique ne s'enregistre ou ne se prolonge qu'avec une dérogation.";
    VERDICT_DENIED = "flow.verdict.denied", Warning, ["REQ-011-12", "REQ-011-25"], "011/F8",
        "Un flux enregistré que la politique refuse désormais, sans dérogation valide, est non conforme : anomalie, fermée quand il redevient conforme.";
    VERDICT_PARTIAL = "flow.verdict.partial", Warning, ["REQ-011-10"], "011/F6",
        "Une partie seulement des paires d'adresses du flux est autorisée.";
    WAIVER_INVALID = "flow.waiver.invalid", Error, ["REQ-011-9"], "011/F6, Q8",
        "Une dérogation a un motif et un risque accepté, une fin au plus tôt de celle du flux et du plafond, et un auteur autre que le demandeur.";
    SEPARATION = "flow.separation", Error, ["REQ-011-22"], "011/F17, C1",
        "L'instructeur d'une demande n'en est pas le demandeur.";
    REFERENCED = "flow.referenced", Error, ["REQ-011-3"], "011/F3, D33",
        "Un objet extrémité d'un flux actif ou à venir ne se clôt pas sans cascade explicite.";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_namespaced() {
        let mut ids: Vec<_> = FLOW_RULES.iter().map(|r| r.id).collect();
        assert!(ids.iter().all(|id| id.starts_with("flow.")));
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), FLOW_RULES.len());
    }
}
