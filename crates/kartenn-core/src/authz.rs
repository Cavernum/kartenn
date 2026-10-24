//! Catalogue of permissions and delivered roles (spec 005, REQ-005-16, 17; K12; D36).
//!
//! A permission is `<module>.<object>.<action>`, `<module>.<action>` when the object is
//! named after the module (spec 018, R2: `flow.read`), or `prop.<key>.<action>` for a
//! property (003). A role lists catalogue permissions and two patterns: `prop.*.<action>` covers the
//! properties that are **not sensitive** (a sensitive one needs its own permission,
//! 003/P17), and `*` covers everything. `docs/spec/permissions.md` is generated from here, with the
//! permissions of the modules of the build, and checked by a test of `kartenn-server`.
//!
//! The modules of a build add their permissions to the catalogue when their registry is
//! checked (spec 008, REQ-008-8): a lookup, a role entry and the scopes of a permission
//! see them as the core's.

use std::fmt::Write as _;
use std::sync::{PoisonError, RwLock};

/// A permission of the catalogue.
#[derive(Debug, PartialEq, Eq)]
pub struct Permission {
    /// `<module>.<object>.<action>`, or `<module>.<action>` (spec 018, R2).
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
    ENTITY_EDIT = "core.entity.edit", false, "Créer, modifier ou clore une entité dans un changeset et le soumettre.";
    ENTITY_APPLY = "core.entity.apply", false, "Appliquer ou rejeter le changeset d'un autre qui touche une entité.";
    ENTITY_APPLY_OWN = "core.entity.apply_own", false, "Appliquer son propre changeset qui touche une entité.";
    SITE_READ = "core.site.read", false, "Lire les sites.";
    SITE_EDIT = "core.site.edit", false, "Créer, modifier ou clore un site dans un changeset et le soumettre.";
    SITE_APPLY = "core.site.apply", false, "Appliquer ou rejeter le changeset d'un autre qui touche un site.";
    SITE_APPLY_OWN = "core.site.apply_own", false, "Appliquer son propre changeset qui touche un site.";
    DOMAIN_READ = "core.domain.read", false, "Lire les domaines d'adressage.";
    DOMAIN_EDIT = "core.domain.edit", false, "Créer, modifier ou clore un domaine d'adressage dans un changeset et le soumettre.";
    DOMAIN_APPLY = "core.domain.apply", false, "Appliquer ou rejeter le changeset d'un autre qui touche un domaine d'adressage.";
    DOMAIN_APPLY_OWN = "core.domain.apply_own", false, "Appliquer son propre changeset qui touche un domaine d'adressage.";
    PREFIX_READ = "core.prefix.read", false, "Lire les préfixes.";
    PREFIX_EDIT = "core.prefix.edit", false, "Créer, modifier ou clore un préfixe dans un changeset et le soumettre.";
    PREFIX_APPLY = "core.prefix.apply", false, "Appliquer ou rejeter le changeset d'un autre qui touche un préfixe.";
    PREFIX_APPLY_OWN = "core.prefix.apply_own", false, "Appliquer son propre changeset qui touche un préfixe.";
    RANGE_READ = "core.range.read", false, "Lire les plages.";
    RANGE_EDIT = "core.range.edit", false, "Créer, modifier ou clore une plage dans un changeset et le soumettre.";
    RANGE_APPLY = "core.range.apply", false, "Appliquer ou rejeter le changeset d'un autre qui touche une plage.";
    RANGE_APPLY_OWN = "core.range.apply_own", false, "Appliquer son propre changeset qui touche une plage.";
    ADDRESS_READ = "core.address.read", false, "Lire les adresses.";
    ADDRESS_EDIT = "core.address.edit", false, "Créer, modifier ou clore une adresse dans un changeset et le soumettre.";
    ADDRESS_APPLY = "core.address.apply", false, "Appliquer ou rejeter le changeset d'un autre qui touche une adresse.";
    ADDRESS_APPLY_OWN = "core.address.apply_own", false, "Appliquer son propre changeset qui touche une adresse.";
    CHANGESET_READ = "core.changeset.read", true, "Lire les changesets, leurs opérations et leur différentiel.";
    ANOMALY_ACKNOWLEDGE ="core.anomaly.acknowledge", false, "Acquitter, avec un motif, une anomalie d'un objet du périmètre.";
    PROPERTY_ADMIN = "core.property.admin", true, "Déclarer une propriété, changer sa sensibilité.";
    APPROVAL_POLICY_ADMIN = "core.approval_policy.admin", false, "Créer, modifier ou désactiver une politique d'approbation.";
    RBAC_READ = "core.rbac.read", true, "Lire les rôles, liaisons, rattachements, comptes de service et l'état des identités.";
    RBAC_ADMIN = "core.rbac.admin", false, "Poser ou clore des liaisons dans le périmètre, pour des rôles dont on détient les permissions.";
    ROLE_ADMIN = "core.role.admin", true, "Définir, modifier ou clore un rôle.";
    ISSUER_ADMIN = "core.issuer.admin", true, "Rattacher un émetteur OIDC à des entités ou au périmètre global.";
    ACCOUNT_ADMIN = "core.account.admin", true, "Gérer les comptes de service, désactiver une identité, révoquer ses sessions.";
    AUDIT_READ = "core.audit.read", false, "Consulter ou exporter le journal d'audit.";
    IDENTITY_PSEUDONYMISE = "core.identity.pseudonymise", true, "Pseudonymiser sans délai les données d'identité d'une personne.";
    IDENTITY_ACT_ON_BEHALF = "core.identity.act_on_behalf", true, "Pour un compte de service : écrire pour le compte d'un demandeur nommé par l'en-tête Kartenn-On-Behalf-Of, sous ses propres droits.";
    OBSERVABILITY_ADMIN = "core.observability.admin", true, "Changer à chaud le filtre d'une sortie technique.";
    DRAFT_ADMIN = "core.draft.admin", true, "Lire, supprimer ou attribuer à un autre le brouillon d'une personne.";
}

