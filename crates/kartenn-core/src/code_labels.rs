//! The labels of the codes of the core (spec 018, R8 b, REQ-018-10; ADR 0040): French
//! and English for each permission, rule, kind of object, state, nature and usage. The
//! French of a permission or a rule is its definition's (`summary`); the English is here,
//! beside it, keyed by the same identifier. A test proves that every code has both.

use crate::labels::CodeLabel;

/// English summaries of the permissions of the core, by name.
pub static PERMISSIONS_EN: &[(&str, &str)] = &[
    ("core.entity.read", "Read the entities."),
    (
        "core.entity.edit",
        "Create, change or close an entity in a changeset and submit it.",
    ),
    (
        "core.entity.apply",
        "Apply or reject the changeset of someone else that touches an entity.",
    ),
    (
        "core.entity.apply_own",
        "Apply one's own changeset that touches an entity.",
    ),
    ("core.site.read", "Read the sites."),
    (
        "core.site.edit",
        "Create, change or close a site in a changeset and submit it.",
    ),
    (
        "core.site.apply",
        "Apply or reject the changeset of someone else that touches a site.",
    ),
    (
        "core.site.apply_own",
        "Apply one's own changeset that touches a site.",
    ),
    ("core.domain.read", "Read the addressing domains."),
    (
        "core.domain.edit",
        "Create, change or close an addressing domain in a changeset and submit it.",
    ),
    (
        "core.domain.apply",
        "Apply or reject the changeset of someone else that touches an addressing domain.",
    ),
    (
        "core.domain.apply_own",
        "Apply one's own changeset that touches an addressing domain.",
    ),
    ("core.prefix.read", "Read the prefixes."),
    (
        "core.prefix.edit",
        "Create, change or close a prefix in a changeset and submit it.",
    ),
    (
        "core.prefix.apply",
        "Apply or reject the changeset of someone else that touches a prefix.",
    ),
    (
        "core.prefix.apply_own",
        "Apply one's own changeset that touches a prefix.",
    ),
    ("core.range.read", "Read the ranges."),
    (
        "core.range.edit",
        "Create, change or close a range in a changeset and submit it.",
    ),
    (
        "core.range.apply",
        "Apply or reject the changeset of someone else that touches a range.",
    ),
    (
        "core.range.apply_own",
        "Apply one's own changeset that touches a range.",
    ),
    ("core.address.read", "Read the addresses."),
    (
        "core.address.edit",
        "Create, change or close an address in a changeset and submit it.",
    ),
    (
        "core.address.apply",
        "Apply or reject the changeset of someone else that touches an address.",
    ),
    (
        "core.address.apply_own",
        "Apply one's own changeset that touches an address.",
    ),
    (
        "core.changeset.read",
        "Read the changesets, their operations and their differential.",
    ),
    (
        "core.anomaly.acknowledge",
        "Acknowledge, with a motive, an anomaly of an object of the scope.",
    ),
    (
        "core.property.admin",
        "Declare a property, change its sensitivity.",
    ),
    (
        "core.approval_policy.admin",
        "Create, change or disable an approval policy.",
    ),
    (
        "core.rbac.read",
        "Read the roles, groups, members, bindings, attachments, service accounts and the state of the identities.",
    ),
    (
        "core.rbac.admin",
        "Create groups, set or close their bindings and their members in the scope, for roles whose permissions one holds.",
    ),
    ("core.role.admin", "Define, change or close a role."),
    (
        "core.issuer.admin",
        "Attach an OIDC issuer to entities or to the global scope.",
    ),
    (
        "core.account.admin",
        "Manage the service accounts, disable an identity, revoke its sessions.",
    ),
    ("core.audit.read", "Consult or export the audit journal."),
    (
        "core.identity.pseudonymise",
        "Pseudonymise without delay the identity data of a person.",
    ),
    (
        "core.identity.act_on_behalf",
        "For a service account: write on behalf of a requester named by the Kartenn-On-Behalf-Of header, under its own rights.",
    ),
    (
        "core.observability.admin",
        "Change the filter of a technical output at run time.",
    ),
    (
        "core.draft.admin",
        "Read, delete or hand over to someone else the draft of a person.",
    ),
];

