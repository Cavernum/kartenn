"""Root of the `kartenn` command (skeleton)."""

from click.testing import CliRunner
from syrupy.assertion import SnapshotAssertion

from kartenn_cli.main import main


def test_version_names_the_command_and_the_distribution_version() -> None:
    result = CliRunner().invoke(main, ["--version"])
    assert result.exit_code == 0
    assert result.output == "kartenn, version 0.1.0.dev0\n"


def test_help(snapshot: SnapshotAssertion) -> None:
    result = CliRunner().invoke(main, ["--help"], prog_name="kartenn")
    assert result.exit_code == 0
    assert result.output == snapshot
