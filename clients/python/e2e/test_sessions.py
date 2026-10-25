"""The sessions of the maquette (specs/023-client-python-cli/maquette.md) replayed against
a real kartenn-server, PostgreSQL and kartenn-dev-idp (spec 023, task 26).

The scenes run in order and build on each other, as in the maquette: the break-glass
account opens the instance to the issuer, Alice signs in (loopback, then device flow),
loads the data set by a changeset, reads, writes, imports, checks and requests. Outputs
are compared after `normalized` (UUIDs and dates), on the lines that carry the scene.

A scene the server cannot serve yet is marked: `xfail(strict=True)` when the client and
the contract disagree (it turns red the day it passes, to be unmarked), `skip` when the
server lacks the feature (spec 022).
"""

from __future__ import annotations

import json
import re
import subprocess
import time
import urllib.parse
import urllib.request
from typing import TYPE_CHECKING, Any

import pexpect

from conftest import LABO, PROD, STATE, Cli, Run, breakglass, normalized

if TYPE_CHECKING:
    from pathlib import Path

ISSUER = "https://127.0.0.1:9444"
MOTIVE = "sessions de bout en bout"
# What a scene leaves to the next ones.
FACTS: dict[str, Any] = {}


def items(run: Run) -> list[dict[str, Any]]:
    """The objects of a JSON list output (`-o json`)."""
    return json.loads(run.out)


def lines(run: Run) -> list[str]:
    return [line.rstrip() for line in normalized(run.out + run.err).splitlines()]


def ids_by(cli: Cli, *args: str, key: str = "key") -> dict[str, str]:
    """`{key: id}` of a listing."""
    return {
        str(item[key]): str(item["id"])
        for item in items(cli(*args, "-o", "json", "--fields", f"id,{key}", "--all"))
    }


# 1. First connection ------------------------------------------------------------------


def test_profiles_and_break_glass(cli: Cli) -> None:
    """The operator opens each instance to the issuer and binds the administrators, by
    the CLI under the break-glass account (the only secret outside the API)."""
    ca = str(STATE / "ca.pem")
    run = cli("config", "add-profile", "prod", "--url", PROD, "--issuer", "dev", "--ca-bundle", ca)
    assert "Profil « prod » créé" in run.out
    assert "choisi par défaut" in run.out
    cli("config", "add-profile", "labo", "--url", LABO, "--issuer", "dev", "--ca-bundle", ca)
    for name, url in (("secours", PROD), ("secours-labo", LABO)):
        cli("config", "add-profile", name, "--url", url, "--auth", "none", "--ca-bundle", ca)
    breakglass(cli, "secours", "breakglass-cli.txt")
    breakglass(cli, "secours-labo", "breakglass-cli-labo.txt")
    for profile in ("secours", "secours-labo"):
        me = cli("-p", profile, "me")
        assert re.search(r"^Identité +e2e +\(breakglass", me.out, re.MULTILINE), me.out
        cli("-p", profile, "issuer-attachments", "create", "--issuer", ISSUER, "--motive", MOTIVE)
        # The group `kartenn-admins` of kartenn, whose member is the group of the issuer
        # (spec 025): a binding serves a group of kartenn.
        group = [
            {"op": "set_group", "key": "kartenn-admins", "labels": {"fr": "Administrateurs"}},
            {
                "op": "add_member",
                "group": "kartenn-admins",
                "member": {"kind": "idp_group", "issuer": ISSUER, "group": "kartenn-admins"},
            },
        ]
        (cli.home / "groupe.yaml").write_text("\n".join(f"- {json.dumps(o)}" for o in group))
        cli("-p", profile, "changeset", "new", "--motive", MOTIVE)
        cli("-p", profile, "changeset", "add", "--from-file", "groupe.yaml")
        run = cli("-p", profile, "changeset", "apply")
        assert re.match(r"Changeset <uuid> appliqué\.", normalized(run.out)), run.both
        run = cli(
            "-p",
            profile,
            "bindings",
            "create",
            "--role",
            "administrator",
            "--set",
            "group=kartenn-admins",
            "--set",
            'scope={"kind": "global"}',
            "--motive",
            MOTIVE,
        )
        assert re.search(r"^Créé : /v1/bindings/<uuid>$", run.both, re.MULTILINE), run.both


