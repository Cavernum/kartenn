"""Every operation of the reference contract is classified (REQ-023-43)."""

from __future__ import annotations

import copy

from fakes import CONTRACT, MODULES, capabilities
from kartenn.catalog import derive
from kartenn_cli.coverage import EXCLUDED, reached, unclassified

ALL = capabilities(modules=MODULES)


def test_every_operation_is_reached_or_excluded() -> None:
    catalog = derive("u", ALL, CONTRACT)
    assert unclassified(catalog) == []
    found = reached(catalog)
    assert found["list_prefixes"] == "prefixes list"
    assert found["query_prefixes"] == "prefixes list"
    assert found["list_policy_rules"] == "policy rules list"
    assert found["show_verdicts"] == "policy check"
    assert found["waive_flow"] == "flow waive"
    assert found["submit_changeset"] == "changeset"
    total = len(catalog.operations)
    print(f"{total - len(EXCLUDED)} of {total} operations have a command")  # noqa: T201


def test_an_operation_added_and_not_classified_fails() -> None:
    contract = copy.deepcopy(CONTRACT)
    contract["paths"]["/v1/odd/thing/{id}/{other}"] = {
        "post": {"operationId": "odd_thing", "tags": ["nowhere"], "responses": {}}
    }
    contract["paths"]["/v2/elsewhere"] = {
        "get": {"operationId": "elsewhere", "tags": ["core"], "responses": {}}
    }
    found = unclassified(derive("u", ALL, contract))
    assert found == ["elsewhere"]
    del contract["paths"]["/v1/auth/login"]
    assert "login" not in [o.operation_id for o in derive("u", ALL, contract).operations]