/// Actions of a property permission (spec 024, Q-024-6).
pub const PROPERTY_ACTIONS: [&str; 4] = ["read", "edit", "apply", "apply_own"];

/// What a subject does with a changeset (spec 024, M7): edit it (create and submit it),
/// apply someone else's, or apply its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// Create and submit a changeset; it is pending.
    Edit,
    /// Apply or reject a pending changeset of someone else.
    Apply,
    /// Apply one's own pending changeset.
    ApplyOwn,
}

/// The permission of `verb` that matches the edit permission `edit` (spec 024, M7):
/// `<resource>.apply` or `<resource>.apply_own` for `<resource>.edit`. A permission of
/// another action (`core.role.admin`, `flow.request`) has no apply of its own: it is its
/// own edit, apply and apply of one's own changesets.
#[must_use]
pub fn verb_permission(edit: &str, verb: Verb) -> String {
    match (edit.strip_suffix(".edit"), verb) {
        (Some(resource), Verb::Apply) => format!("{resource}.apply"),
        (Some(resource), Verb::ApplyOwn) => format!("{resource}.apply_own"),
        _ => edit.to_owned(),
    }
}

/// The permission retired by spec 024 (REQ-024-9) that grants `approve`: the validation of
/// any pending changeset of the scope.
pub const RETIRED_APPROVE: &str = "core.changeset.approve";

