"""The device authorization grant (RFC 8628 §3.1 to 3.5; REQ-023-7, `--device`).

The client asks the `device_authorization_endpoint` for a code, shows the address and the
code with the warning of §5.4, then asks the token endpoint at the pace of `interval`
(5 s by default, §3.2), longer by 5 s at each `slow_down` and for every request after it
(§3.5). It stops on `access_denied` and `expired_token`, or when `expires_in` is past.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, cast

from kartenn._core.problems import ClientError
from kartenn.auth.oidc_native import audience_fields, form_request, json_object

if TYPE_CHECKING:
    from collections.abc import Callable

    from kartenn._core.plans import Plan
    from kartenn.auth.oidc_native import OidcNative

__all__ = ["DeviceCode", "device_login"]

GRANT = "urn:ietf:params:oauth:grant-type:device_code"
DEFAULT_INTERVAL = 5
SLOW_DOWN = 5
# RFC 8628 §5.4: tell the user they are authorising a device.
WARNING = (
    "You are authorising this terminal to act in your name on kartenn. Go on only if you "
    "started this login yourself, here and now."
)


@dataclass(frozen=True)
class DeviceCode:
    """What to show: the address, the code, the complete address when the issuer gives
    one, how long it holds, and the warning of RFC 8628 §5.4."""

    user_code: str
    verification_uri: str
    verification_uri_complete: str | None
    expires_in: float
    warning: str = WARNING


def _request(auth: OidcNative) -> Plan[dict[str, Any]]:
    metadata = yield from auth.metadata()
    issuer = yield from auth.issuer()
    endpoint = metadata.get("device_authorization_endpoint")
    if not isinstance(endpoint, str):
        msg = "the issuer offers no device authorization (device_authorization_endpoint)"
        raise ClientError("client.device-unavailable", msg)
    fields = {
        "client_id": issuer.client_id,
        "scope": " ".join(issuer.scopes),
        **audience_fields(issuer),
    }
    answer = yield form_request(endpoint, fields)
    data = json_object(answer)
    if answer.status >= 400 or "device_code" not in data:  # noqa: PLR2004 - an HTTP error
        msg = f"the issuer refused the device authorization ({data.get('error', answer.status)})"
        raise ClientError("client.oidc-denied", msg)
    return data


def device_login(
    auth: OidcNative,
    run: Callable[[Plan[Any]], Any],
    *,
    display: Callable[[DeviceCode], object],
    sleep: Callable[[float], object],
    clock: Callable[[], float],
) -> None:
    """Log in with the device flow; `run` runs a plan without authentication."""
    data = cast("dict[str, Any]", run(_request(auth)))
    expires_in = float(data.get("expires_in", 600))
    complete = data.get("verification_uri_complete")
    display(
        DeviceCode(
            user_code=str(data.get("user_code", "")),
            verification_uri=str(data.get("verification_uri", "")),
            verification_uri_complete=complete if isinstance(complete, str) else None,
            expires_in=expires_in,
        )
    )
    interval = float(data.get("interval") or DEFAULT_INTERVAL)
    deadline = clock() + expires_in
    fields = {"grant_type": GRANT, "device_code": str(data["device_code"])}
    while True:
        sleep(interval)
        if clock() > deadline:
            msg = "the device code expired before the login was approved"
            raise ClientError("client.oidc-expired", msg)
        tokens = cast("dict[str, Any]", run(auth.exchange(fields)))
        error = tokens.get("error")
        if error is None:
            auth.keep(tokens)
            return
        if error == "authorization_pending":
            continue
        if error == "slow_down":
            interval += SLOW_DOWN
            continue
        if error == "access_denied":
            msg = "the login was denied at the issuer"
            raise ClientError("client.oidc-denied", msg)
        if error == "expired_token":
            msg = "the device code expired before the login was approved"
            raise ClientError("client.oidc-expired", msg)
        msg = f"the issuer refused the device login ({error})"
        raise ClientError("client.oidc-error", msg)
