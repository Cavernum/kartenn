//! Catalogue of audit events (spec 012, REQ-012-8, 30): name, severity, criticality,
//! details, requirements, and the matching OCSF class and ECS category. Every event kind
//! is declared once here; `docs/spec/evenements-audit.md` is generated from it and
//! checked by a test.

use std::fmt::Write as _;

use super::{Criticality, Severity};

/// A kind of audit event.
#[derive(Debug, PartialEq, Eq)]
pub struct EventKind {
    /// Stable name, `<domain>.<object>.<action>`; never reused for another meaning.
    pub name: &'static str,
    /// Severity (REQ-012-4).
    pub severity: Severity,
    /// Criticality, which sets the retention (012/L22).
    pub criticality: Criticality,
    /// What the event records, in one sentence.
    pub summary: &'static str,
    /// Keys allowed in `details`.
    pub details: &'static [&'static str],
    /// Requirements served.
    pub requirements: &'static [&'static str],
    /// OCSF class and activity, for the export mapping (indicative, OCSF 1.9).
    pub ocsf: &'static str,
    /// ECS `event.category` (ECS 9).
    pub ecs: &'static str,
}

macro_rules! events {
    ($($name:ident = $id:literal, $sev:ident, $crit:ident, [$($detail:literal),*], [$($req:literal),*], $ocsf:literal, $ecs:literal, $summary:literal;)*) => {
        $(
            #[doc = $summary]
            pub static $name: EventKind = EventKind {
                name: $id,
                severity: Severity::$sev,
                criticality: Criticality::$crit,
                summary: $summary,
                details: &[$($detail),*],
                requirements: &[$($req),*],
                ocsf: $ocsf,
                ecs: $ecs,
            };
        )*
        /// Every event kind, in catalogue order.
        pub static EVENTS: &[&EventKind] = &[$(&$name),*];
    };
}

