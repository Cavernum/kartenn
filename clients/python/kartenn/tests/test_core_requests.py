import json
import re
from typing import Any, cast
from urllib.parse import parse_qsl

import pytest
from hypothesis import given
from hypothesis import strategies as st

from kartenn._core.messages import Headers, Request, Response
from kartenn._core.problems import ClientError
from kartenn._core.requests import (
    URL_LIMIT,
    Filter,
    between,
    encode,
    exists,
    filters_from,
    idempotency_key,
    list_request,
    parse_prop,
    traceparent,
    user_agent,
)

texts = st.text(
    alphabet=st.characters(codec="utf-8", exclude_categories=("Cs", "Cc")), min_size=1, max_size=12
)
fields = st.from_regex(r"[a-z][a-z_]{0,8}", fullmatch=True).filter(
    lambda name: name not in {"valid_at", "as_of", "cursor", "page_size"}
)
keys = st.from_regex(r"[a-z][a-z0-9_]{0,5}\.[a-z][a-z0-9_]{0,5}", fullmatch=True)


def eq_filter(field: str, values: list[str]) -> Filter:
    return Filter(field, "eq", tuple(values))


def between_filter(key: str, low: str | None, high: str) -> Filter:
    return Filter(f"prop.{key}", "between", (low, high))


def exists_filter(key: str, flag: bool) -> Filter:  # noqa: FBT001 - a strategy target
    return Filter(f"prop.{key}", "exists", flag)


eq_filters = st.builds(eq_filter, fields, st.lists(texts, min_size=1, max_size=3))
between_filters = st.builds(
    between_filter,
    keys,
    st.one_of(st.none(), st.integers(0, 10_000).map(str)),
    st.integers(0, 10_000).map(str),
)
exists_filters = st.builds(exists_filter, keys, st.booleans())
any_filters = st.lists(st.one_of(eq_filters, between_filters, exists_filters), max_size=6)


def conditions_of_url(params: list[tuple[str, str]]) -> list[dict[str, Any]]:
    """The QUERY conditions a server reads from a query string (006 § Filtres)."""
    found: list[dict[str, Any]] = []
    for name, value in params:
        if name in {"valid_at", "as_of", "page_size"}:
            continue
        field, _, op = name.partition(":")
        if op == "between":
            low, _, high = value.partition("..")
            found.append({"field": field, "op": "between", "value": [low or None, high or None]})
        elif op == "exists":
            found.append({"field": field, "op": "exists", "value": value == "true"})
        else:
            found.append({"field": field, "op": "eq", "value": [value]})
    return found


def flatten(conditions: list[dict[str, Any]]) -> list[dict[str, Any]]:
    flat: list[dict[str, Any]] = []
    for condition in conditions:
        if condition["op"] == "eq":
            value: Any = condition["value"]
            values: list[Any] = cast("list[Any]", value) if isinstance(value, list) else [value]
            flat.extend({"field": condition["field"], "op": "eq", "value": [v]} for v in values)
        else:
            flat.append(condition)
    return flat


@given(any_filters)
def test_url_and_query_body_say_the_same(filters: list[Filter]) -> None:
    short = list_request("/v1/prefixes", filters, valid_at="2026-01-01T00:00:00Z", page_size=10)
    assert short.method == "GET"
    from_url = conditions_of_url(parse_qsl(encode(short.params), keep_blank_values=True))
    long = list_request(
        "/v1/prefixes",
        filters,
        valid_at="2026-01-01T00:00:00Z",
        page_size=10,
        query=True,
        base="x" * URL_LIMIT,
    )
    assert long.method == "QUERY"
    assert long.headers.get("content-type") == "application/json"
    body = json.loads(long.content or b"")
    assert body["valid_at"] == "2026-01-01T00:00:00Z"
    assert body["page_size"] == 10
    assert flatten(body["filters"]) == from_url


