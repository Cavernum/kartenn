import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

from fakes import CONTRACT, capabilities
from kartenn.__main__ import main
from kartenn.catalog import derive
from kartenn.stubs import render

ROOT = Path(__file__).resolve().parents[4]
REFERENCE = ROOT / "specs/006-api-rest/openapi-v1.json"

GOOD = """
from typing import TYPE_CHECKING, cast

import kartenn

if TYPE_CHECKING:
    from kartenn_server import Client


def main() -> None:
    kn = cast("Client", kartenn.Client("https://kartenn.test"))
    kn.vlans.create(motive="m", name="admin")
    kn.prefixes.list(domain="interne", value=["192.0.2.0/24"])
    kn.resource("vlans").get("x")
    print("ran")
"""
BAD = """
from typing import TYPE_CHECKING, cast

import kartenn

if TYPE_CHECKING:
    from kartenn_server import Client


def main() -> None:
    kn = cast("Client", kartenn.Client("https://kartenn.test"))
    kn.vlans.create(motive="m", nmae="admin")
    kn.prefixess.list()
"""


def test_render_of_the_reference_contract() -> None:
    text = render(derive("https://kartenn.test", capabilities(), CONTRACT))
    assert '"""Types of the kartenn server https://kartenn.test (version 0.1.0' in text
    assert 'Prefix = _t.TypedDict("Prefix", {' in text
    assert 'Nature: _t.TypeAlias = _t.Literal["container", "network"]' in text
    assert "class PrefixesResource(_t.Protocol):" in text
    assert "class PolicyRulesResource(_t.Protocol):" in text
    assert "    def prefixes(self) -> PrefixesResource: ..." in text
    assert "def properties_explain(self" in text
    compile(text, "stub.pyi", "exec")


def test_module_unknown_to_the_library() -> None:
    contract = {
        "paths": {
            "/v1/demo-widgets": {
                "get": {"operationId": "list_widgets", "tags": ["demo"]},
                "post": {
                    "operationId": "create_widget",
                    "tags": ["demo"],
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/NewWidget"}
                            }
                        }
                    },
                },
            },
            "/v1/demo-widgets/{id}": {
                "patch": {
                    "operationId": "modify_widget",
                    "tags": ["demo"],
                    "requestBody": {
                        "content": {"application/json": {"schema": {"type": "string"}}}
                    },
                }
            },
        },
        "components": {
            "schemas": {
                "NewWidget": {
                    "properties": {
                        "name": {"type": ["string", "null"]},
                        "class": {"type": "integer"},
                        "motive": {"type": "string"},
                        "tags": {"type": "array", "items": {"type": "string"}},
                        "kind": {
                            "oneOf": [{"$ref": "#/components/schemas/Kind"}, {"type": "null"}]
                        },
                        "any": {},
                    },
                    "required": ["name"],
                },
                "Kind": {"enum": ["a", "b"]},
                "Both": {
                    "allOf": [
                        {"$ref": "#/components/schemas/NewWidget"},
                        {"properties": {"extra": {"type": "boolean"}}},
                    ]
                },
                "One": {"allOf": [{"$ref": "#/components/schemas/Kind"}]},
                "Client": {"type": "object"},
                "2fa": {"type": "number", "enum": [1, 2]},
            }
        },
    }
    caps = capabilities(modules=[{"name": "demo", "version": "1"}])
    text = render(derive("https://demo.test", caps, contract))
    assert "class DemoWidgetsResource(_t.Protocol):" in text
    assert '"class": _t.NotRequired[int]' in text
    assert '"name": str | None' in text
    assert "_t.Unpack[_CreateWidgetMembers]" in text
    assert '"motive"' not in text.split("_CreateWidgetMembers = ")[1].split("\n")[0]
    assert '"extra": _t.NotRequired[bool]' in text
    assert "One: _t.TypeAlias = Kind" in text
    assert "Client_: _t.TypeAlias = dict[str, _t.Any]" in text
    assert "_2fa: _t.TypeAlias = float" in text
    assert "def update(self, ref: object, changes:" in text
    compile(text, "stub.pyi", "exec")


def test_command_line(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    assert (
        main(["stubs", "generate", "--contract", str(REFERENCE), "--out", str(tmp_path / "a")]) == 0
    )
    assert (tmp_path / "a" / "kartenn_server" / "__init__.pyi").exists()
    caps = tmp_path / "caps.json"
    caps.write_text(json.dumps(capabilities()))
    assert (
        main(
            [
                "stubs",
                "generate",
                "--contract",
                str(REFERENCE),
                "--capabilities",
                str(caps),
                "--out",
                str(tmp_path / "b"),
            ]
        )
        == 0
    )
    assert "version 0.1.0" in (tmp_path / "b" / "kartenn_server" / "__init__.pyi").read_text()
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / "nowhere"))
    monkeypatch.delenv("KARTENN_PROFILE", raising=False)
    assert main(["stubs", "generate", "--profile", "prod", "--out", str(tmp_path / "c")]) == 1
    assert "no configuration" in capsys.readouterr().err


@pytest.mark.timeout(180)
@pytest.mark.skipif(shutil.which("pyright") is None, reason="pyright comes with devenv")
def test_pyright_flags_unknown_members_and_never_imports_the_stubs(tmp_path: Path) -> None:
    assert (
        main(
            ["stubs", "generate", "--contract", str(REFERENCE), "--out", str(tmp_path / "typings")]
        )
        == 0
    )
    (tmp_path / "pyrightconfig.json").write_text(
        json.dumps(
            {
                "stubPath": "typings",
                "typeCheckingMode": "strict",
                "pythonVersion": "3.11",
                "reportMissingModuleSource": "none",
            }
        )
    )
    (tmp_path / "good.py").write_text(GOOD)
    (tmp_path / "bad.py").write_text(BAD)
    pyright = shutil.which("pyright") or "pyright"

    def check(name: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [pyright, "--pythonpath", sys.executable, name],
            cwd=tmp_path,
            capture_output=True,
            text=True,
            check=False,
        )

    good, bad = check("good.py"), check("bad.py")
    assert good.returncode == 0, good.stdout
    assert bad.returncode == 1
    assert 'No parameter named "nmae"' in bad.stdout
    assert "prefixess" in bad.stdout
    probe = (
        "import runpy, sys\n"
        "script = runpy.run_path('good.py', run_name='example')\n"
        "print('kartenn_server' in sys.modules)"
    )
    ran = subprocess.run(
        [sys.executable, "-c", probe], cwd=tmp_path, capture_output=True, text=True, check=True
    )
    assert ran.stdout.strip() == "False"
