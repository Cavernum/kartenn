"""Root of the `kartenn` command.

The command tree is derived from the contract of the server (design § CLI); this
skeleton only answers `--version` and `--help`.
"""

import click


@click.group(context_settings={"help_option_names": ["-h", "--help"]})
@click.version_option(package_name="kartenn-cli", prog_name="kartenn")
def main() -> None:
    """Command line of kartenn."""