def test_login_by_the_browser(cli: Cli) -> None:
    """Maquette § 1: `kartenn login`, the browser at the issuer (PKCE, loopback), then the
    discovery of the server."""
    child = cli.terminal("login")
    child.expect(pexpect.EOF, timeout=60)
    child.close()
    shown = normalized(str(child.before))
    assert child.exitstatus == 0, shown
    assert "Ouverture du navigateur pour vous connecter chez « dev »" in shown
    assert "Si rien ne s'ouvre : https://127.0.0.1:9444/authorize?" in shown
    assert "Connecté : Alice Exemple (https://127.0.0.1:9444)." in shown
    me = cli("me")
    assert lines(me) == [
        "Identité     Alice Exemple (https://127.0.0.1:9444)  (person, bearer)",
        "Groupes      kartenn-admins (kartenn-admins)",
        "Groupes IdP  kartenn-admins",
        "Rôles        administrator",
    ]


def test_login_by_the_device_flow(cli: Cli) -> None:
    """Maquette § 1, on a host without a browser: the code shown, approved at the issuer,
    one `slow_down` on the way (web/e2e/users.toml)."""
    process = subprocess.Popen(
        cli.argv("-p", "labo", "login", "--device"),
        env=cli.environ(),
        cwd=cli.home,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert process.stderr is not None
    shown: list[str] = []
    url = None
    while url is None:
        line = process.stderr.readline()
        assert line, "".join(shown)
        shown.append(line)
        found = re.search(r"(https://\S+/device\?\S*user_code=\S+)", line)
        url = found[1] if found else None
    assert "Vous autorisez un APPAREIL" in "".join(shown)
    code = urllib.parse.parse_qs(urllib.parse.urlsplit(url).query)["user_code"][0]
    import ssl  # noqa: PLC0415

    context = ssl.create_default_context(cafile=str(STATE / "ca.pem"))
    form = urllib.parse.urlencode({"user_code": code, "user": "alice"}).encode()
    request = urllib.request.Request(f"{ISSUER}/device", data=form, method="POST")  # noqa: S310
    with urllib.request.urlopen(request, context=context, timeout=10) as answer:  # noqa: S310
        assert "Device approved" in answer.read().decode()
    out, err = process.communicate(timeout=60)
    assert process.returncode == 0, out + err
    assert "Connecté : Alice Exemple (https://127.0.0.1:9444)." in out


def test_groups_of_kartenn(cli: Cli) -> None:
    """Spec 025 from the CLI: Alice, administrator by the group of her issuer, makes a
    group named by its key, gives it a group of the issuer as member and a binding, reads
    them, then closes it; the same writes go through a local changeset."""
    run = cli(
        "groups", "create", "--key", "reseau", "--set", 'labels={"fr": "Réseau"}',
        "--motive", MOTIVE,
    )  # fmt: skip
    assert re.search(r"^Créé : /v1/groups/<uuid>$", run.both, re.MULTILINE), run.both
    member = f"{ISSUER}/lecteurs"
    add = ("groups", "members", "add", "reseau", member, "--motive", MOTIVE)
    simulated = cli(*add, "--dry-run")
    assert "Simulation — rien n'est écrit" in simulated.out, simulated.both
    assert items(cli("groups", "members", "list", "reseau", "-o", "json")) == []
    run = cli(*add)
    created = r"^Créé : /v1/groups/<uuid>/members/<uuid>$"
    assert re.search(created, run.both, re.MULTILINE), run.both
    members = items(cli("groups", "members", "list", "reseau", "-o", "json"))
    assert [(m["kind"], m["issuer"], m["group"]) for m in members] == [
        ("idp_group", ISSUER, "lecteurs")
    ]
    cli(
        "bindings", "create", "--group", "reseau", "--role", "reader",
        "--set", 'scope={"kind": "global"}', "--motive", MOTIVE,
    )  # fmt: skip
    bound = items(cli("bindings", "list", "--group", "reseau", "-o", "json"))
    assert [(b["group"], b["role"]) for b in bound] == [("reseau", "reader")]
    shown = cli("groups", "get", "reseau", "-o", "json")
    assert json.loads(shown.out)["members"] == 1
    # A bearer token opens no session (spec 025, Q-025-1): Alice's card has no last one.
    alice = json.loads(cli("me", "-o", "json").out)["identity"]
    card = lines(cli("identities", "get", alice, "-o", "table"))
    assert any(re.fullmatch(r"last_session +—", line) for line in card), card
    cli("groups", "members", "remove", "reseau", member, "--motive", MOTIVE)
    assert items(cli("groups", "members", "list", "reseau", "-o", "json")) == []
    cli("groups", "close", "reseau", "--motive", MOTIVE)
    assert "reseau" not in ids_by(cli, "groups", "list")
    # The same group, composed in a local changeset (operations of the core).
    cli("changeset", "new", "--motive", MOTIVE)
    labels = 'labels={"fr": "Réseau"}'
    cli("groups", "create", "--key", "reseau-2", "--set", labels, "--into-changeset")
    cli("groups", "members", "add", "kartenn-admins", member, "--into-changeset")
    run = cli("changeset", "apply")
    assert re.match(r"Changeset <uuid> appliqué\.", normalized(run.out)), run.both
    assert "reseau-2" in ids_by(cli, "groups", "list")
    cli("groups", "members", "remove", "kartenn-admins", member, "--motive", MOTIVE)
    cli("groups", "close", "reseau-2", "--motive", MOTIVE)


# 2, 3. Help of the root and of a module discovered -------------------------------------


def test_help_of_the_root(cli: Cli) -> None:
    """Maquette § 2: the core at the root, the modules served in their section."""
    run = cli("--help")
    text = normalized(run.out)
    assert "Client de kartenn — profil « prod », https://127.0.0.1:9443 (" in text
    core = re.search(r"^Noyau:\n((?:  .*\n)+)", text, re.MULTILINE)
    assert core, text
    for name in ("addresses", "domains", "entities", "prefixes", "ranges", "sites"):
        assert name in core[1]
    modules = re.search(r"^Modules servis:\n((?:  .*\n)+)", text, re.MULTILINE)
    assert modules, text
    served = [line.split()[0] for line in modules[1].splitlines() if not line.startswith("    ")]
    assert served == ["flow", "machines", "policy", "vlan"]
    rights = re.search(r"^Droits et audit:\n  (.*)$", text, re.MULTILINE)
    assert rights, text
    assert "bindings" in rights[1]
    assert "issuer-attachments" in rights[1]


def test_help_of_a_module(cli: Cli) -> None:
    """Maquette § 3."""
    run = cli("policy", "--help")
    assert "Usage: kartenn policy [OPTIONS] COMMANDE [ARGS]..." in run.out
    assert "Politique de flux : règles, matrice, catalogue, vérification." in run.out
    assert re.search(r"^  check +Verdict d'une communication", run.out, re.MULTILINE)
    assert re.search(r"^  report +Rapport d'audit", run.out, re.MULTILINE)
    for name in ("rules", "services", "environments", "flow-types"):
        assert name in run.out


# Data set -------------------------------------------------------------------------------


def test_the_data_set_by_a_changeset(cli: Cli) -> None:
    """The data of the maquette, composed in a local changeset, simulated, applied
    (maquette § 8): the domain `interne` and its owner, prefixes and addresses with their
    environment, the policy (environments, a flow type, two services, one rule) and an
    external system."""
    FACTS["before"] = time.time()
    time.sleep(1.1)
    cli("entities", "create", "--slug", "exemple", "--motive", MOTIVE)
    entity = ids_by(cli, "entities", "list", key="slug")["exemple"]
    declarations = ids_by(cli, "property-declarations", "list")
    owner = declarations["core.owner"]
    environment = declarations["policy.environment"]
    system = declarations["core.external_system"]
    operations: list[dict[str, Any]] = [
        {"op": "policy.environment.create", "key": key} for key in ("prod", "recette", "dev")
    ]
    operations += [
        {
            "op": "policy.flow_type.create",
            "local": 10,
            "key": "admin",
            "labels": {"fr": "Administration"},
        },
        {
            "op": "policy.service.create",
            "key": "ssh",
            "labels": {"fr": "SSH"},
            "flow_type": {"local": 10},
            "entries": [{"transport": "tcp", "ports": 22}],
        },
        {
            "op": "policy.service.create",
            "key": "https",
            "labels": {"fr": "HTTPS"},
            "flow_type": {"local": 10},
            "entries": [{"transport": "tcp", "ports": 443}],
        },
        {
            "op": "policy.rule.create",
            "key": "prod-vers-dev",
            "labels": {"fr": "La production administre le développement"},
            "effect": "allow",
            "flow_types": [{"local": 10}],
            "source": {"policy.environment": ["prod"]},
            "destination": {"policy.environment": ["dev"]},
        },
        {"op": "add_enum_value", "property": {"id": system}, "key": "ticketing"},
        {"op": "create_domain", "local": 0, "slug": "interne"},
        {
            "op": "assign_property",
            "object": {"local": 0},
            "kind": "domain",
            "property": {"id": owner},
            "values": [entity],
        },
    ]
    prefixes = [
        ("198.51.100.0/24", "container", None),
        ("198.51.100.0/26", "network", "prod"),
        ("198.51.100.64/26", "network", "recette"),
        ("198.51.100.128/25", "network", "dev"),
        ("203.0.113.0/24", "container", None),
    ]
    for index, (value, nature, env) in enumerate(prefixes, start=1):
        operations.append(
            {
                "op": "create_prefix",
                "local": index,
                "domain": {"local": 0},
                "prefix": value,
                "nature": nature,
            }
        )
        if env:
            operations.append(
                {
                    "op": "assign_property",
                    "object": {"local": index},
                    "kind": "prefix",
                    "property": {"id": environment},
                    "values": [env],
                }
            )
    operations += [
        {"op": "create_address", "domain": {"local": 0}, "address": a, "usage": "host"}
        for a in ("198.51.100.10", "198.51.100.140")
    ]
    (cli.home / "jeu.yaml").write_text("\n".join(f"- {json.dumps(o)}" for o in operations))

    run = cli("changeset", "new", "--motive", "Jeu de la maquette")
    assert "(0 opération)" in run.out
    run = cli("changeset", "add", "--from-file", "jeu.yaml")
    assert f"Ajouté (opération {len(operations) - 1}) : create_address" in run.out
    run = cli("changeset", "simulate")
    assert lines(run)[:2] == [
        "Simulation — rien n'est écrit",
        f"  Opérations  {len(operations)}   Violations  0   Avertissements  7",
    ]
    assert "! [core.addr.documentation] /operations/" in run.out
    run = cli("changeset", "apply")
    assert re.match(r"Changeset <uuid> appliqué\.", normalized(run.out)), run.both
    assert not (cli.home / "changeset.yaml").exists()
    time.sleep(1.1)
    FACTS["loaded"] = time.time()


# 4. Lists at a date, in a table and in JSON --------------------------------------------


def test_lists_at_dates(cli: Cli) -> None:
    run = cli("prefixes", "list", "--domain", "interne", "-o", "table")
    assert lines(run) == [
        "DOMAIN   VALUE              NATURE     PUBLIC",
        "interne  198.51.100.0/24    container  false",
        "interne  198.51.100.0/26    network    false",
        "interne  198.51.100.64/26   network    false",
        "interne  198.51.100.128/25  network    false",
        "interne  203.0.113.0/24     container  false",
        "5 éléments",
    ]
    # Maquette § 4: `--within`, a filter the contract declares (task 021-28).
    run = cli(
        "prefixes", "list", "--domain", "interne", "--within", "198.51.100.0/24", "-o", "table"
    )
    assert lines(run) == [
        "DOMAIN   VALUE              NATURE     PUBLIC",
        "interne  198.51.100.0/24    container  false",
        "interne  198.51.100.0/26    network    false",
        "interne  198.51.100.64/26   network    false",
        "interne  198.51.100.128/25  network    false",
        "4 éléments",
    ]
    stamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(FACTS["before"]))
    run = cli("prefixes", "list", "--domain", "interne", "-o", "table", "--as-of", stamp)
    assert lines(run) == ["DOMAIN  VALUE  NATURE  PUBLIC", "0 élément — état au <date>"]
    run = cli("prefixes", "list", "--domain", "interne", "-o", "table", "--at", "2020-01-01")
    assert lines(run)[-1] == "0 élément — valide au <date>"
    run = cli(
        "prefixes",
        "list",
        "--domain",
        "interne",
        "--prop",
        "policy.environment=prod",
        "-o",
        "json",
        "--fields",
        "value,properties.policy.environment",
    )
    # The filter holds; the member does not: an item of a list carries no properties
    # (its `links.properties` does), unlike the maquette.
    assert items(run) == [{"value": "198.51.100.0/26"}]
    run = cli("prefixes", "list", "--domain", "interne", "-o", "ndjson", "--all")
    assert len(run.out.splitlines()) == 5