/// English summaries of the rules of the core, by identifier.
pub static RULES_EN: &[(&str, &str)] = &[
    (
        "import.missing-external-id",
        "An import element that creates an object carries the external identifier of its source.",
    ),
    (
        "import.unknown-reference",
        "A local reference of an import names an element of the same import.",
    ),
    (
        "import.order",
        "Each local name of an import is defined once and its references form no cycle.",
    ),
    (
        "import.invalid",
        "The elements of an import sort; any other sort error takes it back to upload.",
    ),
    (
        "core.addr.syntax",
        "An address is written in the strict form of its family.",
    ),
    (
        "core.addr.zone",
        "An address or a prefix carries no zone identifier.",
    ),
    (
        "core.addr.ipv4_mapped",
        "No IPv4-mapped IPv6 prefix or address is stored; the IPv4 value is used.",
    ),
    (
        "core.prefix.syntax",
        "A prefix is written address/length, the length explicit and within the width of the family.",
    ),
    ("core.prefix.unaligned", "A prefix has no host bit set."),
    (
        "core.prefix.duplicate",
        "A prefix exists at most once per domain over any period.",
    ),
    (
        "core.prefix.network_not_leaf",
        "A network contains no other prefix.",
    ),
    (
        "core.prefix.not_empty_for_nature",
        "A prefix changes its nature only if its content allows it.",
    ),
    (
        "core.prefix.p2p_length",
        "Only /31 and /32 (IPv4) or /127 and /128 (IPv6) networks are point-to-point.",
    ),
    (
        "core.prefix.anycast_network",
        "Only networks are marked anycast.",
    ),
    (
        "core.anycast.address_usage",
        "The addresses of an anycast network are of anycast usage.",
    ),
    (
        "core.prefix.site_on_container",
        "Only networks are attached to a site.",
    ),
    (
        "core.range.bounds",
        "A range has bounds of one family, the first at most equal to the last.",
    ),
    (
        "core.range.outside_network",
        "A range lies entirely in a single network of its domain.",
    ),
    (
        "core.range.overlap",
        "The ranges of a network do not overlap.",
    ),
    (
        "core.range.reserved_bounds",
        "A range excludes the network and broadcast addresses (IPv4) and the zero IID (IPv6).",
    ),
    (
        "core.address.outside_network",
        "An address belongs to exactly one network of its domain.",
    ),
    (
        "core.address.duplicate",
        "An address exists at most once per domain over any period.",
    ),
    (
        "core.address.range_usage",
        "Addresses respect the usage of the range they fall in.",
    ),
    (
        "core.address.not_assignable",
        "Network, broadcast, zero IID, loopback, unspecified, reserved and multicast addresses are not assigned.",
    ),
    (
        "core.public.overlap",
        "Public space is unique across domains, except anycast addresses and anycast networks of the same value everywhere.",
    ),
    (
        "core.domain.immutable",
        "An addressing object never changes its domain.",
    ),
    (
        "core.anchor.cycle",
        "The trees of the entities and of the sites have no cycle.",
    ),
    (
        "core.anchor.in_use",
        "An entity or a site is closed only if nothing refers to it.",
    ),
    (
        "core.slug.syntax",
        "A slug has 1 to 63 characters [a-z0-9-], without a hyphen at the start or the end.",
    ),
    (
        "core.slug.duplicate",
        "A slug is unique per kind of object at any time.",
    ),
    (
        "core.external_id.duplicate",
        "An external identifier designates a single object per source.",
    ),
    (
        "core.addr.documentation",
        "Documentation blocks are not used in production.",
    ),
    (
        "core.addr.shared_space",
        "Shared address space is meant for carrier-grade NAT.",
    ),
    (
        "core.prefix.ipv6_long_subnet",
        "IPv6 host networks are /64, but point-to-point /127 and /128.",
    ),
    (
        "core.prefix.ula_global_id",
        "A unique local prefix uses a pseudo-random Global ID.",
    ),
    (
        "core.public.anycast_shared",
        "A public anycast address or network appears in several domains.",
    ),
    (
        "prop.declaration.invalid",
        "A property declaration has a valid key, type and parameters.",
    ),
    (
        "prop.declaration.namespace",
        "A module declares in its namespace, an administrator in the custom namespace with the built-in types only.",
    ),
    (
        "prop.declaration.duplicate",
        "A property key designates a single declaration over any period.",
    ),
    (
        "prop.declaration.values_of",
        "A closed list borrows the values of another closed list of the same declarant only, which borrows from none.",
    ),
    (
        "prop.enum_value.invalid",
        "A closed-list value has a key [a-z0-9-] unique in its property, of closed-list type, valid over its period.",
    ),
    (
        "prop.declaration.incompatible",
        "A change of declaration invalidates no assignment and changes no resolved value without an explicit assignment.",
    ),
    (
        "prop.assignment.not_carrier",
        "A property is assigned only to the kinds of objects that carry it.",
    ),
    (
        "prop.assignment.invalid_value",
        "A value respects its type and its parameters.",
    ),
    (
        "prop.assignment.required",
        "An object in the scope of a mandatory property has a resolved value.",
    ),
    (
        "prop.assignment.forbidden_effect",
        "The author holds the permission of the property on every object whose resolved value changes.",
    ),
    (
        "prop.assignment.dangling_reference",
        "A reference designates an object valid over the whole period of the assignment.",
    ),
    (
        "prop.resolution.expected_mismatch",
        "The resolved value differs from the value expected by the configured source.",
    ),
    (
        "changeset.operation.no_version",
        "An operation targets an object only over a period where it exists.",
    ),
    (
        "changeset.operation.conflict",
        "An operation that gives the expected version of an object fails if the current version differs.",
    ),
    (
        "changeset.state.not_pending",
        "Only a pending changeset can be approved, rejected or withdrawn.",
    ),
    (
        "changeset.state.not_author",
        "Only the author withdraws a pending changeset.",
    ),
    (
        "changeset.approval.self",
        "The author does not apply their own changeset when an approval policy holds it or a module requires its review; they never reject it.",
    ),
    (
        "changeset.apply_own.missing",
        "The author applies their own changeset only if they hold apply_own on everything it touches, objects and effects.",
    ),
    (
        "changeset.approval.forbidden",
        "Whoever applies or rejects the changeset of someone else holds the apply permission of everything it touches.",
    ),
    (
        "core.review.operation",
        "Only the operations a module declares « of review » join the approval of a proposal.",
    ),
    (
        "changeset.policy.invalid",
        "An approval policy has a name and at least one trigger.",
    ),
    (
        "core.approval_policy.trigger",
        "A trigger of a policy names a measure declared by a module of the build.",
    ),
    (
        "core.approval_policy.author",
        "The authors of an approval policy are designated by a role or a group of kartenn in force, never by an account.",
    ),
    (
        "changeset.policy.forbidden",
        "Creating, changing or disabling an approval policy needs the right to administer the policies.",
    ),
    (
        "core.role.invalid",
        "A role has a key, labels and entries of the catalogue; an issuer and a group are not empty.",
    ),
    (
        "core.rbac.forbidden",
        "Administering the roles, issuers, accounts or bindings needs the matching administration permission.",
    ),
    (
        "core.binding.escalation",
        "A role is bound on a scope only by one who holds each of its entries on that scope.",
    ),
    (
        "core.role.builtin",
        "A delivered role changes only by the synchronisation of the core.",
    ),
    ("core.role.unknown", "A binding designates a role in force."),
    (
        "core.binding.requester",
        "A group has no « requester » identity as member: a requester is not an account; the person who later logs in through its issuer becomes one.",
    ),
    (
        "core.binding.group_closed",
        "A binding designates a group in force: a group closed since a pending changeset that binds it was written puts that changeset in conflict at its approval.",
    ),
    (
        "core.group.invalid",
        "A group has a key and labels; a member is an identity or a non-empty group of an issuer, present at most once in a group at a given date.",
    ),
    (
        "core.group.unknown",
        "A membership designates a group in force.",
    ),
    (
        "core.group.nested",
        "A kartenn group is a member of no other: a member is an identity or a group of the issuer, the form of a membership admits nothing else.",
    ),
    (
        "core.group.escalation",
        "A member is added to or removed from a group, or the group closed, only by one who could create each of its bindings: administer the rights on their scope and hold each entry of their roles.",
    ),
    (
        "core.group.builtin",
        "A delivered group, its labels and its bindings change only by the synchronisation of the core; its members are managed as those of any group.",
    ),
    (
        "core.object.forbidden",
        "Creating, changing or closing an object needs the edit permission of its kind on the object.",
    ),
    (
        "core.label.invalid",
        "A label has 1 to 200 characters without control characters, under a BCP 47 language tag; a non-empty table of labels holds the default language.",
    ),
    (
        "core.object.dependents",
        "An object is closed only if no living object of a module refers to it; an explicit cascade closes or detaches the dependents by the policy of each reference.",
    ),
];

