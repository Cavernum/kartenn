//! Catalogue of permissions and delivered roles (spec 005, REQ-005-16, 17; K12; D36).
//!
//! A permission is `<module>.<object>.<action>`, or `prop.<key>.<action>` for a property
//! (003). A role lists catalogue permissions and two patterns: `prop.*.<action>` covers the
//! properties that are **not sensitive** (a sensitive one needs its own permission,
//! 003/P17), and `*` covers everything. `docs/spec/permissions.md` is generated from here
//! and checked by a test.

use std::fmt::Write as _;

/// A permission of the catalogue.
#[derive(Debug, PartialEq, Eq)]
pub struct Permission {
    /// `<module>.<object>.<action>`.
    pub name: &'static str,
    /// What it allows, in one sentence.
    pub summary: &'static str,
    /// Only on the global scope (K7, K11).
    pub global_only: bool,
}

macro_rules! permissions {
    ($($id:ident = $name:literal, $global:literal, $summary:literal;)*) => {
        $(
            #[doc = $summary]
            pub static $id: Permission = Permission {
                name: $name,
                summary: $summary,
                global_only: $global,
            };
        )*
        /// Every permission of the core, in catalogue order.
        pub static PERMISSIONS: &[&Permission] = &[$(&$id),*];
    };
}

permissions! {
    ENTITY_READ = "core.entity.read", false, "Lire les entités.";
    ENTITY_WRITE = "core.entity.write", false, "Créer, modifier ou clore une entité.";
    ENTITY_PROPOSE = "core.entity.propose", false, "Proposer un changement d'entité.";
    SITE_READ = "core.site.read", false, "Lire les sites.";
    SITE_WRITE = "core.site.write", false, "Créer, modifier ou clore un site.";
    SITE_PROPOSE = "core.site.propose", false, "Proposer un changement de site.";
    DOMAIN_READ = "core.domain.read", false, "Lire les domaines d'adressage.";
    DOMAIN_WRITE = "core.domain.write", false, "Créer, modifier ou clore un domaine d'adressage.";
    DOMAIN_PROPOSE = "core.domain.propose", false, "Proposer un changement de domaine d'adressage.";
    PREFIX_READ = "core.prefix.read", false, "Lire les préfixes.";
    PREFIX_WRITE = "core.prefix.write", false, "Créer, modifier ou clore un préfixe.";
    PREFIX_PROPOSE = "core.prefix.propose", false, "Proposer un changement de préfixe.";
    RANGE_READ = "core.range.read", false, "Lire les plages.";
    RANGE_WRITE = "core.range.write", false, "Créer, modifier ou clore une plage.";
    RANGE_PROPOSE = "core.range.propose", false, "Proposer un changement de plage.";
    ADDRESS_READ = "core.address.read", false, "Lire les adresses.";
    ADDRESS_WRITE = "core.address.write", false, "Créer, modifier ou clore une adresse.";
    ADDRESS_PROPOSE = "core.address.propose", false, "Proposer un changement d'adresse.";
    CHANGESET_APPROVE = "core.changeset.approve", false, "Valider un changeset en attente qui touche le périmètre.";
    ANOMALY_ACKNOWLEDGE = "core.anomaly.acknowledge", false, "Acquitter, avec un motif, une anomalie d'un objet du périmètre.";
    PROPERTY_ADMIN = "core.property.admin", true, "Déclarer une propriété, changer sa sensibilité.";
    APPROVAL_POLICY_ADMIN = "core.approval_policy.admin", false, "Créer, modifier ou désactiver une politique d'approbation.";
    RBAC_ADMIN = "core.rbac.admin", false, "Poser ou clore des liaisons dans le périmètre, pour des rôles dont on détient les permissions.";
    ROLE_ADMIN = "core.role.admin", true, "Définir, modifier ou clore un rôle.";
    ISSUER_ADMIN = "core.issuer.admin", true, "Rattacher un émetteur OIDC à des entités ou au périmètre global.";
    ACCOUNT_ADMIN = "core.account.admin", true, "Gérer les comptes de service, désactiver une identité, révoquer ses sessions.";
    AUDIT_READ = "core.audit.read", false, "Consulter ou exporter le journal d'audit.";
    OBSERVABILITY_ADMIN = "core.observability.admin", true, "Changer à chaud le filtre d'une sortie technique.";
}

/// Actions of a property permission.
pub const PROPERTY_ACTIONS: [&str; 3] = ["read", "write", "propose"];

/// The permission `name` of the catalogue, if any.
#[must_use]
pub fn permission(name: &str) -> Option<&'static Permission> {
    PERMISSIONS.iter().copied().find(|p| p.name == name)
}

/// Whether `entry` may stand in a role: a catalogue permission, `prop.<key>.<action>`,
/// `prop.*.<action>`, or `*`. The key of a property permission is checked against the
/// declarations by the service (`rbac::check`).
#[must_use]
pub fn valid_role_entry(entry: &str) -> bool {
    if entry == "*" || permission(entry).is_some() {
        return true;
    }
    entry
        .strip_prefix("prop.")
        .and_then(|rest| rest.rsplit_once('.'))
        .is_some_and(|(key, action)| !key.is_empty() && PROPERTY_ACTIONS.contains(&action))
}

