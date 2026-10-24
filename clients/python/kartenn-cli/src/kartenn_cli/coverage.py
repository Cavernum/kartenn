"""Coverage of the API by the CLI (REQ-023-43; design § Couverture de l'API).

Every operation of the contract is reached by a derived command, a named command or a
command of the base, or is excluded here with its reason; all of them stay reachable by
`kartenn api`. `unclassified(catalog)` names the operations that are neither: a test
fails on any.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kartenn.catalog import Catalog

__all__ = ["BASE", "EXCLUDED", "reached", "unclassified"]

# Operations reachable by `kartenn api` only, and why.
EXCLUDED: dict[str, str] = {
    "login": "flow of the BFF for the browser (005/K1): the CLI asks the issuer itself (T10)",
    "login_callback": "flow of the BFF for the browser (005/K1)",
    "login_issuers": "login page of the web; the CLI reads the native clients (A5)",
    "query_verdicts": "QUERY twin of show_verdicts; policy check sends short filters by GET",
    "labels": "dictionary of the 018, read by the tables of the CLI (`client.labels()`, T15)",
    # Spec 024: a draft is composed in the interface or by the library (`kn.drafts`).
    "create_draft": "a draft is composed in the interface or by the library (`kn.drafts`)",
    "update_draft": "a draft is composed in the interface or by the library (`kn.drafts`)",
    "add_draft_operations": "a draft is composed in the interface or by the library",
    "simulate_draft": "simulated by the interface while it is composed",
    "take_over_draft": "an action of the administration, rare (`kartenn api`)",
}

# Commands of the base, written by hand, and the operations they reach.
BASE: dict[str, tuple[str, ...]] = {
    "changeset": (
        "submit_changeset",
        "show_changeset",
        "list_changesets",
        "changeset_diff",
        "validate_changeset",
        "reject_changeset",
        "withdraw_changeset",
        "undo_changeset",
        "list_proposals",
    ),
    "import": (
        "create_import",
        "deposit_import_chunk",
        "start_import",
        "show_import",
        "resume_import",
        "cancel_import",
        "list_imports",
    ),
    # The drafts kept by the server (spec 024).
    "drafts": ("list_drafts", "get_draft", "submit_draft", "delete_draft"),
    "api spec": ("openapi",),
    "capabilities": ("capabilities",),
    # The counters of the home page (spec 024).
    "stats": ("stats",),
    # The stream of the 022 (`GET /v1/events`).
    "watch": ("events",),
    "health": ("live", "ready"),
    "login": ("breakglass", "native_clients", "me"),
    "logout": ("logout",),
    "me": ("me",),
}


def reached(catalog: Catalog) -> dict[str, str]:
    """The operations the CLI reaches, and the command that does."""
    from kartenn_cli.named import NAMED, replaced  # noqa: PLC0415
    from kartenn_cli.tree import VERB_ALIASES, layout  # noqa: PLC0415

    found: dict[str, str] = {}
    shape = layout(catalog)
    places: list[tuple[str, str]] = [(name, res) for name, res in shape.resources.items()]
    for module, children in shape.modules.items():
        hidden = replaced(module, catalog)
        places += [
            (f"{module} {child}", res) for child, res in children.items() if child not in hidden
        ]
    places += [(module, res) for module, res in shape.mains.items()]
    for path, name in places:
        resource = catalog.resource(name)
        for verb in resource.verbs:
            alias = VERB_ALIASES.get(verb.name, verb.name)
            shown = "list" if verb.name == "query" else (alias or verb.name)
            found.setdefault(verb.operation_id, f"{path} {shown.replace('.', ' ')}")
    for entry in NAMED:
        for operation in entry.operation_ids:
            found.setdefault(operation, f"{entry.module} {entry.name}")
    for command, operations in BASE.items():
        for operation in operations:
            found.setdefault(operation, command)
    return found


def unclassified(catalog: Catalog) -> list[str]:
    """The operations of the contract neither reached nor excluded."""
    known = set(reached(catalog)) | set(EXCLUDED)
    return sorted(o.operation_id for o in catalog.operations if o.operation_id not in known)