/// The permissions that replace the retired entry `entry` (spec 024, REQ-024-9), or `None`
/// for an entry that is not retired: `<r>.write` → `<r>.edit`, `<r>.apply_own`;
/// `<r>.propose` → `<r>.edit`; [`RETIRED_APPROVE`] → every `<r>.apply` of `catalogue` and
/// `prop.*.apply`. A name renamed by spec 018 is renamed first. `<r>` is a resource of
/// `catalogue` (one with an edit permission) or a property, `prop.<key>` or `prop.*`.
#[must_use]
pub fn retired(entry: &str, catalogue: &[&Permission]) -> Option<Vec<String>> {
    let renamed = RENAMED_PERMISSIONS
        .iter()
        .find(|(old, _)| *old == entry)
        .map(|(_, new)| *new);
    let name = renamed.unwrap_or(entry);
    if name == RETIRED_APPROVE {
        let mut applies: Vec<String> = catalogue
            .iter()
            .filter(|p| p.name.rsplit('.').next() == Some("apply"))
            .map(|p| p.name.to_owned())
            .collect();
        applies.push("prop.*.apply".into());
        return Some(applies);
    }
    let (resource, action) = name.rsplit_once('.')?;
    let known = resource.starts_with("prop.")
        || catalogue
            .iter()
            .any(|p| p.name.strip_suffix(".edit") == Some(resource));
    match action {
        "write" if known => Some(vec![
            format!("{resource}.edit"),
            format!("{resource}.apply_own"),
        ]),
        "propose" if known => Some(vec![format!("{resource}.edit")]),
        _ => renamed.map(|new| vec![new.to_owned()]),
    }
}

/// The retired names of `catalogue` and what replaces each (spec 024, REQ-024-9; spec 018,
/// R3): `<r>.write` and `<r>.propose` of each resource with an edit permission, the names
/// renamed by spec 018, `prop.*.write`, `prop.*.propose` and [`RETIRED_APPROVE`]. The
/// labels of the history and of the audit journal name them (018/R6).
#[must_use]
pub fn retired_names(catalogue: &[&Permission]) -> Vec<(String, Vec<String>)> {
    let mut olds: Vec<String> = catalogue
        .iter()
        .filter_map(|p| p.name.strip_suffix(".edit"))
        .chain(std::iter::once("prop.*"))
        .flat_map(|r| [format!("{r}.write"), format!("{r}.propose")])
        .collect();
    olds.extend(RENAMED_PERMISSIONS.iter().map(|(old, _)| (*old).to_owned()));
    olds.push(RETIRED_APPROVE.to_owned());
    olds.into_iter()
        .filter_map(|old| retired(&old, catalogue).map(|new| (old, new)))
        .collect()
}

/// The permissions renamed by spec 018 (R2, R3), `(old, new)`: the only source for the
/// migration of the custom roles (Q-018-3) and for the labels of the old names that the
/// audit journal and the history keep (R6). It opens **no alias** (Q-018-2): an old name
/// is refused in a role and never granted. The names in `write` and `propose` it gives
/// were retired since by spec 024 ([`retired`] composes both).
pub static RENAMED_PERMISSIONS: &[(&str, &str)] = &[
    ("vlan.vlan.read", "vlan.read"),
    ("vlan.vlan.write", "vlan.write"),
    ("vlan.vlan.propose", "vlan.propose"),
    ("machines.machine.read", "machines.read"),
    ("machines.machine.write", "machines.write"),
    ("machines.machine.propose", "machines.propose"),
    ("flow.flow.read", "flow.read"),
    ("flow.flow.request", "flow.request"),
    ("flow.flow.review", "flow.review"),
    ("flow.flow.write", "flow.write"),
    ("flow.flow.waive", "flow.waiver.grant"),
];

/// The problems of a catalogue against the naming rule of spec 018 (REQ-018-1, R2, R5):
/// `<module>.<resource>.<action>`, or `<module>.<action>` when the resource is named after
/// the module (never `<module>.<module>.<action>`), lowercase, unique, and no resource
/// named after an action of the catalogue (so that `<module>.<action>` stays unambiguous).
#[must_use]
pub fn catalogue_problems(permissions: &[&Permission]) -> Vec<String> {
    let mut problems = Vec::new();
    let actions: std::collections::BTreeSet<&str> = permissions
        .iter()
        .filter_map(|p| p.name.rsplit('.').next())
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    for p in permissions {
        let name = p.name;
        if !seen.insert(name) {
            problems.push(format!("`{name}`: duplicate"));
        }
        let segments: Vec<&str> = name.split('.').collect();
        let well_formed = (2..=3).contains(&segments.len())
            && segments.iter().all(|s| {
                s.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
                    && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
            });
        if !well_formed {
            problems.push(format!("`{name}`: not `<module>[.<resource>].<action>`"));
            continue;
        }
        if matches!(segments.last(), Some(&("write" | "propose"))) {
            problems.push(format!(
                "`{name}`: a retired action, `edit`, `apply` and `apply_own` replace it"
            ));
        }
        if let [module, resource, _] = segments[..] {
            if resource == module {
                problems.push(format!("`{name}`: the resource repeats the module"));
            }
            if actions.contains(resource) {
                problems.push(format!("`{name}`: the resource is named after an action"));
            }
        }
    }
    problems
}