events! {
    // Authentication (005 emits them).
    AUTHN_LOGIN_SUCCEEDED = "authn.login.succeeded", Info, Normal, ["method"], ["REQ-012-8"],
        "Authentication 3002 / Logon", "authentication",
        "Une identité s'est authentifiée.";
    AUTHN_LOGIN_FAILED = "authn.login.failed", Warn, High, ["method"], ["REQ-012-8", "REQ-012-9"],
        "Authentication 3002 / Logon", "authentication",
        "Une authentification a échoué ; l'identifiant revendiqué n'est conservé qu'en empreinte.";
    AUTHN_FAILURES_AGGREGATED = "authn.login.failures_aggregated", Warn, High, ["count", "window_seconds"], ["REQ-012-19"],
        "Authentication 3002 / Logon", "authentication",
        "Échecs répétés d'une même source agrégés sur une minute, avec leur nombre exact.";
    AUTHN_ACCOUNT_LOCKED = "authn.account.locked", Warn, High, [], ["REQ-012-8"],
        "Account Change 3001 / Lock", "iam",
        "Un compte a été verrouillé après des échecs répétés.";
    AUTHN_TOKEN_REUSED = "authn.token.reused", Warn, High, [], ["REQ-012-8"],
        "Authentication 3002 / Logon", "authentication",
        "Un jeton déjà utilisé ou révoqué a été présenté.";
    AUTHN_SESSION_CLOSED = "authn.session.closed", Info, Low, ["cause"], ["REQ-012-8"],
        "Authentication 3002 / Logoff", "authentication",
        "Une session s'est terminée (déconnexion ou expiration).";
    // Authorisation.
    AUTHZ_DENIED = "authz.access.denied", Warn, High, ["permission"], ["REQ-012-8"],
        "Authorize Session 3003 / Assign Privileges", "iam",
        "Un droit a été refusé à l'acteur sur les objets cibles.";
    // Writes: one event per state of a changeset, which carries the detail (D11).
    CHANGESET_SUBMITTED_PENDING = "changeset.state.pending", Info, Normal, ["kind"], ["REQ-012-8"],
        "API Activity 6003 / Create", "configuration",
        "Un changeset attend une approbation ou une validation de proposition.";
    CHANGESET_APPLIED = "changeset.state.applied", Info, Normal, ["operations"], ["REQ-012-8"],
        "API Activity 6003 / Update", "configuration",
        "Un changeset a été appliqué.";
    CHANGESET_REFUSED = "changeset.state.refused", Info, Normal, ["rules"], ["REQ-012-8"],
        "API Activity 6003 / Update", "configuration",
        "Un changeset a été refusé par la validation.";
    CHANGESET_WITHDRAWN = "changeset.state.withdrawn", Info, Normal, [], ["REQ-012-8"],
        "API Activity 6003 / Delete", "configuration",
        "Son auteur a retiré un changeset en attente.";
    CHANGESET_REJECTED = "changeset.state.rejected", Info, Normal, [], ["REQ-012-8"],
        "API Activity 6003 / Update", "configuration",
        "Un valideur a rejeté un changeset en attente.";
    CHANGESET_EXPIRED = "changeset.state.expired", Info, Normal, [], ["REQ-012-8"],
        "API Activity 6003 / Update", "configuration",
        "Un changeset en attente a expiré.";
    CHANGESET_REVALIDATION_FAILED = "changeset.state.stale", Warn, Normal, ["rules"], ["REQ-012-8"],
        "API Activity 6003 / Update", "configuration",
        "Un changeset en attente n'a pas passé sa revalidation au moment d'être validé.";
    // Administration.
    RBAC_ROLE_CHANGED = "rbac.role.changed", Info, High, ["role"], ["REQ-012-8"],
        "Entity Management 3004 / Update", "iam",
        "Un rôle ou ses permissions ont changé.";
    RBAC_BINDING_CHANGED = "rbac.binding.changed", Info, High, ["role", "scope"], ["REQ-012-8"],
        "Entity Management 3004 / Update", "iam",
        "Une liaison sujet × rôle × périmètre a été créée ou retirée.";
    SERVICE_ACCOUNT_CHANGED = "account.service.changed", Info, High, ["change"], ["REQ-012-8"],
        "Account Change 3001 / Create", "iam",
        "Un compte de service a été créé, renouvelé ou révoqué.";
    APPROVAL_POLICY_CHANGED = "policy.approval.changed", Info, High, ["enabled"], ["REQ-012-8"],
        "Entity Management 3004 / Update", "configuration",
        "Une politique d'approbation a été créée, modifiée ou désactivée.";
    DECLARATION_CHANGED = "property.declaration.changed", Info, Normal, ["key", "sensitive"], ["REQ-012-8", "REQ-003-27"],
        "Entity Management 3004 / Update", "configuration",
        "Une déclaration de propriété a été créée ou modifiée.";
    PERSON_PSEUDONYMISED = "person.identity.pseudonymised", Info, High, [], ["REQ-012-8"],
        "Account Change 3001 / Other", "iam",
        "Les données d'identité d'une personne ont été pseudonymisées.";
    // Data.
    OBJECTS_READ = "data.objects.read", Info, Low, ["criteria", "targets_omitted"], ["REQ-012-18"],
        "API Activity 6003 / Read", "database",
        "Une requête de lecture a renvoyé ces objets (jamais leurs valeurs).";
    SENSITIVE_READ = "data.sensitive.read", Info, High, ["property"], ["REQ-012-18"],
        "API Activity 6003 / Read", "database",
        "La valeur d'une propriété sensible a été lue.";
    OBJECTS_EXPORTED = "data.objects.exported", Info, High, ["criteria", "count"], ["REQ-012-8"],
        "API Activity 6003 / Read", "database",
        "Un export en masse a été produit.";
    AUDIT_READ = "audit.events.read", Info, High, ["criteria", "count"], ["REQ-012-17"],
        "API Activity 6003 / Read", "database",
        "Le journal d'audit a été consulté ou exporté.";
    // Operation.
    SYS_STARTED = "sys.instance.started", Info, Normal, ["version"], ["REQ-012-8"],
        "Application Lifecycle 6002 / Start", "process",
        "Une instance a démarré.";
    SYS_STOPPED = "sys.instance.stopped", Info, Normal, ["lost_technical_events"], ["REQ-012-8", "REQ-012-34"],
        "Application Lifecycle 6002 / Stop", "process",
        "Une instance s'est arrêtée.";
    SYS_MIGRATED = "sys.schema.migrated", Info, Normal, ["version"], ["REQ-012-8"],
        "Application Lifecycle 6002 / Update", "configuration",
        "Le job de migration a été exécuté.";
    SYS_CONFIG_CHANGED = "sys.config.changed", Info, High, ["setting"], ["REQ-012-8"],
        "Application Lifecycle 6002 / Update", "configuration",
        "Un réglage a changé.";
    SYS_LOG_FILTER_CHANGED = "sys.log_filter.changed", Info, High, ["output", "filter", "expires_at"], ["REQ-012-27"],
        "Application Lifecycle 6002 / Update", "configuration",
        "Le filtre d'une sortie technique a été modifié à chaud.";
    SYS_OUTPUT_UNAVAILABLE = "sys.output.unavailable", Warn, High, ["output"], ["REQ-012-26"],
        "Application Lifecycle 6002 / Other", "process",
        "Une sortie est inaccessible depuis plus d'une minute.";
    SYS_OUTPUT_AVAILABLE = "sys.output.available", Info, Normal, ["output"], ["REQ-012-26"],
        "Application Lifecycle 6002 / Other", "process",
        "Une sortie inaccessible est revenue.";
    SYS_OUTPUT_DISABLED = "sys.output.disabled", Warn, High, ["output"], ["REQ-012-26"],
        "Application Lifecycle 6002 / Other", "process",
        "Une sortie a été désactivée.";
    AUDIT_SEALED = "audit.seal.created", Info, Normal, ["seal", "count", "hash", "key_id"], ["REQ-012-13"],
        "Application Lifecycle 6002 / Other", "configuration",
        "Un lot d'événements d'audit a été scellé.";
    AUDIT_CHAIN_BROKEN = "audit.chain.broken", Error, High, ["seal"], ["REQ-012-14"],
        "Detection Finding 2004 / Create", "intrusion_detection",
        "La vérification de la chaîne des sceaux a trouvé un lot altéré.";
    // Abuse.
    RATE_LIMITED = "abuse.rate.exceeded", Warn, Normal, ["limit"], ["REQ-012-8"],
        "API Activity 6003 / Other", "web",
        "Une limite de débit a été dépassée.";
    INPUT_REJECTED = "abuse.input.rejected", Warn, Normal, ["reason"], ["REQ-012-8"],
        "API Activity 6003 / Other", "web",
        "Une entrée a été refusée par la validation à la frontière.";
}

