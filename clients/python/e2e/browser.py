"""The browser of the end-to-end sessions (spec 023, task 26): named by `BROWSER`, it
signs in at the development issuer as `KARTENN_E2E_USER` (`alice` by default) and
follows the redirection to the loopback listener of `kartenn login` (RFC 8252 §7.3).

The CLI opens the browser before it serves its listener: the script answers at once and
does the work in a detached child, as a real browser would.
"""

from __future__ import annotations

import os
import ssl
import subprocess
import sys
import urllib.parse
import urllib.request
from pathlib import Path

CA = Path(__file__).resolve().parents[3] / "web" / "e2e" / ".state" / "ca.pem"


def sign_in(url: str) -> None:
    """POST the authorization request with the chosen user, then follow the redirection
    to the loopback (`urllib` turns a 303 into a GET)."""
    split = urllib.parse.urlsplit(url)
    form = dict(urllib.parse.parse_qsl(split.query))
    form["user"] = os.environ.get("KARTENN_E2E_USER", "alice")
    action = urllib.parse.urlunsplit((split.scheme, split.netloc, split.path, "", ""))
    context = ssl.create_default_context(cafile=str(CA))
    data = urllib.parse.urlencode(form).encode()
    request = urllib.request.Request(action, data=data, method="POST")  # noqa: S310 - https
    with urllib.request.urlopen(request, context=context, timeout=10) as answer:  # noqa: S310
        answer.read()


def main() -> None:
    if sys.argv[1] == "--now":
        sign_in(sys.argv[2])
        return
    subprocess.Popen(
        [sys.executable, __file__, "--now", sys.argv[1]],
        start_new_session=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


if __name__ == "__main__":
    main()