/// Kinds of the objects of the core: key, French, English.
pub static KINDS: &[(&str, &str, &str)] = &[
    ("core.domain", "Domaine d'adressage", "Addressing domain"),
    ("core.prefix", "Préfixe", "Prefix"),
    ("core.range", "Plage", "Range"),
    ("core.address", "Adresse", "Address"),
    ("core.entity", "Entité", "Entity"),
    ("core.site", "Site", "Site"),
    ("core.property", "Propriété", "Property"),
    (
        "core.enum_value",
        "Valeur de liste fermée",
        "Closed-list value",
    ),
    ("core.role", "Rôle", "Role"),
    ("core.binding", "Liaison", "Binding"),
    ("core.changeset", "Changeset", "Changeset"),
    ("core.import", "Import", "Import"),
];

/// States of the core: `<object>.<state>`, French, English.
pub static STATES: &[(&str, &str, &str)] = &[
    ("changeset.applied", "Appliqué", "Applied"),
    ("changeset.refused", "Refusé", "Refused"),
    ("changeset.pending", "En attente", "Pending"),
    ("changeset.rejected", "Rejeté", "Rejected"),
    ("changeset.withdrawn", "Retiré", "Withdrawn"),
    ("changeset.expired", "Expiré", "Expired"),
    (
        "changeset.failed_revalidation",
        "Échec de la revalidation",
        "Revalidation failed",
    ),
    ("changeset.kind.direct", "Direct", "Direct"),
    (
        "changeset.kind.approval",
        "Soumis à approbation",
        "Subject to approval",
    ),
    ("changeset.kind.proposal", "Proposition", "Proposal"),
    ("import.uploading", "En dépôt", "Uploading"),
    ("import.sorting", "En tri", "Sorting"),
    ("import.ready", "Prêt", "Ready"),
    ("import.queued", "En file", "Queued"),
    ("import.running", "En cours", "Running"),
    ("import.paused", "En pause", "Paused"),
    ("import.done", "Terminé", "Done"),
    ("import.cancelled", "Annulé", "Cancelled"),
];

