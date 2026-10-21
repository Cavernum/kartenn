# The NixOS module in a virtual machine (spec 015, REQ-015-5 to 10): local PostgreSQL 18,
# migration in its oneshot unit, two instances, probes, first administrator by
# kartenn-breakglass and its login, an operator plugin published, the instance role
# refused an UPDATE of history (INV-11), secrets kept out of the unit, the command line
# and the environment, the hardening exposure, a failed migration that blocks the
# instances.
{
  testers,
  module,
  runCommand,
}: let
  # A plugin of the operator: the `notes` test plugin of the end-to-end journeys.
  plugin = runCommand "kartenn-plugin-notes" {} ''
    mkdir -p $out/share/kartenn/plugins/notes
    cp -r ${../../web/e2e/plugins/notes/1.0.0} $out/share/kartenn/plugins/notes/1.0.0
  '';
in
  testers.runNixOSTest {
    name = "kartenn";
    nodes.machine = {pkgs, ...}: {
      imports = [module];
      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;
      environment.systemPackages = [pkgs.curl pkgs.jq pkgs.oath-toolkit pkgs.openssl];

      # The secrets, made at boot outside the store: a key ring and a TLS certificate.
      systemd.services.vmtest-secrets = {
        wantedBy = ["multi-user.target"];
        before = ["kartenn-migrate.service" "kartenn@main.service" "kartenn@second.service"];
        requiredBy = ["kartenn-migrate.service"];
        path = [pkgs.openssl pkgs.coreutils];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = ''
          mkdir -p /run/keys
          echo "k1 $(head -c 32 /dev/urandom | base64 -w0)" > /run/keys/kartenn-auth
          openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 2 \
            -subj /CN=localhost -addext subjectAltName=IP:127.0.0.1 \
            -keyout /run/keys/tls.key -out /run/keys/tls.crt 2>/dev/null
          chmod 0400 /run/keys/*
        '';
      };

      services.kartenn = {
        enable = true;
        database.createLocally = true;
        instances = {
          main.listen = "127.0.0.1:8443";
          second.listen = "127.0.0.1:8444";
        };
        credentials = {
          auth-keys = "/run/keys/kartenn-auth";
          tls-cert = "/run/keys/tls.crt";
          tls-key = "/run/keys/tls.key";
        };
        settings = {
          auth.key_file = "%d/auth-keys";
          http.tls = {
            cert_file = "%d/tls-cert";
            key_file = "%d/tls-key";
          };
        };
        plugins = [plugin];
      };
    };

    testScript = ''
      import json, re

      def find(pattern: str, text: str, flags: int = 0) -> str:
          match = re.search(pattern, text, flags)
          assert match is not None, f"{pattern!r} not in {text[-2000:]!r}"
          return match.group(1)

      machine.wait_for_unit("kartenn@main.service")
      machine.wait_for_unit("kartenn@second.service")

      with subtest("the migration is a oneshot that ran before the instances"):
          machine.succeed("systemctl is-active kartenn-migrate.service")
          assert machine.succeed("systemctl show -p Type --value kartenn-migrate.service").strip() == "oneshot"

      with subtest("each instance is ready"):
          for port in (8443, 8444):
              machine.wait_until_succeeds(f"curl -sfk https://127.0.0.1:{port}/health/ready", timeout=60)

      with subtest("the first administrator: kartenn-breakglass, then its login"):
          proofs = machine.succeed("kartenn-breakglass admin 1")
          passphrase = find(r"^passphrase: (.+)$", proofs, re.M)
          secret = find(r"secret=([A-Z2-7]+)", proofs)
          code = machine.succeed(f"oathtool --totp -b {secret}").strip()
          body = json.dumps({"name": "admin", "passphrase": passphrase, "code": code})
          machine.succeed(f"echo '{body}' > /tmp/login.json")
          machine.succeed(
              "curl -sfk -c /tmp/jar -H 'Content-Type: application/json' "
              "--data @/tmp/login.json https://127.0.0.1:8443/v1/auth/breakglass"
          )
          me = json.loads(machine.succeed("curl -sfk -b /tmp/jar https://127.0.0.1:8443/v1/me"))
          assert "administrator" in json.dumps(me), me

      with subtest("the first-party and the operator plugins are published"):
          caps = json.loads(machine.succeed("curl -sfk -b /tmp/jar https://127.0.0.1:8443/v1/capabilities"))
          names = {p["name"] for p in caps["ui_plugins"]}
          assert {"vlan", "machines", "notes"} <= names, names

      with subtest("no database URL nor secret on the command line, in the environment or the unit"):
          pid = machine.succeed("systemctl show -p MainPID --value kartenn@main.service").strip()
          cmdline = machine.succeed(f"tr '\\0' ' ' < /proc/{pid}/cmdline")
          assert "postgres" not in cmdline, cmdline
          environ = machine.succeed(f"tr '\\0' '\\n' < /proc/{pid}/environ")
          assert "postgres" not in environ, environ
          key = machine.succeed("cut -d' ' -f2 /run/keys/kartenn-auth").strip()
          show = machine.succeed("systemctl show kartenn@main.service kartenn-migrate.service")
          assert key not in show and key not in environ and key not in cmdline
          for unit in ("kartenn@main", "kartenn-migrate"):
              config = find(r"(\S+\.toml)", machine.succeed(f"systemctl show -p ExecStart --value {unit}.service"))
              text = machine.succeed(f"cat {config}")
              assert key not in text and "postgres" not in text, text

      with subtest("the instance role may not rewrite history (INV-11)"):
          # The refusal is a missing privilege, not a missing column.
          refusal = machine.fail(
              "sudo -u postgres psql -v ON_ERROR_STOP=1 -d kartenn "
              "-c 'SET ROLE kartenn_server; UPDATE entity_history SET id = id WHERE false' 2>&1"
          )
          assert "permission denied for table entity_history" in refusal, refusal
          # The same role reads it: the refusal is the one of INV-11.
          machine.succeed(
              "sudo -u postgres psql -v ON_ERROR_STOP=1 -d kartenn "
              "-c 'SET ROLE kartenn_server; SELECT count(*) FROM entity_history'"
          )

      with subtest("the hardening of an instance scores 2.0 or less"):
          out = machine.succeed("systemd-analyze security kartenn@main.service --no-pager")
          score = float(find(r"Overall exposure level for \S+: ([0-9.]+)", out))
          print(f"systemd-analyze security kartenn@main: {score}")
          assert score <= 2.0, out[-2000:]

      with subtest("a failed migration keeps the instances from starting"):
          machine.succeed("sudo -u postgres psql -c 'ALTER ROLE kartenn NOLOGIN'")
          machine.fail("systemctl restart kartenn-migrate.service")
          machine.fail("systemctl restart kartenn@second.service")
          machine.succeed("sudo -u postgres psql -c 'ALTER ROLE kartenn LOGIN'")
          machine.succeed("systemctl restart kartenn-migrate.service")
          machine.succeed("systemctl restart kartenn@second.service")
          machine.wait_until_succeeds("curl -sfk https://127.0.0.1:8444/health/ready", timeout=60)
    '';
  }