/// Whether a role holding `entry` grants `permission`, `sensitive` telling whether the
/// property of a property permission is sensitive.
#[must_use]
pub fn grants(entry: &str, permission: &str, sensitive: bool) -> bool {
    if entry == "*" || entry == permission {
        return true;
    }
    match (
        entry.strip_prefix("prop.*."),
        permission.strip_prefix("prop."),
    ) {
        (Some(action), Some(rest)) => {
            !sensitive && rest.rsplit_once('.').is_some_and(|(_, a)| a == action)
        }
        _ => false,
    }
}

/// A role delivered with kartenn (K12).
#[derive(Debug, PartialEq, Eq)]
pub struct BuiltinRole {
    /// Stable key.
    pub key: &'static str,
    /// Label.
    pub label: &'static str,
    /// Its entries (permissions and patterns).
    pub entries: &'static [&'static str],
}

const READS: [&str; 6] = [
    "core.entity.read",
    "core.site.read",
    "core.domain.read",
    "core.prefix.read",
    "core.range.read",
    "core.address.read",
];

/// Reader: every read but sensitive values.
pub static READER: BuiltinRole = BuiltinRole {
    key: "reader",
    label: "Lecteur",
    entries: &READS,
};

/// Editor: the reader's reads, the writes of the objects and of the non-sensitive
/// properties.
pub static EDITOR: BuiltinRole = BuiltinRole {
    key: "editor",
    label: "Éditeur",
    entries: &[
        "core.entity.read",
        "core.site.read",
        "core.domain.read",
        "core.prefix.read",
        "core.range.read",
        "core.address.read",
        "core.entity.write",
        "core.site.write",
        "core.domain.write",
        "core.prefix.write",
        "core.range.write",
        "core.address.write",
        "prop.*.write",
    ],
};

/// Approver: the reader's reads and the validation of pending changesets.
pub static APPROVER: BuiltinRole = BuiltinRole {
    key: "approver",
    label: "Approbateur",
    entries: &[
        "core.entity.read",
        "core.site.read",
        "core.domain.read",
        "core.prefix.read",
        "core.range.read",
        "core.address.read",
        "core.changeset.approve",
    ],
};

/// Administrator: everything; the global-only permissions act on the global scope alone.
pub static ADMINISTRATOR: BuiltinRole = BuiltinRole {
    key: "administrator",
    label: "Administrateur",
    entries: &["*"],
};

/// The delivered roles.
pub static BUILTIN_ROLES: &[&BuiltinRole] = &[&READER, &EDITOR, &APPROVER, &ADMINISTRATOR];

/// The catalogue as Markdown (REQ-005-16).
#[must_use]
pub fn catalogue_markdown() -> String {
    let mut out = String::from(
        "<!-- Généré par kartenn-core (authz::catalogue_markdown) : ne pas modifier à la main. -->\n\n\
         # Catalogue des permissions\n\n\
         Permissions du noyau (spec 005, REQ-005-16). S'y ajoutent, par propriété déclarée,\n\
         `prop.<clé>.write` et `prop.<clé>.propose`, et `prop.<clé>.read` pour une propriété\n\
         sensible. Un rôle peut contenir `prop.*.<action>` (toute propriété non sensible) et\n\
         `*` (tout). « Global » : ne s'accorde que sur le périmètre global.\n\n\
         | Permission | Global | Description |\n\
         |---|---|---|\n",
    );
    for p in PERMISSIONS {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} |",
            p.name,
            if p.global_only { "oui" } else { "—" },
            p.summary
        );
    }
    out.push_str("\n## Rôles livrés\n\n| Rôle | Clé | Contenu |\n|---|---|---|\n");
    for r in BUILTIN_ROLES {
        let entries = r
            .entries
            .iter()
            .map(|e| format!("`{e}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "| {} | `{}` | {} |", r.label, r.key, entries);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn names_are_unique_and_well_formed() {
        let names: BTreeSet<_> = PERMISSIONS.iter().map(|p| p.name).collect();
        assert_eq!(names.len(), PERMISSIONS.len(), "duplicate permission");
        for name in names {
            assert_eq!(name.split('.').count(), 3, "{name}");
            assert!(name.starts_with("core."), "{name}");
            assert!(
                name.bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'_'),
                "{name}"
            );
        }
    }

    #[test]
    fn delivered_roles_use_the_catalogue() {
        for role in BUILTIN_ROLES {
            for entry in role.entries {
                assert!(valid_role_entry(entry), "{}: {entry}", role.key);
            }
        }
        assert!(!valid_role_entry("core.nothing.read"));
        assert!(!valid_role_entry("prop.custom.x.delete"));
        assert!(valid_role_entry("prop.custom.x.read"));
    }

    #[test]
    fn patterns_spare_sensitive_properties() {
        assert!(grants("prop.*.write", "prop.custom.rack.write", false));
        assert!(!grants("prop.*.write", "prop.custom.secret.write", true));
        assert!(!grants("prop.*.write", "prop.custom.rack.read", false));
        assert!(!grants("prop.*.write", "core.address.write", false));
        assert!(grants("*", "prop.custom.secret.read", true));
        assert!(grants("core.address.read", "core.address.read", false));
        assert!(!grants("core.address.read", "core.address.write", false));
    }

    #[test]
    fn the_published_catalogue_matches_the_code() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/spec/permissions.md"
        );
        let generated = catalogue_markdown();
        if std::env::var_os("KARTENN_BLESS").is_some() {
            std::fs::write(path, &generated).expect("write the catalogue");
        }
        let published = std::fs::read_to_string(path).unwrap_or_default();
        assert_eq!(
            published, generated,
            "docs/spec/permissions.md is stale: run `KARTENN_BLESS=1 cargo test -p kartenn-core authz`"
        );
    }
}
