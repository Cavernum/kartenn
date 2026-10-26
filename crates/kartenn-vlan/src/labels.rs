//! The English of the codes of the module (spec 018, R8 b): its permissions and rules
//! beside their French definitions, its kinds.

use kartenn_core::code_labels::English;

/// The English of the module.
pub(crate) static ENGLISH: English = English {
    permissions: &[
        ("vlan.group.read", "Read the VLAN groups."),
        (
            "vlan.group.edit",
            "Create, change or close a VLAN group in a changeset and submit it.",
        ),
        (
            "vlan.group.apply",
            "Apply or reject the changeset of someone else that touches a VLAN group.",
        ),
        (
            "vlan.group.approval.self",
            "Apply one's own changeset that touches a VLAN group.",
        ),
        ("vlan.read", "Read the VLANs."),
        (
            "vlan.edit",
            "Create, change or close a VLAN in a changeset and submit it.",
        ),
        (
            "vlan.apply",
            "Apply or reject the changeset of someone else that touches a VLAN.",
        ),
        (
            "vlan.approval.self",
            "Apply one's own changeset that touches a VLAN.",
        ),
    ],
    rules: &[
        ("vlan.vid.invalid", "A VID is an integer from 1 to 4094."),
        (
            "vlan.vid.out_of_range",
            "A VID lies in the range of its group, and the range of a group keeps its VLANs.",
        ),
        (
            "vlan.vid.duplicate",
            "A VID is alive at most once per group at any time.",
        ),
        (
            "vlan.name.invalid",
            "A VLAN name has 1 to 64 characters, without control characters.",
        ),
        (
            "vlan.name.duplicate",
            "A VLAN name is alive at most once per group at any time.",
        ),
        (
            "vlan.group.anchor",
            "A group is anchored on an entity, a site or a domain alive over its whole period.",
        ),
        (
            "vlan.prefix.not_network",
            "The property vlan.vlan is set on a network only.",
        ),
        (
            "vlan.site.mismatch",
            "The site of a network and that of the group of its VLAN agree.",
        ),
        (
            "vlan.domain.mismatch",
            "The domain of a network and that of the group of its VLAN agree.",
        ),
    ],
    kinds: &[
        ("vlan.group", "Groupe de VLAN", "VLAN group"),
        ("vlan.vlan", "VLAN", "VLAN"),
    ],
    states: &[],
};