/// The event kind of `name`, if defined.
#[must_use]
pub fn event_kind(name: &str) -> Option<&'static EventKind> {
    EVENTS.iter().copied().find(|e| e.name == name)
}

/// The catalogue as Markdown (REQ-012-30).
#[must_use]
pub fn catalogue_markdown() -> String {
    let mut out = String::from(
        "<!-- Généré par kartenn-core (audit::catalogue_markdown) : ne pas modifier à la main. -->\n\n\
         # Catalogue des événements d'audit\n\n\
         Noms stables (spec 012, REQ-012-5, 30). La sévérité suit la table de REQ-012-4 ; la\n\
         criticité fixe la durée de conservation (L22). Les correspondances OCSF (1.9) et ECS\n\
         (9) sont indicatives et s'appliquent à l'export.\n\n\
         | Événement | Sévérité | Criticité | Description | Détails | Exigences | OCSF | ECS |\n\
         |---|---|---|---|---|---|---|---|\n",
    );
    for e in EVENTS {
        let details = if e.details.is_empty() {
            "—".to_owned()
        } else {
            e.details
                .iter()
                .map(|d| format!("`{d}`"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} | {} | {} | {} | `{}` |",
            e.name,
            e.severity.as_str(),
            e.criticality.as_str(),
            e.summary,
            details,
            e.requirements.join(", "),
            e.ocsf,
            e.ecs
        );
    }
    out
}

/// The JSON Schema (draft 2020-12) of an exported audit event: the stored row of
/// `audit.event` as JSON, which the outputs receive and the seals cover (REQ-012-10). One
/// branch per event of the catalogue fixes its name, severity and criticality and the keys
/// of its `details`; attribution follows REQ-012-9.
#[must_use]
pub fn event_schema() -> serde_json::Value {
    use serde_json::json;
    let text = || json!({ "type": "string" });
    let maybe = |format: Option<&str>| match format {
        Some(f) => json!({ "type": ["string", "null"], "format": f }),
        None => json!({ "type": ["string", "null"] }),
    };
    let branches: Vec<serde_json::Value> = EVENTS
        .iter()
        .map(|e| {
            let details: serde_json::Map<String, serde_json::Value> = e
                .details
                .iter()
                .map(|d| ((*d).to_owned(), json!({})))
                .collect();
            json!({
                "properties": {
                    "name": { "const": e.name },
                    "severity": { "const": e.severity.as_str() },
                    "criticality": { "const": e.criticality.as_str() },
                    "details": {
                        "type": "object",
                        "properties": details,
                        "additionalProperties": false
                    }
                }
            })
        })
        .collect();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "urn:kartenn:audit-event",
        "title": "Événement d'audit de kartenn, sous sa forme exportée",
        "type": "object",
        "required": [
            "seq", "id", "name", "severity", "criticality", "occurred_at", "recorded_at",
            "actor_kind", "outcome", "service_version", "instance_id", "details"
        ],
        "additionalProperties": false,
        "properties": {
            "seq": { "type": "integer", "minimum": 1 },
            "id": { "type": "string", "format": "uuid" },
            "name": text(),
            "severity": { "enum": ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"] },
            "criticality": { "enum": ["low", "normal", "high"] },
            "occurred_at": { "type": "string", "format": "date-time" },
            "recorded_at": { "type": "string", "format": "date-time" },
            "actor_kind": { "enum": ["person", "service_account", "system", "unknown"] },
            "actor_id": maybe(Some("uuid")),
            "auth_method": maybe(None),
            "issuer": maybe(None),
            "claimed": maybe(None),
            "source_ip": maybe(None),
            "user_agent": { "type": ["string", "null"], "maxLength": 256 },
            "target_kind": maybe(None),
            "target_ids": {
                "type": ["array", "null"],
                "items": { "type": "string", "format": "uuid" }
            },
            "outcome": { "enum": ["success", "failure", "denied"] },
            "reason": maybe(None),
            "motive": maybe(None),
            "changeset_id": maybe(Some("uuid")),
            "trace_id": maybe(None),
            "span_id": maybe(None),
            "request_id": maybe(None),
            "service_version": text(),
            "instance_id": { "type": "string", "format": "uuid" },
            "details": { "type": "object" }
        },
        "allOf": [
            {
                "if": { "properties": { "actor_kind": { "enum": ["person", "service_account"] } } },
                "then": { "properties": { "actor_id": { "type": "string" } } }
            },
            {
                "if": { "properties": { "actor_kind": { "const": "unknown" } } },
                "then": { "properties": { "claimed": { "type": "string" } } }
            }
        ],
        "oneOf": branches
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn names_are_unique_and_well_formed() {
        let names: BTreeSet<_> = EVENTS.iter().map(|e| e.name).collect();
        assert_eq!(names.len(), EVENTS.len(), "duplicate event name");
        for name in names {
            assert_eq!(name.split('.').count(), 3, "{name}");
            // The MSGID of RFC 5424 §6.2.7 holds at most 32 octets.
            assert!(name.len() <= 32, "{name}");
            assert!(
                name.bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'_'),
                "{name}"
            );
        }
        assert_eq!(event_kind("authz.access.denied"), Some(&AUTHZ_DENIED));
        assert_eq!(event_kind("nope.nope.nope"), None);
    }

    #[test]
    fn criticality_follows_l22() {
        // Refusals, administration and sensitive reads are kept longest; ordinary reads
        // the shortest.
        for e in [
            &AUTHZ_DENIED,
            &APPROVAL_POLICY_CHANGED,
            &SENSITIVE_READ,
            &AUDIT_READ,
        ] {
            assert_eq!(e.criticality, Criticality::High, "{}", e.name);
        }
        assert_eq!(OBJECTS_READ.criticality, Criticality::Low);
    }

    #[test]
    fn the_published_schema_matches_the_code() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/spec/evenements-audit.schema.json"
        );
        let generated = serde_json::to_string_pretty(&event_schema()).expect("a JSON value") + "\n";
        if std::env::var_os("KARTENN_BLESS").is_some() {
            std::fs::write(path, &generated).expect("write the schema");
        }
        let published = std::fs::read_to_string(path).unwrap_or_default();
        assert_eq!(
            published, generated,
            "docs/spec/evenements-audit.schema.json is stale: run `KARTENN_BLESS=1 cargo test -p kartenn-core audit`"
        );
    }

    #[test]
    fn the_published_catalogue_matches_the_code() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/spec/evenements-audit.md"
        );
        let generated = catalogue_markdown();
        if std::env::var_os("KARTENN_BLESS").is_some() {
            std::fs::write(path, &generated).expect("write the catalogue");
        }
        let published = std::fs::read_to_string(path).unwrap_or_default();
        assert_eq!(
            published, generated,
            "docs/spec/evenements-audit.md is stale: run `KARTENN_BLESS=1 cargo test -p kartenn-core audit`"
        );
    }
}