# 5. Completion -------------------------------------------------------------------------


def test_completion(cli: Cli) -> None:
    """A completion from the server (the domains) and a local one (the verbs of a
    module)."""

    def complete(words: str) -> list[str]:
        # The completion reads the words of the line, not the argv: the token file of
        # the sessions (no keyring here) is on the line, as a user would type it.
        words = words.replace("kartenn ", f"kartenn --token-file {cli.tokens} ", 1)
        env = cli.environ(
            _KARTENN_COMPLETE="bash_complete", COMP_WORDS=words, COMP_CWORD=str(len(words.split()))
        )
        done = subprocess.run(
            cli.argv(), env=env, cwd=cli.home, capture_output=True, text=True, check=True
        )
        return [line.split(",", 1)[1] for line in done.stdout.splitlines() if "," in line]

    assert "interne" in complete("kartenn prefixes list --domain ")
    verbs = complete("kartenn vlan ")
    for verb in ("create", "get", "list", "update", "groups"):
        assert verb in verbs


# 8. Simulate then apply a changeset; a direct write with --dry-run ----------------------


def test_a_write_simulated_then_composed(cli: Cli) -> None:
    """`--dry-run` on a write of the core (`create_<x>`) and of a module
    (`<module>.<operationId>`), then the same writes in a local changeset."""
    entity = ids_by(cli, "entities", "list", key="slug")["exemple"]
    cli(
        "vlan",
        "groups",
        "create",
        "--slug",
        "site-nord",
        "--set",
        f'anchor={{"entity": {{"id": "{entity}"}}}}',
        "--motive",
        MOTIVE,
    )
    run = cli("entities", "create", "--slug", "lot3", "--dry-run")
    assert lines(run)[:2] == [
        "Simulation — rien n'est écrit",
        "  Opérations  1   Violations  0   Avertissements  0",
    ]
    assert "lot3" not in ids_by(cli, "entities", "list", key="slug")
    run = cli(
        "vlan", "create", "--group", "site-nord", "--vid", "140", "--name", "lot3", "--dry-run"
    )
    assert "Opérations  1   Violations  0" in run.out

    cli("changeset", "new", "--motive", "Ouverture du lot 3", "--valid-from", "2027-01-01")
    run = cli("entities", "create", "--slug", "lot3", "--into-changeset")
    assert run.out.startswith("Ajouté (opération 0) : create_entity"), run.both
    run = cli(
        "vlan",
        "create",
        "--group",
        "site-nord",
        "--vid",
        "140",
        "--name",
        "lot3",
        "--into-changeset",
    )
    assert run.out.startswith("Ajouté (opération 1) : vlan.create_vlan"), run.both
    run = cli("changeset", "simulate")
    assert lines(run)[:2] == [
        "Simulation — rien n'est écrit (valide à partir du <date>)",
        "  Opérations  2   Violations  0   Avertissements  0",
    ]
    run = cli("changeset", "apply")
    assert re.match(r"Changeset <uuid> appliqué", normalized(run.out)), run.both
    assert ids_by(cli, "vlan", "list", "--at", "2027-01-02", key="vid") != {}
    assert "lot3" not in ids_by(cli, "entities", "list", key="slug")
    assert "lot3" in ids_by(cli, "entities", "list", "--at", "2027-01-02", key="slug")

    # Refused: the same VID twice in its group (code 8, nothing written).
    run = cli(
        "--at",
        "2027-01-02",
        "vlan",
        "create",
        "--group",
        "site-nord",
        "--vid",
        "140",
        "--name",
        "doublon",
        "--valid-from",
        "2027-01-02",
        "--dry-run",
        code=8,
    )
    assert "✗ [vlan." in run.both, run.both