/// The permissions the modules of the build added to the catalogue.
static MODULE_PERMISSIONS: RwLock<Vec<&'static Permission>> = RwLock::new(Vec::new());

/// Adds the permissions of a module to the catalogue; a name already there is kept once.
pub fn register(permissions: &[&'static Permission]) {
    let mut known = MODULE_PERMISSIONS
        .write()
        .unwrap_or_else(PoisonError::into_inner);
    for p in permissions {
        if !known.iter().any(|k| k.name == p.name) {
            known.push(p);
        }
    }
}

/// The permissions the modules added, in the order of their registration.
#[must_use]
pub fn module_permissions() -> Vec<&'static Permission> {
    MODULE_PERMISSIONS
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// The permissions an object type of a module is read and edited under, when they are not
/// `<type>.read` and `<type>.edit` (ADR 0026, clause 5): several types may share one
/// permission (the policy module, spec 010/S15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypePermissions {
    /// The qualified type, `<module>.<type>`.
    pub kind: &'static str,
    /// Its read permission.
    pub read: &'static str,
    /// Its edit permission; its applications end in `.apply` and `.apply_own` when it ends
    /// in `.edit` ([`verb_permission`]).
    pub edit: &'static str,
}

/// The type permissions the modules of the build declared.
static TYPE_PERMISSIONS: RwLock<Vec<TypePermissions>> = RwLock::new(Vec::new());

/// Records the permissions of object types of a module; a type already there is kept once.
pub fn register_types(types: &[TypePermissions]) {
    let mut known = TYPE_PERMISSIONS
        .write()
        .unwrap_or_else(PoisonError::into_inner);
    for t in types {
        if !known.iter().any(|k| k.kind == t.kind) {
            known.push(*t);
        }
    }
}

fn type_permissions(kind: &str) -> Option<TypePermissions> {
    TYPE_PERMISSIONS
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .copied()
        .find(|t| t.kind == kind)
}

/// The permission `<kind>.<action>` by the naming rule (spec 018, R2, R4): a type named
/// after its module, `m.m`, gives `m.<action>`.
fn default_type_permission(kind: &str, action: &str) -> String {
    match kind.split_once('.') {
        Some((module, ty)) if module == ty => format!("{module}.{action}"),
        _ => format!("{kind}.{action}"),
    }
}

/// The read permission of the object type `kind` of a module: its declared one, else
/// `<kind>.read`, or `<module>.read` for a type named after its module (spec 018, R4).
#[must_use]
pub fn type_read_permission(kind: &str) -> String {
    type_permissions(kind).map_or_else(
        || default_type_permission(kind, "read"),
        |t| t.read.to_owned(),
    )
}

/// The edit permission of the object type `kind` of a module: its declared one, else
/// `<kind>.edit`, or `<module>.edit` for a type named after its module (spec 018, R4;
/// spec 024, REQ-024-9).
#[must_use]
pub fn type_edit_permission(kind: &str) -> String {
    type_permissions(kind).map_or_else(
        || default_type_permission(kind, "edit"),
        |t| t.edit.to_owned(),
    )
}

/// The permission `name` of the catalogue, if any.
#[must_use]
pub fn permission(name: &str) -> Option<&'static Permission> {
    PERMISSIONS
        .iter()
        .copied()
        .find(|p| p.name == name)
        .or_else(|| module_permissions().into_iter().find(|p| p.name == name))
}

