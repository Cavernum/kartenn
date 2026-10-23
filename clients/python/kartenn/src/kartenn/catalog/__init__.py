"""The catalog of a server, discovered from its capabilities and its contract (T3, T20).

`discover` is a plan (`kartenn._core.plans`): it reads the capabilities at most once a
minute, and the contract only when the revision changes. Same version, same modules:
the contract is asked again under `If-None-Match` when it carries an ETag (304, nothing
downloaded), not at all otherwise.
"""

from __future__ import annotations

from http import HTTPStatus
from typing import TYPE_CHECKING, Any, cast

from kartenn._core.messages import Headers, Request
from kartenn._core.plans import Plan, checked
from kartenn._core.problems import ClientError
from kartenn.catalog.cache import CatalogCache
from kartenn.catalog.derive import derive
from kartenn.catalog.model import Catalog, Module, Operation, Param, Resource, UnknownResource, Verb

if TYPE_CHECKING:
    from kartenn._core.messages import Response

__all__ = [
    "Catalog",
    "CatalogCache",
    "Module",
    "Operation",
    "Param",
    "Resource",
    "UnknownResource",
    "Verb",
    "derive",
    "discover",
]


def _object(response: Response, what: str) -> dict[str, Any]:
    data = checked(response).json()
    if not isinstance(data, dict):
        msg = f"the {what} of the server is not a JSON object"
        raise ClientError("client.unexpected-answer", msg)
    return cast("dict[str, Any]", data)


def _modules(capabilities: dict[str, Any]) -> set[tuple[str, str]]:
    found: set[tuple[str, str]] = set()
    for raw in capabilities.get("modules", []):
        if isinstance(raw, dict):
            module = cast("dict[str, Any]", raw)
            found.add((str(module.get("name")), str(module.get("version", ""))))
    return found


def discover(server: str, cache: CatalogCache, *, refresh: bool = False) -> Plan[Catalog]:
    """The catalog of `server`, from the cache when it still holds."""
    if not refresh and (fresh := cache.fresh(server)) is not None:
        return fresh
    capabilities = _object((yield Request("GET", "/v1/capabilities")), "capabilities")
    known = cache.last(server)
    contract_request = Request("GET", "/v1/openapi.json")
    if (
        known is not None
        and known.version == str(capabilities.get("version", ""))
        and {(m.name, m.version) for m in known.modules} == _modules(capabilities)
    ):
        current = known.with_capabilities(capabilities)
        if known.contract_etag is None:
            cache.touch(current)
            return current
        headers = Headers.of({"If-None-Match": known.contract_etag})
        answer = yield Request("GET", "/v1/openapi.json", headers=headers)
        if answer.status == HTTPStatus.NOT_MODIFIED:
            cache.touch(current)
            return current
    else:
        answer = yield contract_request
    contract = _object(answer, "contract")
    catalog = derive(server, capabilities, contract, contract_etag=answer.etag)
    cache.save(catalog)
    return catalog