def test_a_prefix_simulated_by_dry_run(cli: Cli) -> None:
    """`--dry-run` sends the write itself with `?simulate=true` (spec 024, REQ-024-17):
    the members of the REST write, not those of a changeset operation."""
    run = cli(
        "prefixes",
        "create",
        "--domain",
        "interne",
        "--value",
        "203.0.113.0/26",
        "--nature",
        "network",
        "--dry-run",
        code=None,
    )
    assert run.code == 0, run.both
    head = lines(run)[:2]
    assert head[0] == "Simulation — rien n'est écrit", run.both
    assert head[1].startswith("  Opérations  1   Violations  0"), run.both
    assert "203.0.113.0/26" not in ids_by(cli, "prefixes", "list", key="value")


# 10. Import a file, with a coded error and a resumption ---------------------------------


def _wait_import(cli: Cli, profile: str, import_id: str) -> dict[str, Any]:
    deadline = time.monotonic() + 60
    while True:
        record = json.loads(cli("-p", profile, "api", "GET", f"/v1/imports/{import_id}").out)
        if record["state"] in {"done", "paused", "cancelled"} or time.monotonic() > deadline:
            return record
        time.sleep(0.5)


def test_import_with_an_error_then_resumed(cli: Cli) -> None:
    """Maquette § 10: a batch refused (an address outside any network of its domain),
    code 6 and the rule; the network created, the import resumed to its end. Under the
    break-glass account: see the next scene for Alice."""
    (cli.home / "adresses.csv").write_text(
        "domain,address,usage,external_id\n"
        "interne,198.51.100.21,host,src-0001\n"
        "interne,203.0.113.70,host,src-0002\n"
    )
    run = cli(
        "-p",
        "secours",
        "import",
        "adresses.csv",
        "--resource",
        "addresses",
        "--id-column",
        "external_id",
        "--source",
        "cmdb-export",
        code=6,
    )
    text = normalized(run.both)
    assert "erreur [client.import-paused] L'import <uuid> est en pause : le lot 0" in text
    assert "✗ [core.address.outside_network] 203.0.113.70 is not inside a network" in text
    assert "« kartenn import --resume <uuid> »" in text
    import_id = re.search(r"--resume (\S+) »", run.err)
    assert import_id
    cli(
        "prefixes",
        "create",
        "--domain",
        "interne",
        "--value",
        "203.0.113.64/26",
        "--nature",
        "network",
        "--motive",
        "réseau du lot 3",
    )
    cli("-p", "secours", "import", "--resume", import_id[1])
    assert _wait_import(cli, "secours", import_id[1])["state"] == "done"
    addresses = ids_by(cli, "addresses", "list", "--domain", "interne", key="value")
    assert {"198.51.100.21", "203.0.113.70"} <= set(addresses)