/// Natures of the prefixes: key, French, English.
pub static NATURES: &[(&str, &str, &str)] = &[
    ("container", "Conteneur", "Container"),
    ("network", "Réseau", "Network"),
];

/// Usages of the addresses and ranges: `<kind>.<usage>`, French, English.
pub static USAGES: &[(&str, &str, &str)] = &[
    ("address.host", "Hôte", "Host"),
    ("address.gateway", "Passerelle", "Gateway"),
    ("address.vip", "Adresse virtuelle", "Virtual address"),
    ("address.anycast", "Anycast", "Anycast"),
    ("address.reserved", "Réservée", "Reserved"),
    ("range.dhcp", "DHCP", "DHCP"),
    ("range.reserved", "Réservée", "Reserved"),
    ("range.static", "Statique", "Static"),
];

/// The English of `id` in `table`, empty when missing (the completeness test fails then).
#[must_use]
pub fn english(table: &[(&str, &'static str)], id: &str) -> &'static str {
    table
        .iter()
        .find(|(k, _)| *k == id)
        .map_or("", |(_, en)| en)
}

/// Labels of a table `(key, fr, en)` in `space`.
pub fn triples(
    space: &'static str,
    table: &[(&str, &str, &str)],
) -> impl Iterator<Item = CodeLabel> {
    table
        .iter()
        .map(move |(key, fr, en)| CodeLabel::new(space, key, fr, en))
}

/// The English of a module's codes: its permissions and rules by identifier, its kinds
/// and states as `(key, fr, en)`.
#[derive(Debug, Clone, Copy)]
pub struct English {
    /// Permissions, by name.
    pub permissions: &'static [(&'static str, &'static str)],
    /// Rules, by identifier.
    pub rules: &'static [(&'static str, &'static str)],
    /// Kinds of objects.
    pub kinds: &'static [(&'static str, &'static str, &'static str)],
    /// States.
    pub states: &'static [(&'static str, &'static str, &'static str)],
}

