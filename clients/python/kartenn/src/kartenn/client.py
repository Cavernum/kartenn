"""The clients, synchronous and asynchronous, over the same core (REQ-023-4, 9 to 16).

```python
with Client.from_profile("prod") as kn:
    at = kn.at(valid_at="2026-11-01T00:00:00+01:00")
    for p in at.prefixes.list(domain="interne"):
        ...
    vlan = kn.vlans.get(identifier)
    kn.vlans.update(vlan, {"name": "admin"}, motive="renaming")   # If-Match = vlan.etag
```
"""

from __future__ import annotations

import copy
import os
from importlib.metadata import version
from typing import TYPE_CHECKING, Any, Self, TypeVar

from kartenn import changesets, events, imports
from kartenn._core.plans import arun, run
from kartenn._core.problems import ClientError
from kartenn._core.requests import user_agent
from kartenn._core.sse import EventTracker
from kartenn.auth import Anonymous
from kartenn.auth.basic import BasicAuth, Secret
from kartenn.auth.bearer import BearerToken
from kartenn.auth.oidc_native import Issuer, OidcNative
from kartenn.auth.store import MemoryStore
from kartenn.labels import invalidating
from kartenn.profiles import Profile, read_profile
from kartenn.resources import AsyncResource, Context, Resource
from kartenn.retry import AsyncSender, RetryPolicy, Sender

if TYPE_CHECKING:
    import os as _os
    from collections.abc import AsyncIterator, Callable, Iterable, Iterator, Mapping
    from datetime import datetime
    from pathlib import Path
    from types import TracebackType

    from kartenn._core.messages import Request, Response
    from kartenn._core.plans import Plan
    from kartenn.auth import Auth, TokenStore
    from kartenn.catalog import Catalog, CatalogCache
    from kartenn.labels import Dictionary
    from kartenn.transport.port import AsyncTransport, Transport

__all__ = ["AsyncClient", "Client", "auth_of_profile", "store_key"]

T = TypeVar("T")
_UNAUTHORIZED = 401


def _agent() -> str:
    return user_agent(version("kartenn"))


def _attribute_to_resource(attribute: str) -> str:
    return attribute.replace("_", "-")


def store_key(profile: str) -> str:
    """The prefix of the entries of a profile in a `TokenStore` (`kartenn:<profile>`)."""
    return f"kartenn:{profile}"


def _issuer(profile: Profile) -> Issuer | str:
    """The issuer of a profile: given in full (`discovery_url`, `client_id`…), else its
    local identifier, resolved by `GET /v1/auth/native-clients`."""
    extra = profile.extra
    if "discovery_url" in extra and "client_id" in extra:
        return Issuer.parse({**extra, "id": profile.issuer})
    if profile.issuer is None:
        msg = f"the profile {profile.name!r} names no issuer"
        raise ClientError("client.invalid-configuration", msg)
    return profile.issuer


def auth_of_profile(profile: Profile, store: TokenStore) -> Auth:
    """The authentication a profile names (design § Authentification)."""
    if profile.auth == "none":
        return Anonymous()
    if profile.auth == "token":
        token = os.environ.get("KARTENN_TOKEN")
        if not token:
            msg = f"the profile {profile.name!r} expects a token in KARTENN_TOKEN"
            raise ClientError("client.login-required", msg)
        return BearerToken(token)
    if profile.auth == "basic":
        if profile.account is None:
            msg = f"the profile {profile.name!r} names no account"
            raise ClientError("client.invalid-configuration", msg)
        kept = store.get(f"{store_key(profile.name)}:secret")
        path = os.environ.get("KARTENN_SECRET_FILE")
        if kept is not None:
            secret = Secret(kept)
        elif path:
            secret = Secret.from_file(path)
        else:
            msg = (
                f"no secret for the account {profile.account!r}: keep it in the keyring, "
                "or name its file in KARTENN_SECRET_FILE"
            )
            raise ClientError("client.secret-required", msg)
        return BasicAuth(profile.account, secret)
    if profile.auth == "oidc":
        return OidcNative(_issuer(profile), store, key=store_key(profile.name))
    if profile.auth == "breakglass":
        msg = "a break-glass session is opened by kartenn login --breakglass, never by a profile"
        raise ClientError("client.login-required", msg)
    msg = f"unknown authentication {profile.auth!r} in the profile {profile.name!r}"
    raise ClientError("client.invalid-configuration", msg)