def test_import_by_alice(cli: Cli) -> None:
    """Alice's rights come from a group of hers at the issuer: the batches, run in a
    background task without her token, are submitted with the groups she started it with."""
    (cli.home / "alice.csv").write_text(
        "domain,address,usage,external_id\ninterne,198.51.100.30,host,src-0101\n"
    )
    cli("import", "alice.csv", "--resource", "addresses", "--id-column", "external_id")
    addresses = ids_by(cli, "addresses", "list", "--domain", "interne", key="value")
    assert "198.51.100.30" in addresses


# 11. Generic command --------------------------------------------------------------------


def test_the_generic_command(cli: Cli) -> None:
    run = cli("api", "GET", "/v1/capabilities")
    names = {m["name"] for m in json.loads(run.out)["modules"]}
    assert {"core", "policy", "flow", "vlan", "machines"} <= names
    prefix = ids_by(cli, "prefixes", "list", "--domain", "interne", key="value")["203.0.113.0/24"]
    run = cli(
        "api", "GET", f"/v1/prefixes/{prefix}/next-free", "-f", "kind=prefix", "-f", "length=28"
    )
    assert json.loads(run.out)["value"] == "203.0.113.0/28"


# 6. Check a communication ----------------------------------------------------------------


def test_policy_check(cli: Cli) -> None:
    """Maquette § 6: allowed by the rule prod → dev; refused the other way, code 9 with
    --exit-status."""
    run = cli(
        "policy",
        "check",
        "prefix:interne/198.51.100.0/26",
        "prefix:interne/198.51.100.128/25",
        "--service",
        "ssh",
        "-o",
        "table",
    )
    assert lines(run)[0].startswith("Verdict  ✓ autorisé"), run.both
    assert "prod-vers-dev" in run.out
    run = cli(
        "policy",
        "check",
        "address:interne/198.51.100.140",
        "address:interne/198.51.100.10",
        "--service",
        "ssh",
        "--exit-status",
        "-o",
        "table",
        code=9,
    )
    assert lines(run)[0].startswith("Verdict  ✗ refusé"), run.both


