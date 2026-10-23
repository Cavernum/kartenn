//! The English of the codes of the module (spec 018, R8 b): its permissions and rules
//! beside their French definitions, its kinds.

use kartenn_core::code_labels::English;

/// The English of the module.
pub(crate) static ENGLISH: English = English {
    permissions: &[
        ("machines.cluster.read", "Read the clusters."),
        (
            "machines.cluster.write",
            "Create, change or close a cluster.",
        ),
        ("machines.cluster.propose", "Propose a change of a cluster."),
        ("machines.read", "Read the machines."),
        ("machines.write", "Create, change or close a machine."),
        ("machines.propose", "Propose a change of a machine."),
        ("machines.interface.read", "Read the interfaces."),
        (
            "machines.interface.write",
            "Create, change or close an interface.",
        ),
        (
            "machines.interface.propose",
            "Propose a change of an interface.",
        ),
    ],
    rules: &[
        (
            "machines.operation.invalid",
            "An operation of the module carries the fields it expects, with admitted values and objects that exist.",
        ),
        (
            "machines.name.invalid",
            "A machine name is a sequence of LDH labels of 1 to 63 characters, 253 in all, kept in lower case.",
        ),
        (
            "machines.name.duplicate",
            "Two living machines of the same resolved owner do not carry the same name over overlapping periods.",
        ),
        (
            "machines.cluster.duplicate",
            "A cluster slug is unique at any time.",
        ),
        (
            "machines.host.invalid",
            "A machine is hosted only in the allowed combinations, never by itself nor in a cycle, nor by a closed host.",
        ),
        (
            "machines.interface.duplicate",
            "A machine does not carry two interfaces of the same name at the same time.",
        ),
        (
            "machines.mac.invalid",
            "The MAC address of an interface is a unicast EUI-48 written in hexadecimal.",
        ),
        (
            "machines.vlan.mode",
            "The 802.1Q mode of an interface and its VLANs agree, and its VLANs are alive over the period.",
        ),
        (
            "machines.address.taken",
            "A non-shared address is attached to one interface at a time.",
        ),
        (
            "machines.address.primary",
            "An interface has at most one primary address per family.",
        ),
        (
            "machines.address.closed",
            "An attached address is alive over the whole period of the attachment.",
        ),
        (
            "machines.mac.duplicate",
            "A universal MAC is carried by one living interface only.",
        ),
        (
            "machines.address.vlan_mismatch",
            "The network of an attached address carries no VLAN foreign to the interface.",
        ),
        (
            "machines.host.site_mismatch",
            "A hosted machine and its host have the same resolved site.",
        ),
    ],
    kinds: &[
        ("machines.cluster", "Cluster", "Cluster"),
        ("machines.machine", "Machine", "Machine"),
        ("machines.interface", "Interface", "Interface"),
    ],
    states: &[],
};