class Client:
    """The synchronous client. `kn.<resource>` (`kn.policy_rules`) or
    `kn.resource("policy-rules")` reaches a resource of the catalog."""

    def __init__(  # noqa: PLR0913 - the settings of a connection
        self,
        url: str,
        *,
        auth: Auth | None = None,
        ca_bundle: str | _os.PathLike[str] | None = None,
        http2: bool | None = None,
        token_store: TokenStore | None = None,
        transport: Transport | None = None,
        cache: CatalogCache | None = None,
        retry: RetryPolicy | None = None,
        profile: str | None = None,
    ) -> None:
        self.url = url.rstrip("/")
        self._auth: Auth = auth or Anonymous()
        self._auth.check_server(self.url)
        if transport is None:
            from kartenn.transport.httpx2 import Httpx2Transport  # noqa: PLC0415 - lazy

            transport = Httpx2Transport(self.url, ca_bundle=ca_bundle, http2=http2)
        self._sender = Sender(transport, user_agent=_agent(), policy=retry)
        self._context = Context.of(self.url, cache, profile=profile or "")
        self.token_store: TokenStore = token_store or MemoryStore()
        self._owner = True

    @classmethod
    def from_profile(
        cls,
        name: str | None = None,
        *,
        config: Path | None = None,
        token_store: TokenStore | None = None,
        transport: Transport | None = None,
        cache: CatalogCache | None = None,
    ) -> Client:
        """A client of a profile of `config.toml`, its secrets taken from `token_store`."""
        profile = read_profile(name, path=config)
        store = token_store or MemoryStore()
        return cls(
            profile.url,
            auth=auth_of_profile(profile, store),
            ca_bundle=profile.ca_bundle,
            http2=profile.http2,
            token_store=store,
            transport=transport,
            cache=cache,
            profile=profile.name,
        )

    # Sending ---------------------------------------------------------------------

    def send(self, request: Request) -> Response:
        """A request authenticated, retried: once again after a 401 when the
        authentication renews itself."""
        response = self._sender.send(run(self._auth.authorize(request), self._sender.send))
        if response.status == _UNAUTHORIZED and run(
            self._auth.unauthorized(response), self._sender.send
        ):
            authorized = run(self._auth.authorize(request), self._sender.send)
            response = self._sender.send(authorized)
        return response

    def run(self, plan: Plan[T]) -> T:
        return run(plan, self.send)

    def run_unauthenticated(self, plan: Plan[T]) -> T:
        """A plan whose requests carry no authentication (the issuer, a login)."""
        return run(plan, self._sender.send)

    def sleep(self, seconds: float) -> None:
        self._sender.transport.sleep(seconds)

    # The catalog -----------------------------------------------------------------

    @property
    def catalog(self) -> Catalog:
        return self.run(self._context.catalog())

    def refresh(self) -> Catalog:
        """Read the capabilities again now, and the contract if the revision changed."""
        return self.run(self._context.catalog(refresh=True))

    @property
    def modules(self) -> tuple[str, ...]:
        return self.catalog.module_names

    def has(self, module: str) -> bool:
        return self.catalog.has(module)

    def resource(self, name: str) -> Resource:
        self.catalog.resource(name)
        return Resource(self._context, self.run, name)

    def labels(self, lang: str | None = None) -> Dictionary:
        """The dictionary of labels of `lang` (spec 018), the default language of the
        server when `None`: kept in memory until an event `labels`, `ready` or `reset`
        of `events()`, on disk between runs (revalidated under `If-None-Match`)."""
        return self.run(self._context.labels(lang))

    def __getattr__(self, attribute: str) -> Resource:
        if attribute.startswith("_"):
            raise AttributeError(attribute)
        return self.resource(_attribute_to_resource(attribute))

    # Calls -----------------------------------------------------------------------

    def at(
        self, valid_at: datetime | str | None = None, as_of: datetime | str | None = None
    ) -> Client:
        """A view of the client at these dates: every reading carries them."""
        view = copy.copy(self)
        view._context = self._context.at(valid_at, as_of)  # noqa: SLF001 - its own class
        view._owner = False  # noqa: SLF001
        return view

    def call(
        self,
        operation_id: str,
        *,
        body: object = None,
        etag: str | None = None,
        idempotency_key: str | None = None,
        headers: Mapping[str, str] | None = None,
        **params: object,
    ) -> Any:  # noqa: ANN401 - what the operation answers
        """Any operation of the catalog by its `operationId` (the twin of `kartenn api`)."""
        return self.run(
            self._context.call(
                operation_id,
                body=body,
                etag=etag,
                key=idempotency_key,
                headers=headers,
                params=params,
            )
        )

    # Changesets and imports ------------------------------------------------------

    def changeset(
        self,
        *,
        motive: str,
        valid_from: datetime | str | None = None,
        valid_to: datetime | str | None = None,
        on_behalf_of: str | None = None,
        idempotency_key: str | None = None,
    ) -> changesets.ChangesetBuilder:
        """A changeset to compose, then simulate or submit."""
        plans = changesets.builder(
            self._context,
            motive=motive,
            valid_from=valid_from,
            valid_to=valid_to,
            on_behalf_of=on_behalf_of,
            idempotency_key=idempotency_key,
        )
        return changesets.ChangesetBuilder(plans, self.run)

    def changeset_of(self, ref: object) -> changesets.ChangesetHandle:
        """An existing changeset, by its identifier or as read."""
        return changesets.ChangesetHandle(changesets.existing(self._context, ref), self.run)

    def import_session(
        self,
        source: str,
        *,
        digest: str,
        chunk_elements: int | None = None,
        state_dir: Path | None = None,
        on_progress: Callable[[imports.ImportProgress], None] | None = None,
    ) -> imports.ImportSession:
        """An import of `source`, its digest identifying the content (`file_digest`)."""
        plans = imports.plans(
            self._context,
            source=source,
            digest=digest,
            chunk_elements=chunk_elements,
            state_dir=state_dir,
            on_progress=on_progress,
        )
        return imports.ImportSession(plans, self.run, self._sender.transport.sleep)

    # Events ----------------------------------------------------------------------

    def events(  # noqa: PLR0913 - the filters of the 022
        self,
        *,
        types: Iterable[str] | None = None,
        domains: Iterable[str] | None = None,
        objects: Iterable[str] | None = None,
        kinds: Iterable[str] | None = None,
        last_event_id: str | None = None,
        reconnect: bool = True,
    ) -> Iterator[events.Event]:
        """The events of `GET /v1/events` (022), resumed after each cut."""
        events.require_events(self.catalog)
        request = events.event_request(types=types, domains=domains, objects=objects, kinds=kinds)
        transport = self._sender.transport

        def authorize(request: Request) -> Request:
            return run(self._auth.authorize(self._sender.prepare(request)), self._sender.send)

        def renew(response: Response) -> bool:
            return run(self._auth.unauthorized(response), self._sender.send)

        followed = events.follow(
            request,
            EventTracker(last_event_id),
            open_stream=transport.sse,
            authorize=authorize,
            renew=renew,
            sleep=transport.sleep,
            reconnect=reconnect,
        )
        return self._observed(followed)

    def _observed(self, followed: Iterator[events.Event]) -> Iterator[events.Event]:
        """The events, the dictionaries of labels made stale by those that say so."""
        for event in followed:
            if invalidating(event.kind):
                self._context.forget_labels()
            yield event

    # Life ------------------------------------------------------------------------

    def close(self) -> None:
        if self._owner:
            self._sender.transport.close()

    def __enter__(self) -> Self:
        return self

    def __exit__(
        self,
        kind: type[BaseException] | None,
        error: BaseException | None,
        trace: TracebackType | None,
    ) -> None:
        self.close()