# 7. Request a flow --------------------------------------------------------------------------


def test_flow_request(cli: Cli) -> None:
    """Maquette § 7: allowed by the policy, the flow is recorded at once."""
    run = cli(
        "flow",
        "request",
        "--source",
        "address:interne/198.51.100.10",
        "--destination",
        "prefix:interne/198.51.100.128/25",
        "--service",
        "https",
        "--entity",
        "exemple",
        "--ref",
        "ticketing:CHG-2048",
        "--justification",
        "Appel de l'API de facturation de recette",
        "--motive",
        "nouvelle recette",
    )
    assert lines(run)[0] == "Changeset <uuid> appliqué.", run.both
    assert "Créé : /v1/flows/<uuid>" in normalized(run.out)
    run = cli("flow", "list", "-o", "table")
    assert lines(run)[1].startswith("allowed  active"), run.both


def test_flow_request_with_an_unknown_system(cli: Cli) -> None:
    """A system that names no value of `core.external_system` is refused with its rule,
    never dropped."""
    run = cli(
        "flow",
        "request",
        "--source",
        "address:interne/198.51.100.10",
        "--destination",
        "prefix:interne/198.51.100.64/26",
        "--service",
        "https",
        "--entity",
        "exemple",
        "--ref",
        "tickets:CHG-2049",
        "--motive",
        "nouvelle recette",
        code=6,
    )
    assert "✗ [flow.reference.system]" in run.both, run.both
    assert '"tickets" names no value of core.external_system' in run.both, run.both


