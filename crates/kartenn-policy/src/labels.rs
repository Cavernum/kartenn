//! The English of the codes of the module (spec 018, R8 b): its permissions and rules
//! beside their French definitions, its kinds.

use kartenn_core::code_labels::English;

/// The English of the module.
pub(crate) static ENGLISH: English = English {
    permissions: &[
        (
            "policy.catalogue.read",
            "Read the policy: axes, values, flow types, services, rules, patterns, conditions and signals.",
        ),
        (
            "policy.verdict.read",
            "Ask for a compliance verdict between two endpoints one reads.",
        ),
        (
            "policy.flow.request",
            "Request a flow for an entity: verdict of any pair, explanation reduced to values, rules, patterns and conditions.",
        ),
        (
            "policy.axis.admin",
            "Designate, change or remove an axis, describe its values.",
        ),
        (
            "policy.environment.admin",
            "Create, describe or close an environment.",
        ),
        (
            "policy.flow_type.admin",
            "Create, change or close a flow type.",
        ),
        ("policy.service.admin", "Create, change or close a service."),
        (
            "policy.rule.admin",
            "Create, change or close a rule, a pattern, an admissibility condition, a risk signal or an exempt role.",
        ),
        (
            "policy.inference.admin",
            "Write the inference rules that cite a domain of the scope.",
        ),
    ],
    rules: &[
        (
            "policy.key.syntax",
            "A key of the policy has 1 to 63 characters [a-z0-9] and hyphens neither doubled nor at an end.",
        ),
        (
            "policy.service.entry_invalid",
            "An entry of a service has a known transport, ports 1–65535 for tcp, udp, sctp and dccp, an ICMP type and code 0–255 without port, an IP protocol 0–255 without port.",
        ),
        (
            "policy.service.flow_type",
            "A service has exactly one flow type.",
        ),
        (
            "policy.service.entries_empty",
            "A service has at least one transport and ports entry.",
        ),
        (
            "policy.service.wide_ports",
            "A service that covers more than 1,024 ports of one transport opens wide.",
        ),
        (
            "policy.rule.flow_selector_empty",
            "A rule cites at least one flow type or service.",
        ),
        (
            "policy.rule.unknown_axis",
            "A rule cites only properties designated as axes of the policy.",
        ),
        (
            "policy.rule.term_empty",
            "A term of an endpoint selector admits at least one value.",
        ),
        (
            "policy.rule.conditional_deny",
            "Only an allow may require patterns.",
        ),
        (
            "policy.matrix.stale",
            "The write of a cell that carries the fingerprint of the matrix of its flow type applies only if the matrix has not changed since.",
        ),
        (
            "policy.text.missing",
            "A deny, a pattern and a condition carry a pedagogical text in the default language.",
        ),
        (
            "policy.text.invalid",
            "A pedagogical text is not empty, without control characters but line breaks, of at most 4,000 characters.",
        ),
        (
            "policy.condition.params_invalid",
            "The parameters of an admissibility condition follow the schema of its kind.",
        ),
        (
            "policy.signal.params_invalid",
            "The parameters of a signal follow the schema of its kind.",
        ),
        (
            "policy.inference.regex_invalid",
            "An inference regular expression is anchored, of at most 256 characters, in the syntax of the linear-time engine.",
        ),
        (
            "policy.inference.depth",
            "The exceptions of an inference rule nest on 4 levels at most.",
        ),
        (
            "policy.inference.criterion_invalid",
            "An inference criterion admits at least one value, and a rule at least one expected value.",
        ),
        (
            "policy.machine.environment_mismatch",
            "The addresses of a machine, but the interfaces of an exempt role, resolve the environment it declares, or, without declaration, one same environment.",
        ),
        (
            "policy.inference.conflict",
            "Inference rules of the same priority, at the same node, expect different sets.",
        ),
        (
            "policy.inference.mismatch",
            "The heirs of a declared value of policy.environment carry the value the inference expects; the anomaly, on the node of origin, counts the heirs that differ.",
        ),
        (
            "policy.operation.invalid",
            "An operation of the policy has a body that follows its schema and cites known objects.",
        ),
        (
            "policy.axis.invalid",
            "An axis is a living property of closed list or external reference, carried by the addressing tree or by the machine as its carrier says; policy.environment is an imposed axis.",
        ),
        (
            "policy.value.unknown",
            "The attributes of a value describe a living value of an axis.",
        ),
        (
            "policy.key.duplicate",
            "The key of an object of the policy is unique over its period among the objects of its type.",
        ),
        (
            "policy.endpoint.machine",
            "An endpoint of a query of compliance is an address, a prefix or an interface; never a machine, whose opening would aim at every address.",
        ),
        (
            "policy.reference.closed",
            "An object of the policy cites only values, flow types, services, patterns and rules alive over its period.",
        ),
    ],
    kinds: &[
        ("policy.axis", "Axe", "Axis"),
        ("policy.value", "Valeur d'axe", "Axis value"),
        ("policy.flow_type", "Type de flux", "Flow type"),
        ("policy.service", "Service", "Service"),
        ("policy.pattern", "Motif", "Pattern"),
        ("policy.rule", "Règle", "Rule"),
        (
            "policy.condition",
            "Condition d'admissibilité",
            "Admissibility condition",
        ),
        ("policy.signal", "Signal de risque", "Risk signal"),
        ("policy.exempt_role", "Rôle exempté", "Exempt role"),
        (
            "policy.inference_rule",
            "Règle d'inférence",
            "Inference rule",
        ),
    ],
    states: &[],
};
