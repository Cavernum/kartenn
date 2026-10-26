//! The English of the codes of the module (spec 018, R8 b): its permissions and rules
//! beside their French definitions, its kind and the states, verdicts and outcomes of a
//! flow.

use kartenn_core::code_labels::English;

/// The English of the module.
pub(crate) static ENGLISH: English = English {
    permissions: &[
        ("flow.read", "Read the flows."),
        (
            "flow.request",
            "Request a flow without reading its endpoints, and read one's own requests.",
        ),
        ("flow.review", "Review a flow request."),
        ("flow.waiver.grant", "Grant a waiver to a denied flow."),
        (
            "flow.edit",
            "Write a flow directly (maintenance) in a changeset and submit it, with the read of its two endpoints.",
        ),
        (
            "flow.apply",
            "Apply or reject the changeset of someone else that writes a flow directly.",
        ),
        (
            "flow.approval.self",
            "Apply one's own changeset that writes a flow directly.",
        ),
    ],
    rules: &[
        (
            "flow.endpoint.invalid",
            "An endpoint is an address, a prefix or, with the Machines module, an interface, valid over the whole period of the flow; never a range.",
        ),
        (
            "flow.endpoint.machine",
            "A machine is not an endpoint: a flow opens on an address, an interface or a subnet, never on every address of a machine.",
        ),
        (
            "flow.service.closed",
            "The service of a flow is active over its whole period.",
        ),
        (
            "flow.reference.missing",
            "A flow carries at least one external reference.",
        ),
        (
            "flow.reference.system",
            "An external reference names its system by the key of a value of the closed list of the external systems (or by its identifier) valid at the start of the operation.",
        ),
        (
            "flow.field.invalid",
            "A justification has 1 to 4,000 characters; an external reference has an identifier of 1 to 256 characters; without control characters.",
        ),
        (
            "flow.kind.reserved",
            "The « observed » kind is reserved to v0.3.",
        ),
        (
            "flow.update.identity",
            "Changing an endpoint or the service of a flow makes a new one: the change is refused.",
        ),
        (
            "flow.update.ended",
            "An ended flow is not extended: a new request replaces it.",
        ),
        (
            "flow.review.required",
            "A flow whose outcome is not automatic applies only after review.",
        ),
        (
            "flow.review.end_required",
            "A reviewed flow has an end date, at most at the ceiling of the review.",
        ),
        (
            "flow.waiver.required",
            "A flow the policy denies is recorded or extended only with a waiver.",
        ),
        (
            "flow.verdict.denied",
            "A recorded flow the policy now denies, without a valid waiver, is non-compliant: an anomaly, closed when it complies again.",
        ),
        (
            "flow.verdict.partial",
            "Only some of the pairs of addresses of the flow are allowed.",
        ),
        (
            "flow.waiver.invalid",
            "A waiver has a motive and an accepted risk, an end no earlier than that of the flow and the ceiling, and an author other than the requester.",
        ),
        (
            "flow.separation",
            "The reviewer of a request is not its requester.",
        ),
        (
            "flow.referenced",
            "An object that is an endpoint of an active or future flow is not closed without an explicit cascade.",
        ),
    ],
    kinds: &[("flow.flow", "Flux", "Flow")],
    states: &[
        ("flow.requested", "Demandé", "Requested"),
        ("flow.rejected", "Rejeté", "Rejected"),
        ("flow.scheduled", "Planifié", "Scheduled"),
        ("flow.active", "Actif", "Active"),
        ("flow.noncompliant", "Non conforme", "Non-compliant"),
        ("flow.waived", "Dérogé", "Waived"),
        ("flow.expired", "Expiré", "Expired"),
        ("flow.withdrawn", "Retiré", "Withdrawn"),
        ("flow.verdict.allowed", "Autorisé", "Allowed"),
        ("flow.verdict.partial", "Partiel", "Partial"),
        ("flow.verdict.denied", "Refusé", "Denied"),
        ("flow.outcome.automatic", "Automatique", "Automatic"),
        ("flow.outcome.review", "À instruire", "To review"),
        ("flow.outcome.noncompliant", "Non conforme", "Non-compliant"),
        ("flow.kind.requested", "Demandé", "Requested"),
        ("flow.kind.observed", "Constaté", "Observed"),
    ],
};