# 12. A server without the `policy` module ------------------------------------------------


def test_a_server_without_policy(cli: Cli) -> None:
    """Maquette § 12: Alice, signed in on `labo` by the device flow."""
    run = cli("-p", "labo", "--help")
    modules = re.search(r"^Modules servis:\n((?:  .*\n)+)", run.out, re.MULTILINE)
    assert modules, run.out
    assert [line.split()[0] for line in modules[1].splitlines()] == ["machines", "vlan"]
    run = cli(
        "-p",
        "labo",
        "policy",
        "check",
        "prefix:interne/198.51.100.0/24",
        "prefix:interne/198.51.100.0/24",
        "--service",
        "ssh",
        code=2,
    )
    shown = lines(run)
    assert shown[0] == (
        "erreur [client.module-unavailable] Le module « policy » n'est pas servi par ce serveur."
    ), run.err
    assert shown[1].startswith("  Profil labo — https://127.0.0.1:9445, version ")
    assert shown[2:] == [
        "  Modules servis : machines, vlan",
        (
            "  La commande existe quand le serveur sert le module ; "
            "aucune mise à jour du client n'est nécessaire."
        ),
    ]
    run = cli("-p", "labo", "addresses", "flows", "2b10", code=2)
    assert "« addresses flows » est servi par le module « flow », absent de ce serveur." in run.err


# 15. Events ---------------------------------------------------------------------------------


def test_watch_without_the_stream(cli: Cli) -> None:
    """Maquette § 15, last block: a server without the stream of the 022."""
    run = cli("-p", "labo", "watch", code=2)
    assert "erreur [client.operation-unavailable] Ce serveur ne sert pas le flux" in run.err


def test_watch(cli: Cli) -> None:
    """Maquette § 15: the stream of the 022, when the server serves it."""
    capabilities = json.loads(cli("capabilities", "-o", "json").out)
    assert capabilities.get("events"), "the server serves the stream of the 022"
    process = subprocess.Popen(
        cli.argv("watch", "--types", "core.prefix", "-o", "ndjson"),
        env=cli.environ(),
        cwd=cli.home,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        assert process.stdout is not None
        ready = json.loads(process.stdout.readline())
        assert ready["kind"] == "ready"
        cli(
            "prefixes",
            "create",
            "--domain",
            "interne",
            "--value",
            "203.0.113.128/26",
            "--nature",
            "network",
            "--motive",
            "suivi",
        )
        event = json.loads(process.stdout.readline())
        assert event["kind"] == "changes"
    finally:
        process.terminate()
        process.wait(timeout=10)


# Logout ---------------------------------------------------------------------------------------


def test_logout(cli: Cli, tmp_path: Path) -> None:
    del tmp_path
    run = cli("logout")
    assert run.out.strip() == "Déconnecté du profil prod."
    run = cli("--no-input", "me", code=4)
    assert "erreur [" in run.err
