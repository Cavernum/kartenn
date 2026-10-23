"""Named commands of the modules (REQ-023-20; design § Commandes nommées).

A table of the CLI bound to `operationId`s of the contract: an entry whose operation is
missing is not offered.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import click

    from kartenn.catalog import Catalog

__all__ = ["named_commands"]


def named_commands(module: str, catalog: Catalog) -> dict[str, click.Command]:  # noqa: ARG001
    """The named commands of a module the server serves."""
    return {}