/// The labels of the codes of a module: the French of its definitions, the English of
/// `english`.
#[must_use]
pub fn of_module(
    permissions: &[&crate::authz::Permission],
    rules: &[&crate::rules::Rule],
    english_of: &English,
) -> Vec<CodeLabel> {
    permissions
        .iter()
        .map(|p| {
            CodeLabel::new(
                "permission",
                p.name,
                p.summary,
                english(english_of.permissions, p.name),
            )
        })
        .chain(
            rules
                .iter()
                .map(|r| CodeLabel::new("rule", r.id, r.summary, english(english_of.rules, r.id))),
        )
        .chain(triples("kind", english_of.kinds))
        .chain(triples("state", english_of.states))
        .collect()
}

/// The labels of the codes of the core: permissions, rules, kinds, states, natures and
/// usages.
#[must_use]
pub fn core() -> Vec<CodeLabel> {
    let permissions = crate::authz::PERMISSIONS.iter().map(|p| {
        CodeLabel::new(
            "permission",
            p.name,
            p.summary,
            english(PERMISSIONS_EN, p.name),
        )
    });
    let rules = crate::rules::all_rules()
        .chain(crate::rules::import_rules())
        .map(|r| CodeLabel::new("rule", r.id, r.summary, english(RULES_EN, r.id)));
    permissions
        .chain(rules)
        .chain(triples("kind", KINDS))
        .chain(triples("state", STATES))
        .chain(triples("nature", NATURES))
        .chain(triples("usage", USAGES))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REQ-018-10: every code of the core has a French and an English label.
    #[test]
    fn every_code_of_the_core_has_french_and_english_labels() {
        let missing: Vec<String> = core()
            .into_iter()
            .filter(|l| l.fr.is_empty() || l.en.is_empty())
            .map(|l| format!("{}/{}", l.space, l.key))
            .collect();
        assert!(missing.is_empty(), "without both labels: {missing:?}");
        for (table, ids) in [
            (
                PERMISSIONS_EN,
                crate::authz::PERMISSIONS
                    .iter()
                    .map(|p| p.name)
                    .collect::<Vec<_>>(),
            ),
            (
                RULES_EN,
                crate::rules::all_rules()
                    .chain(crate::rules::import_rules())
                    .map(|r| r.id)
                    .collect(),
            ),
        ] {
            let stale: Vec<&str> = table
                .iter()
                .map(|(k, _)| *k)
                .filter(|k| !ids.contains(k))
                .collect();
            assert!(stale.is_empty(), "English of no code: {stale:?}");
        }
    }
}