/// Whether `entry` may stand in a role: a catalogue permission, `prop.<key>.<action>`,
/// `prop.*.<action>`, or `*`. The key of a property permission is checked against the
/// declarations by the service (`rbac::check`). An old name of [`RENAMED_PERMISSIONS`] or
/// a retired verb ([`retired`]) is refused, with no alias (spec 018, REQ-018-3, Q-018-2;
/// spec 024, REQ-024-9).
#[must_use]
pub fn valid_role_entry(entry: &str) -> bool {
    if RENAMED_PERMISSIONS.iter().any(|(old, _)| *old == entry)
        || entry == RETIRED_APPROVE
        || matches!(entry.rsplit('.').next(), Some("write" | "propose"))
    {
        return false;
    }
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
    /// Its labels in French and English (spec 018, REQ-018-13).
    pub labels: crate::labels::Compiled,
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
    labels: crate::labels::Compiled::new("Lecteur", "Reader"),
    entries: &READS,
};

/// Editor: the reader's reads, the edition of the objects and of the non-sensitive
/// properties, and the application of its own changesets (spec 024, REQ-024-9: what
/// `write` gave).
pub static EDITOR: BuiltinRole = BuiltinRole {
    key: "editor",
    labels: crate::labels::Compiled::new("Éditeur", "Editor"),
    entries: &[
        "core.entity.read",
        "core.site.read",
        "core.domain.read",
        "core.prefix.read",
        "core.range.read",
        "core.address.read",
        "core.entity.edit",
        "core.site.edit",
        "core.domain.edit",
        "core.prefix.edit",
        "core.range.edit",
        "core.address.edit",
        "prop.*.edit",
        "core.entity.apply_own",
        "core.site.apply_own",
        "core.domain.apply_own",
        "core.prefix.apply_own",
        "core.range.apply_own",
        "core.address.apply_own",
        "prop.*.apply_own",
    ],
};

/// Approver: the reader's reads and the application of the pending changesets of others
/// (spec 024, REQ-024-9: what `core.changeset.approve` gave).
pub static APPROVER: BuiltinRole = BuiltinRole {
    key: "approver",
    labels: crate::labels::Compiled::new("Approbateur", "Approver"),
    entries: &[
        "core.entity.read",
        "core.site.read",
        "core.domain.read",
        "core.prefix.read",
        "core.range.read",
        "core.address.read",
        "core.entity.apply",
        "core.site.apply",
        "core.domain.apply",
        "core.prefix.apply",
        "core.range.apply",
        "core.address.apply",
        "prop.*.apply",
    ],
};

/// Administrator: everything; the global-only permissions act on the global scope alone.
pub static ADMINISTRATOR: BuiltinRole = BuiltinRole {
    key: "administrator",
    labels: crate::labels::Compiled::new("Administrateur", "Administrator"),
    entries: &["*"],
};

/// The delivered roles.
pub static BUILTIN_ROLES: &[&BuiltinRole] = &[&READER, &EDITOR, &APPROVER, &ADMINISTRATOR];