class AsyncClient:
    """The asynchronous client: the same calls, awaited. `await kn.load()` (or
    `async with`) reads the catalog; resources are reached without it."""

    def __init__(  # noqa: PLR0913 - the settings of a connection
        self,
        url: str,
        *,
        auth: Auth | None = None,
        ca_bundle: str | _os.PathLike[str] | None = None,
        http2: bool | None = None,
        token_store: TokenStore | None = None,
        transport: AsyncTransport | None = None,
        cache: CatalogCache | None = None,
        retry: RetryPolicy | None = None,
        profile: str | None = None,
    ) -> None:
        self.url = url.rstrip("/")
        self._auth: Auth = auth or Anonymous()
        self._auth.check_server(self.url)
        if transport is None:
            from kartenn.transport.httpx2 import AsyncHttpx2Transport  # noqa: PLC0415 - lazy

            transport = AsyncHttpx2Transport(self.url, ca_bundle=ca_bundle, http2=http2)
        self._sender = AsyncSender(transport, user_agent=_agent(), policy=retry)
        self._context = Context.of(self.url, cache, profile=profile or "")
        self.token_store: TokenStore = token_store or MemoryStore()
        self._owner = True

    async def send(self, request: Request) -> Response:
        authorized = await arun(self._auth.authorize(request), self._sender.send)
        response = await self._sender.send(authorized)
        if response.status == _UNAUTHORIZED and await arun(
            self._auth.unauthorized(response), self._sender.send
        ):
            authorized = await arun(self._auth.authorize(request), self._sender.send)
            response = await self._sender.send(authorized)
        return response

    async def run(self, plan: Plan[T]) -> T:
        return await arun(plan, self.send)

    async def run_unauthenticated(self, plan: Plan[T]) -> T:
        return await arun(plan, self._sender.send)

    async def sleep(self, seconds: float) -> None:
        await self._sender.transport.sleep(seconds)

    async def load(self, *, refresh: bool = False) -> Catalog:
        return await self.run(self._context.catalog(refresh=refresh))

    async def modules(self) -> tuple[str, ...]:
        return (await self.load()).module_names

    async def has(self, module: str) -> bool:
        return (await self.load()).has(module)

    def resource(self, name: str) -> AsyncResource:
        return AsyncResource(self._context, self.run, name)

    async def labels(self, lang: str | None = None) -> Dictionary:
        """The dictionary of labels of `lang` (spec 018), as `Client.labels`."""
        return await self.run(self._context.labels(lang))

    def __getattr__(self, attribute: str) -> AsyncResource:
        if attribute.startswith("_"):
            raise AttributeError(attribute)
        return self.resource(_attribute_to_resource(attribute))

    def at(
        self, valid_at: datetime | str | None = None, as_of: datetime | str | None = None
    ) -> AsyncClient:
        view = copy.copy(self)
        view._context = self._context.at(valid_at, as_of)  # noqa: SLF001 - its own class
        view._owner = False  # noqa: SLF001
        return view

    async def call(
        self,
        operation_id: str,
        *,
        body: object = None,
        etag: str | None = None,
        idempotency_key: str | None = None,
        headers: Mapping[str, str] | None = None,
        **params: object,
    ) -> Any:  # noqa: ANN401 - what the operation answers
        plan = self._context.call(
            operation_id, body=body, etag=etag, key=idempotency_key, headers=headers, params=params
        )
        return await self.run(plan)

    def changeset(
        self,
        *,
        motive: str,
        valid_from: datetime | str | None = None,
        valid_to: datetime | str | None = None,
        on_behalf_of: str | None = None,
        idempotency_key: str | None = None,
    ) -> changesets.AsyncChangesetBuilder:
        plans = changesets.builder(
            self._context,
            motive=motive,
            valid_from=valid_from,
            valid_to=valid_to,
            on_behalf_of=on_behalf_of,
            idempotency_key=idempotency_key,
        )
        return changesets.AsyncChangesetBuilder(plans, self.run)

    def changeset_of(self, ref: object) -> changesets.AsyncChangesetHandle:
        return changesets.AsyncChangesetHandle(changesets.existing(self._context, ref), self.run)

    def import_session(
        self,
        source: str,
        *,
        digest: str,
        chunk_elements: int | None = None,
        state_dir: Path | None = None,
        on_progress: Callable[[imports.ImportProgress], None] | None = None,
    ) -> imports.AsyncImportSession:
        plans = imports.plans(
            self._context,
            source=source,
            digest=digest,
            chunk_elements=chunk_elements,
            state_dir=state_dir,
            on_progress=on_progress,
        )
        return imports.AsyncImportSession(plans, self.run, self._sender.transport.sleep)

    async def aevents(  # noqa: PLR0913 - the filters of the 022
        self,
        *,
        types: Iterable[str] | None = None,
        domains: Iterable[str] | None = None,
        objects: Iterable[str] | None = None,
        kinds: Iterable[str] | None = None,
        last_event_id: str | None = None,
        reconnect: bool = True,
    ) -> AsyncIterator[events.Event]:
        """The events of `GET /v1/events` (022), resumed after each cut."""
        events.require_events(await self.load())
        request = events.event_request(types=types, domains=domains, objects=objects, kinds=kinds)
        transport = self._sender.transport

        async def authorize(request: Request) -> Request:
            prepared = self._sender.prepare(request)
            return await arun(self._auth.authorize(prepared), self._sender.send)

        async def renew(response: Response) -> bool:
            return await arun(self._auth.unauthorized(response), self._sender.send)

        async for event in events.afollow(
            request,
            EventTracker(last_event_id),
            open_stream=transport.sse,
            authorize=authorize,
            renew=renew,
            sleep=transport.sleep,
            reconnect=reconnect,
        ):
            if invalidating(event.kind):
                self._context.forget_labels()
            yield event

    async def aclose(self) -> None:
        if self._owner:
            await self._sender.transport.aclose()

    async def __aenter__(self) -> Self:
        await self.load()
        return self

    async def __aexit__(
        self,
        kind: type[BaseException] | None,
        error: BaseException | None,
        trace: TracebackType | None,
    ) -> None:
        await self.aclose()