def test_long_url_without_query_is_refused() -> None:
    filters = [Filter("value", "eq", ("192.0.2.0/24",) * 600)]
    with pytest.raises(ClientError) as caught:
        list_request("/v1/prefixes", filters)
    assert caught.value.code == "client.query-unsupported"
    assert list_request("/v1/prefixes", filters, query=True).method == "QUERY"


def test_colon_stays_literal() -> None:
    request = list_request("/v1/prefixes", [parse_prop("dns.ttl:between=60..3600")])
    assert encode(request.params) == "prop.dns.ttl:between=60..3600"


def test_filters_from_keywords() -> None:
    found = filters_from(
        {"domain": "interne", "usage": ["a", "b"], "nature": None},
        {"dns.ttl": between(60, None), "dns.fqdn": exists(present=False), "policy.env": "prod"},
    )
    assert [f.params() for f in found] == [
        [("domain", "interne")],
        [("usage", "a"), ("usage", "b")],
        [("prop.dns.ttl:between", "60..")],
        [("prop.dns.fqdn:exists", "false")],
        [("prop.policy.env", "prod")],
    ]
    assert found[1].condition() == {"field": "usage", "op": "eq", "value": ["a", "b"]}
    assert found[3].condition() == {"field": "prop.dns.fqdn", "op": "exists", "value": False}
    assert Filter("public", "eq", (True,)).params() == [("public", "true")]


def test_unknown_filter_is_refused_with_the_known_ones() -> None:
    with pytest.raises(ClientError) as caught:
        filters_from({"colour": "red"}, known=["domain", "value"])
    assert caught.value.code == "client.unknown-filter"
    assert "domain, value" in str(caught.value)


@pytest.mark.parametrize(
    "members", [{"cursor": "x"}, {"valid_at": "2026-01-01T00:00:00Z"}, {"usage": []}]
)
def test_reserved_or_empty_filters(members: dict[str, object]) -> None:
    with pytest.raises(ClientError):
        filters_from(members)


@pytest.mark.parametrize(
    "text",
    [
        "dns",
        "Dns.ttl=1",
        "dns.ttl:between=..",
        "dns.ttl:between=5",
        "dns.ttl:exists=yes",
        "dns.ttl:like=a",
    ],
)
def test_bad_property_filters(text: str) -> None:
    with pytest.raises(ClientError):
        parse_prop(text)


def test_property_filters() -> None:
    assert parse_prop("policy.environment=prod") == Filter(
        "prop.policy.environment", "eq", ("prod",)
    )
    assert parse_prop("dns.ttl:eq=5").op == "eq"
    assert parse_prop("dns.ttl:between=..10").value == (None, "10")
    assert parse_prop("dns.fqdn:exists=true").value is True
    with pytest.raises(ClientError):
        filters_from({}, {"bad key": 1})
    with pytest.raises(ClientError):
        between()


def test_headers() -> None:
    assert re.fullmatch(r"00-[0-9a-f]{32}-[0-9a-f]{16}-01", traceparent())
    assert re.fullmatch(r"[0-9a-f-]{36}", idempotency_key())
    assert user_agent("1.2").startswith("kartenn-python/1.2 python/")


def test_messages() -> None:
    headers = Headers.of({"Content-Type": "a"}).set("content-type", "b").setdefault("X", "1")
    assert headers.get("CONTENT-TYPE") == "b"
    assert headers.setdefault("x", "2").get("x") == "1"
    assert "x" in headers
    assert 3 not in headers
    assert headers.without("x").get("x") is None
    assert Headers.of([("A", "1"), ("a", "2")]).get_all("a") == ["1", "2"]
    assert Headers.of(None).items == ()
    assert list(Headers.of({"A": "1"})) == [("A", "1")]
    assert not Request("POST", "/v1/x").replayable
    assert Request("POST", "/v1/x").with_header("Idempotency-Key", "k").replayable
    assert Request("QUERY", "/v1/x").with_default_header("A", "1").replayable
    assert Response(204).json() is None
    assert Response(200, Headers.of({"ETag": '"v"'}), b"[1]").json() == [1]
    assert Response(200, Headers.of({"ETag": '"v"'})).etag == '"v"'
