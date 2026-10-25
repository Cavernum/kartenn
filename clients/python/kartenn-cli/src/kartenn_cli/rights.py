"""Groups of kartenn and their members (spec 025, REQ-025-11): what the derivation of the
contract does not say.

- a group is named by its UUID or its key (`CLIENT_KEYS` of `reads`);
- a member of a group (`groups members get|remove GROUP MEMBER`) by the UUID of its
  membership, the UUID of its identity, or `issuer/group`, matched on the members;
- `groups members add GROUP MEMBER`: an identity by its UUID, a group of an issuer as
  `issuer/group`, cut at the last `/` (`--issuer` and `--group` for a group holding one);
- `--into-changeset`: the operations of the core for groups, members and bindings
  (`set_group`, `close_group`, `add_member`, `remove_member`, `bind`, `unbind`);
- `identities get` in a table: groups as `key (via)`, bindings as `role (group, scope)`.
"""

from __future__ import annotations

import re
from typing import TYPE_CHECKING, Any, cast

from kartenn_cli.errors import NOT_FOUND, USAGE, CliError
from kartenn_cli.i18n import t

if TYPE_CHECKING:
    from collections.abc import Mapping

    from kartenn import Client
    from kartenn.catalog import Operation
    from kartenn_cli.render import Output

__all__ = [
    "group_of",
    "member_body",
    "member_id",
    "rights_operation",
    "shown_record",
    "sub_ref",
]

_UUID = re.compile(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
# The operations whose path parameter `member` names a membership.
_MEMBER_PATHS = frozenset({"show_member", "remove_member"})


# Members ------------------------------------------------------------------------------


def _names(member: Mapping[str, Any]) -> set[str]:
    """What names a membership: its UUID, then its identity or `issuer/group`."""
    names = {str(member.get("id"))}
    if member.get("kind") == "identity":
        names.add(str(member.get("identity")))
    else:
        names.add(f"{member.get('issuer')}/{member.get('group')}")
    return names


def member_id(client: Client, group: str, text: str) -> str:
    """The membership of `group` named by `text`."""
    found = [
        str(member["id"])
        for member in client.resource("groups").pages_of("members", group)
        if text in _names(member)
    ]
    if not found:
        raise CliError(
            "client.reference-not-found",
            t("ref.not_found", ref=text, resource="groups members"),
            exit_code=NOT_FOUND,
        )
    if len(found) > 1:
        raise CliError(
            "client.ambiguous-reference",
            t("ref.ambiguous", ref=text, resource="groups members"),
            exit_code=USAGE,
        )
    return found[0]


def sub_ref(client: Client, operation: Operation, name: str, value: str, parent: str | None) -> str:
    """A path parameter after the first: a member resolved, any other as given."""
    if name == "member" and parent is not None and operation.operation_id in _MEMBER_PATHS:
        return member_id(client, parent, value)
    return value


def member_body(text: str) -> dict[str, str]:
    """The body of a membership from `MEMBER`: an identity, or `issuer/group`."""
    if _UUID.fullmatch(text):
        return {"identity": text}
    issuer, sep, group = text.rpartition("/")
    if not sep or not issuer or not group:
        raise CliError("client.invalid-reference", t("group.member_shape", ref=text))
    return {"issuer": issuer, "group": group}


def _member_operand(members: Mapping[str, Any]) -> dict[str, Any]:
    """`GroupMember` of an operation from the body of `POST …/members`."""
    if members.get("identity") is not None:
        return {"kind": "identity", "id": members["identity"]}
    if members.get("issuer") is not None and members.get("group") is not None:
        return {"kind": "idp_group", "issuer": members["issuer"], "group": members["group"]}
    raise CliError("client.invalid-reference", t("group.member_missing"))


def _shown_member(members: Mapping[str, Any]) -> str:
    if members.get("identity") is not None:
        return str(members["identity"])
    return f"{members.get('issuer')}/{members.get('group')}"


# Changeset operations -----------------------------------------------------------------


def _group_key(client: Client, identifier: str | None) -> str:
    return str(client.resource("groups").get(identifier)["key"])


def _keyed(name: str, key: str, members: Mapping[str, Any]) -> tuple[dict[str, Any], str]:
    """The operations of an existing group, named by its key."""
    if name == "set_group":
        return {"op": "set_group", "key": key, "labels": members.get("labels")}, key
    if name == "close_group":
        return {"op": "close_group", "key": key}, key
    operand = _member_operand(members)
    shown = f"{key} + {_shown_member(members)}"
    return {"op": "add_member", "group": key, "member": operand}, shown


def rights_operation(
    client: Client,
    operation: Operation,
    members: Mapping[str, Any],
    identifier: str | None,
    rest: Mapping[str, str],
) -> tuple[dict[str, Any], str] | None:
    """The operation of a changeset and its summary for a write of the rights; `None`
    for any other write."""
    name = operation.operation_id
    if name in {"set_group", "close_group", "add_member"}:
        return _keyed(name, _group_key(client, identifier), members)
    if name == "create_group":
        key = members.get("key")
        return {"op": "set_group", "key": key, "labels": members.get("labels")}, str(key)
    if name == "remove_member":
        return {"op": "remove_member", "id": rest["member"]}, rest["member"]
    if name == "bind":
        shown = f"{members.get('group')} {members.get('role')}"
        return {"op": "bind", **members}, shown
    if name == "unbind":
        return {"op": "unbind", "id": identifier}, str(identifier)
    return None


# Rendering ----------------------------------------------------------------------------


def group_of(group: Mapping[str, Any]) -> str:
    """A group of kartenn of an identity (`GroupOf`): `key`, or `key (via group)`."""
    via = group.get("via")
    return str(group.get("key")) if via == "direct" else f"{group.get('key')} ({via})"


def _held(binding: Mapping[str, Any]) -> str:
    """A binding that serves an identity (`HeldBinding`): `role (group, scope)`."""
    scope = cast("Mapping[str, Any]", binding.get("scope") or {})
    where = str(scope.get("kind", "?"))
    if scope.get("id") is not None:
        where = f"{where} {scope['id']}"
    return f"{binding.get('role')} ({binding.get('group')}, {where})"


def shown_record(operation_id: str, record: dict[str, Any], output: Output) -> dict[str, Any]:
    """The record a table shows: an identity with its groups and bindings in words; the
    data formats keep the answer as it is."""
    if not output.table or operation_id != "show_identity":
        return record
    shown = dict(record)
    groups = cast("list[Mapping[str, Any]]", record.get("groups") or [])
    bindings = cast("list[Mapping[str, Any]]", record.get("bindings") or [])
    shown["groups"] = [group_of(g) for g in groups]
    shown["bindings"] = [_held(b) for b in bindings]
    return shown
