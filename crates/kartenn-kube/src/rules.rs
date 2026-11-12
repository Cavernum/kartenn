//! Rules of the `kube` module (019 design, « Règles »). Identifiers and severities are fixed
//! by the specification; the local checks of names and labels live next to the values they
//! guard (`names`, `selector`, `tracked`), the rules of authorship and of the final state in
//! the module handler. `kube.observation.invalid` is not a rule: it is the Problem Details
//! code of a refused batch of observations, which never goes through the rules pipeline.

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
        pub static KUBE_RULES: &[&Rule] = &[$(&$name),*];
    };
}

rules! {
    OPERATION_INVALID = "kube.operation.invalid", Error, ["REQ-019-5", "REQ-019-6", "REQ-019-7"], "ADR 0026 clause 6",
        "Une opération du module porte les champs qu'il attend, avec des valeurs admises et des objets qui existent.";
    OBJECT_MANAGED = "kube.object.managed", Error, ["REQ-019-12"], "019/X5",
        "Les champs suivis d'un objet Kubernetes ne s'écrivent que par un compte de service membre du groupe du connecteur de son cluster.";
    CONNECTOR_DIRECT = "kube.connector.direct", Error, ["REQ-019-30"], "019/X10, ADR 0044",
        "Un membre d'un groupe de connecteur n'écrit que par l'opération kube.sync.";
    CLUSTER_RANGE_DOMAIN = "kube.cluster.range_domain", Error, ["REQ-019-5"], "019/X6",
        "Une plage d'un cluster est dans le domaine qu'il désigne pour elle.";
    CLUSTER_CONNECTOR_GROUP = "kube.cluster.connector_group", Error, ["REQ-019-5"], "019/X5, 025/G1",
        "Le groupe du connecteur d'un cluster existe et vit sur toute la période.";
    NODE_DUPLICATE = "kube.node.duplicate", Error, ["REQ-019-6"], "Kubernetes, Names",
        "Deux nœuds vivants d'un cluster ne portent pas le même nom sur des périodes qui se chevauchent.";
    NAMESPACE_DUPLICATE = "kube.namespace.duplicate", Error, ["REQ-019-7"], "Kubernetes, Namespaces",
        "Deux namespaces vivants d'un cluster ne portent pas le même nom sur des périodes qui se chevauchent.";
    WORKLOAD_DUPLICATE = "kube.workload.duplicate", Error, ["REQ-019-8"], "019/X3",
        "Deux workloads vivants d'un namespace ne portent pas la même sorte et le même nom sur des périodes qui se chevauchent.";
    SERVICE_DUPLICATE = "kube.service.duplicate", Error, ["REQ-019-9"], "Kubernetes, Names",
        "Deux services vivants d'un namespace ne portent pas le même nom sur des périodes qui se chevauchent.";
    INGRESS_DUPLICATE = "kube.ingress.duplicate", Error, ["REQ-019-10"], "Kubernetes, Names",
        "Deux ingress vivants d'un namespace ne portent pas le même nom sur des périodes qui se chevauchent.";
    NODE_MACHINE_KIND = "kube.node.machine_kind", Error, ["REQ-019-6"], "019/X2",
        "Un nœud désigne une machine physical ou virtual, que nul autre nœud vivant ne désigne.";
    NAME_INVALID = "kube.name.invalid", Error, ["REQ-019-6", "REQ-019-7", "REQ-019-8", "REQ-019-9", "REQ-019-10"], "Kubernetes, Names",
        "Un nom d'objet Kubernetes est une étiquette ou un sous-domaine DNS en minuscules.";
    LABEL_INVALID = "kube.label.invalid", Error, ["REQ-019-7", "REQ-019-8", "REQ-019-9"], "Kubernetes, Labels and Selectors",
        "Une clé de label, sa valeur et un sélecteur suivent la syntaxe de Kubernetes.";
    NODE_RANGE_OUTSIDE = "kube.node.range_outside", Warning, ["REQ-019-13"], "019/X6",
        "La plage de pods d'un nœud est dans un conteneur de plages de pods de son cluster.";
    NODE_UNMATCHED = "kube.node.unmatched", Warning, ["REQ-019-23"], "019/X9",
        "Un nœud désigne une machine trouvée sans ambiguïté.";
    OBJECT_ABSENT = "kube.object.absent", Warning, ["REQ-019-22"], "019/X9",
        "Un objet Kubernetes cité par un dépendant est présent dans son cluster.";
    INGRESS_BACKEND_MISSING = "kube.ingress.backend_missing", Warning, ["REQ-019-10"], "019 design, « Règles »",
        "Une règle d'ingress vise un service présent dans son namespace.";
    NAMESPACE_HELD = "kube.namespace.held", Warning, ["REQ-019-34"], "019/Q-019-35",
        "Un namespace parti de son cluster n'est plus cité par des valeurs admises.";
    LABEL_NOT_ADMITTED = "kube.label.not_admitted", Warning, ["REQ-019-34"], "019/X11",
        "La valeur d'axe d'un label est admise dans le namespace du workload qui le porte.";
    LABEL_CONFLICT = "kube.label.conflict", Warning, ["REQ-019-34"], "019/Q-019-40",
        "Les labels suivis d'un workload ne donnent pas deux valeurs différentes d'un axe à valeur simple.";
    NAMESPACE_MANAGED ="kube.namespace.managed", Error, ["REQ-019-33"], "019/Q-019-39",
        "L'axe kube.namespace d'un workload n'est affecté ou retiré que par kube.sync.";
    MACHINE_LOOKS_LIKE_POD = "kube.machine.looks_like_pod", Warning, ["REQ-019-3"], "019/X2",
        "Une machine container hébergée par la machine d'un nœud ne porte pas le nom d'un pod observé.";
    SYNC_STALE = "kube.sync.stale", Warning, ["REQ-019-18", "REQ-019-20"], "019/X8",
        "Un cluster reçoit un lot ou un signet de son connecteur au moins toutes les 10 minutes.";
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn rule_ids_unique_and_namespaced() {
        let mut seen = HashSet::new();
        for rule in KUBE_RULES {
            assert!(rule.id.starts_with("kube."), "{}", rule.id);
            assert_eq!(rule.id.split('.').count(), 3, "{}", rule.id);
            assert!(seen.insert(rule.id), "doublon {}", rule.id);
            assert!(
                kartenn_core::rules::rule(rule.id).is_none(),
                "{} collides with the core",
                rule.id
            );
            assert!(!rule.requirements.is_empty(), "{}", rule.id);
        }
        // The rules of the design table the later tasks enforce are all declared here.
        for id in [
            "kube.object.managed",
            "kube.connector.direct",
            "kube.cluster.connector_group",
            "kube.name.invalid",
            "kube.label.invalid",
            "kube.sync.stale",
        ] {
            assert!(seen.contains(id), "{id} missing");
        }
        // The 18 of the design, the malformed operation (task 019-3b), the duplicate
        // ingress (task 019-4c), the namespace held by admitted values (task 019-7c) and
        // the axis of the namespace managed by the server and the labels in conflict on an
        // axis (task 019-7d2).
        assert_eq!(KUBE_RULES.len(), 23);
    }
}