impl BuiltinRole {
    /// Its entries in a build with the permissions `modules`: the delivered roles that
    /// read read the objects of the modules too (spec 008, REQ-008-8), and those that
    /// apply the changesets of others apply them on the objects of the modules too, as
    /// `core.changeset.approve` did (spec 024, REQ-024-9); `*` needs nothing.
    #[must_use]
    pub fn entries_with(&self, modules: &[&'static Permission]) -> Vec<&'static str> {
        let mut entries: Vec<&'static str> = self.entries.to_vec();
        for (marker, action) in [("core.entity.read", "read"), ("core.entity.apply", "apply")] {
            if self.entries.contains(&marker) {
                entries.extend(
                    modules
                        .iter()
                        .filter(|p| p.name.rsplit('.').next() == Some(action) && !p.global_only)
                        .map(|p| p.name),
                );
            }
        }
        entries
    }
}

/// The catalogue as Markdown (REQ-005-16).
#[must_use]
pub fn catalogue_markdown() -> String {
    catalogue_markdown_with(&[])
}

/// [`catalogue_markdown`] for a build whose modules declare `modules`.
#[must_use]
pub fn catalogue_markdown_with(modules: &[&'static Permission]) -> String {
    let mut out = String::from(
        "<!-- Généré par kartenn-core (authz::catalogue_markdown) : ne pas modifier à la main. -->\n\n\
         # Catalogue des permissions\n\n\
         Permissions du noyau (spec 005, REQ-005-16). S'y ajoutent, par propriété déclarée,\n\
         `prop.<clé>.edit`, `prop.<clé>.apply` et `prop.<clé>.apply_own`, et `prop.<clé>.read`\n\
         pour une propriété sensible. Un rôle peut contenir `prop.*.<action>` (toute propriété\n\
         non sensible) et `*` (tout). « Global » : ne s'accorde que sur le périmètre global.\n\n\
         Toute modification passe par un changeset (spec 024, M7) : `edit` le crée et le\n\
         soumet, en attente ; `apply` applique ou rejette le changeset d'un autre ;\n\
         `apply_own` applique le sien, sauf quand une politique d'approbation le retient.\n\
         Une permission d'une autre action (`core.role.admin`, `flow.request`) vaut\n\
         édition et application.\n\n\
         | Permission | Global | Description |\n\
         |---|---|---|\n",
    );
    for p in PERMISSIONS.iter().chain(modules) {
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
            .entries_with(modules)
            .iter()
            .map(|e| format!("`{e}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "| {} | `{}` | {} |", r.labels.fr, r.key, entries);
    }
    out.push_str(
        "\n## Anciens noms\n\n\
         Renommées par la spec 018 (R2, R3) ou remplacées par la spec 024 (REQ-024-9). Un\n\
         ancien nom est refusé dans un rôle (`core.role.invalid`) ; il ne reste que dans\n\
         l'historique et le journal d'audit.\n\n\
         | Ancien nom | Nouveaux noms |\n|---|---|\n",
    );
    let catalogue: Vec<&Permission> = PERMISSIONS
        .iter()
        .copied()
        .chain(modules.iter().copied())
        .collect();
    for (old, new) in retired_names(&catalogue) {
        let new = new
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "| `{old}` | {new} |");
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn names_are_unique_and_well_formed() {
        assert_eq!(catalogue_problems(PERMISSIONS), Vec::<String>::new());
        for p in PERMISSIONS {
            assert!(p.name.starts_with("core."), "{}", p.name);
        }
    }

    /// REQ-018-1: a catalogue that repeats the module, or names a resource after an
    /// action, is refused.
    #[test]
    fn permission_names_omit_the_module_resource() {
        static REPEATED: Permission = Permission {
            name: "flow.flow.read",
            summary: "",
            global_only: false,
        };
        static SHORT: Permission = Permission {
            name: "flow.read",
            summary: "",
            global_only: false,
        };
        static GRANT: Permission = Permission {
            name: "flow.waiver.grant",
            summary: "",
            global_only: false,
        };
        static ACTION_AS_RESOURCE: Permission = Permission {
            name: "flow.read.grant",
            summary: "",
            global_only: false,
        };
        assert_eq!(catalogue_problems(&[&SHORT, &GRANT]), Vec::<String>::new());
        assert_eq!(catalogue_problems(&[&REPEATED]).len(), 1);
        assert_eq!(catalogue_problems(&[&SHORT, &ACTION_AS_RESOURCE]).len(), 1);
        assert_eq!(catalogue_problems(&[&SHORT, &SHORT]).len(), 1);
    }

    /// REQ-018-3: an old name stands in no role, even when a catalogue still declares it.
    #[test]
    fn an_old_permission_name_is_refused() {
        static OLD: Permission = Permission {
            name: "flow.flow.read",
            summary: "",
            global_only: false,
        };
        register(&[&OLD]);
        for (old, _) in RENAMED_PERMISSIONS {
            assert!(!valid_role_entry(old), "{old}");
        }
    }

    /// R4: a type named after its module reads and writes under `<module>.<action>`; the
    /// other types keep `<kind>.<action>`, unless their module declares otherwise
    /// (`machines.machine`, see `kartenn-server`).
    #[test]
    fn a_module_kind_reads_under_the_short_permission() {
        assert_eq!(type_read_permission("vlan.vlan"), "vlan.read");
        assert_eq!(type_edit_permission("flow.flow"), "flow.edit");
        assert_eq!(type_read_permission("vlan.group"), "vlan.group.read");
    }

    /// REQ-024-9: each edit permission has its two applications; another action is its
    /// own application.
    #[test]
    fn an_edit_permission_has_two_applications() {
        assert_eq!(
            verb_permission("core.prefix.edit", Verb::Apply),
            "core.prefix.apply"
        );
        assert_eq!(
            verb_permission("vlan.edit", Verb::ApplyOwn),
            "vlan.apply_own"
        );
        assert_eq!(verb_permission("prop.*.edit", Verb::Edit), "prop.*.edit");
        assert_eq!(
            verb_permission("core.role.admin", Verb::Apply),
            "core.role.admin"
        );
        for p in PERMISSIONS {
            if let Some(resource) = p.name.strip_suffix(".edit") {
                for verb in [Verb::Apply, Verb::ApplyOwn] {
                    let applied = verb_permission(p.name, verb);
                    assert!(permission(&applied).is_some(), "{resource}: {applied}");
                }
            }
        }
    }

    /// REQ-024-9: `write` gives `edit` and `apply_own`, `propose` gives `edit`, `approve`
    /// gives every `apply`; the names of spec 018 are renamed first; every retired name is
    /// refused in a role.
    #[test]
    fn retired_verbs_are_replaced_and_refused() {
        static VLAN_EDIT: Permission = Permission {
            name: "vlan.edit",
            summary: "",
            global_only: false,
        };
        static WRITE: Permission = Permission {
            name: "flow.write",
            summary: "",
            global_only: false,
        };
        let catalogue: Vec<&Permission> = PERMISSIONS.to_vec();
        assert_eq!(
            retired("core.prefix.write", &catalogue),
            Some(vec![
                "core.prefix.edit".into(),
                "core.prefix.apply_own".into()
            ])
        );
        assert_eq!(
            retired("prop.custom.rack.propose", &catalogue),
            Some(vec!["prop.custom.rack.edit".into()])
        );
        let approve = retired(RETIRED_APPROVE, &catalogue).expect("approve is retired");
        assert!(approve.contains(&"core.address.apply".to_owned()));
        assert!(approve.contains(&"prop.*.apply".to_owned()));
        assert_eq!(retired("core.prefix.edit", &catalogue), None);
        assert_eq!(retired("core.rbac.read", &catalogue), None);
        let with_vlan = [&VLAN_EDIT];
        assert_eq!(
            retired("vlan.vlan.write", &with_vlan),
            Some(vec!["vlan.edit".into(), "vlan.apply_own".into()])
        );
        for (old, new) in retired_names(&catalogue) {
            assert!(!valid_role_entry(&old), "{old}");
            assert!(!new.is_empty(), "{old}");
        }
        assert_eq!(catalogue_problems(&[&WRITE]).len(), 1);
    }

    /// R3: the table renames each old name once, to a name of the new grammar.
    #[test]
    fn the_renaming_table_is_well_formed() {
        let olds: BTreeSet<_> = RENAMED_PERMISSIONS.iter().map(|(old, _)| *old).collect();
        assert_eq!(olds.len(), RENAMED_PERMISSIONS.len());
        for (old, new) in RENAMED_PERMISSIONS {
            assert_ne!(old, new);
            assert!(!olds.contains(new), "{new} is renamed again");
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
        assert!(valid_role_entry("prop.custom.x.apply_own"));
        assert!(valid_role_entry("core.draft.admin"));
    }

    #[test]
    fn patterns_spare_sensitive_properties() {
        assert!(grants("prop.*.edit", "prop.custom.rack.edit", false));
        assert!(!grants("prop.*.edit", "prop.custom.secret.edit", true));
        assert!(!grants("prop.*.edit", "prop.custom.rack.read", false));
        assert!(!grants("prop.*.edit", "core.address.edit", false));
        assert!(!grants("prop.*.apply", "prop.custom.rack.apply_own", false));
        assert!(grants("*", "prop.custom.secret.read", true));
        assert!(grants("core.address.read", "core.address.read", false));
        assert!(!grants("core.address.read", "core.address.edit", false));
    }
}
