"""REQ-023-44: the library depends on httpx2 and platformdirs only, and imports nothing
of the CLI."""

import subprocess
import sys
from importlib.metadata import requires

from packaging.requirements import Requirement

import kartenn

CLI_MODULES = ("click", "rich_click", "rich", "keyring", "yaml", "jmespath")

# Imports every module of the library in a fresh interpreter, then names the modules of
# the CLI dependencies that got loaded. The workspace environment holds them all, so
# their absence from sys.modules is what is checked, not their absence from the disk.
PROBE = """
import importlib, pkgutil, sys
import kartenn
for module in pkgutil.walk_packages(kartenn.__path__, "kartenn."):
    importlib.import_module(module.name)
print(" ".join(sorted({name.split(".")[0] for name in sys.modules} & set(sys.argv[1:]))))
"""


def test_runtime_dependencies_are_httpx2_and_platformdirs() -> None:
    declared = [Requirement(line) for line in requires("kartenn") or []]
    runtime = {req.name.lower() for req in declared if req.marker is None}
    assert runtime == {"httpx2", "platformdirs"}


def test_library_imports_no_cli_dependency() -> None:
    result = subprocess.run(
        [sys.executable, "-c", PROBE, *CLI_MODULES],
        capture_output=True,
        text=True,
        check=True,
    )
    assert result.stdout.split() == []


def test_version_is_the_distribution_one() -> None:
    assert kartenn.__version__ == "0.1.0.dev0"
