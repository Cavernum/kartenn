# The NixOS module `services.kartenn` (spec 015, REQ-015-5 to 9; ADR 0029/E6, E7, E8): a
# oneshot migration before N stateless instances (D9, ADR 0012), a monthly migration for
# the history partitions (004-25), a daily audit purge (REQ-012-16). Every secret comes
# from a file through `LoadCredential=`; the generated configuration holds paths only.
self: {
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.kartenn;
  toml = pkgs.formats.toml {};
  exe = lib.getExe' cfg.package "kartenn-server";

  # `%d/<name>` in `settings` names a credential: the path under the unit's credentials
  # directory, known in advance (systemd.exec(5), « Credentials »).
  credentialsOf = unit: "/run/credentials/${unit}.service";
  resolve = unit: text: builtins.replaceStrings ["%d/"] ["${credentialsOf unit}/"] text;
  configFile = unit: settings:
    pkgs.writeText "${unit}.toml" (resolve unit (builtins.readFile (toml.generate "${unit}.toml" settings)));

  # One read-only directory for the first-party plugins and the operator's ones, real
  # files (the registry refuses symbolic links).
  pluginsDir = pkgs.runCommand "kartenn-plugins" {} ''
    mkdir -p $out
    for source in ${lib.escapeShellArgs (map (p: "${p}/share/kartenn/plugins") ([cfg.package] ++ cfg.plugins))}; do
      [ -d "$source" ] && cp -rL --no-preserve=mode "$source"/. $out/
    done
  '';

  instanceSettings = name: instance:
    lib.recursiveUpdate cfg.settings {
      database.url_file = "%d/database-url";
      http.listen = instance.listen;
      web = {
        root = "${cfg.package}/share/kartenn/web/shell";
        plugins_dir = "${pluginsDir}";
      };
    };
  # The migration, the audit purge and kartenn-breakglass connect as the owner of the
  # schema; they take the telemetry and audit sections, never `http` (no `listen` there).
  ownerSettingsFor = lib.recursiveUpdate (lib.filterAttrs (n: _: builtins.elem n ["telemetry" "audit"]) cfg.settings) {
    database.url_file = "%d/database-url";
  };

  credentials = url: ["database-url:${url}"] ++ lib.mapAttrsToList (name: path: "${name}:${path}") cfg.credentials;

  # systemd.exec(5) hardening shared by every unit (E6): the target is a
  # `systemd-analyze security` exposure of 2.0 or less.
  hardening = {
    DynamicUser = true;
    ProtectSystem = "strict";
    ProtectHome = true;
    PrivateTmp = true;
    PrivateDevices = true;
    PrivateUsers = true;
    NoNewPrivileges = true;
    CapabilityBoundingSet = "";
    AmbientCapabilities = "";
    RestrictAddressFamilies = ["AF_INET" "AF_INET6" "AF_UNIX"];
    SystemCallFilter = ["@system-service" "~@privileged" "~@resources"];
    SystemCallArchitectures = "native";
    MemoryDenyWriteExecute = true;
    LockPersonality = true;
    ProtectKernelTunables = true;
    ProtectKernelModules = true;
    ProtectKernelLogs = true;
    ProtectControlGroups = true;
    ProtectClock = true;
    ProtectHostname = true;
    ProtectProc = "invisible";
    ProcSubset = "pid";
    RestrictNamespaces = true;
    RestrictRealtime = true;
    RestrictSUIDSGID = true;
    RemoveIPC = true;
    UMask = "0077";
    # The local PostgreSQL socket (connect(2) needs write access to it).
    ReadWritePaths = lib.optional cfg.database.createLocally "/run/postgresql";
  };

  # The socket directory as `host` (sqlx refuses an empty host before `@`), no secret.
  localUrl = user: pkgs.writeText "kartenn-${user}-url" "postgresql:///${cfg.database.name}?host=/run/postgresql&user=${user}";
  databaseUrlFile =
    if cfg.databaseUrlFile != null
    then cfg.databaseUrlFile
    else localUrl cfg.database.instanceUser;
  migrationDatabaseUrlFile =
    if cfg.migrationDatabaseUrlFile != null
    then cfg.migrationDatabaseUrlFile
    else localUrl cfg.database.ownerUser;

  port = listen: lib.toInt (lib.last (lib.splitString ":" listen));
  dbUnits = lib.optionals cfg.database.createLocally ["postgresql.service" "kartenn-db-setup.service"];

  # `breakglass create` as the owner, with the key ring of the instances (005/K17): the
  # first administrator of a new instance.
  breakglass = pkgs.writeShellScriptBin "kartenn-breakglass" ''
    set -euo pipefail
    [ $# -eq 2 ] || { echo "usage: kartenn-breakglass <name> <hours>" >&2; exit 2; }
    exec ${config.systemd.package}/bin/systemd-run --quiet --pipe --wait --collect \
      --unit=kartenn-breakglass \
      -p DynamicUser=yes -p User=${cfg.database.ownerUser} -p PrivateTmp=yes \
      ${lib.concatMapStringsSep " " (c: "-p LoadCredential=${c}") (credentials migrationDatabaseUrlFile)} \
      ${lib.optionalString cfg.database.createLocally "-p ReadWritePaths=/run/postgresql"} \
      ${exe} breakglass create --config ${configFile "kartenn-breakglass" ownerSettingsFor} \
        "$1" "$2" ${credentialsOf "kartenn-breakglass"}/${cfg.authKeysCredential}
  '';
in {
  options.services.kartenn = {
    enable = lib.mkEnableOption "kartenn, the declarative network reference";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.kartenn;
      defaultText = lib.literalExpression "kartenn.packages.\${system}.kartenn";
      description = "The kartenn package: server, web shell and first-party plugins.";
    };

    settings = lib.mkOption {
      inherit (toml) type;
      default = {};
      description = ''
        The configuration of kartenn-server (TOML, `crates/kartenn-server/src/config.rs`),
        merged with what the module sets (`database.url_file`, `http.listen`, `web`).
        `%d/<name>` names the credential `<name>` of `credentials`. Never a secret here:
        this file is in the Nix store.
      '';
    };

    instances = lib.mkOption {
      type = lib.types.attrsOf (lib.types.submodule {
        options.listen = lib.mkOption {
          type = lib.types.str;
          example = "0.0.0.0:8443";
          description = "`[http] listen` of the instance.";
        };
      });
      default = {main.listen = "127.0.0.1:8443";};
      description = "The instances (`kartenn@<name>.service`), stateless and alike (ADR 0012).";
    };

    databaseUrlFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "File holding the URL of the instance role (member of `kartenn_app`); local socket by default with `database.createLocally`.";
    };

    migrationDatabaseUrlFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "File holding the URL of the owner of the schema, for the migration, the audit purge and `kartenn-breakglass`.";
    };

    credentials = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = {};
      example = {
        auth-keys = "/run/secrets/kartenn-auth.keys";
        tls-key = "/run/secrets/kartenn-tls.key";
      };
      description = "Other secrets, passed to every unit by `LoadCredential=`; `settings` names them `%d/<name>`.";
    };

    authKeysCredential = lib.mkOption {
      type = lib.types.str;
      default = "auth-keys";
      description = "The credential holding the key ring (`[auth] key_file`), for `kartenn-breakglass`.";
    };

    plugins = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [];
      description = ''
        The operator's plugins, each under `share/kartenn/plugins/<name>/<version>/`, joined
        to the first-party ones. Installing a plugin entrusts it with the session of every
        user (W4, docs/exploitation/plugins.md).
      '';
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Open the ports of the instances.";
    };

    database = {
      createLocally = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "A local PostgreSQL 18, the owner and instance roles, peer authentication over the socket (E7).";
      };
      name = lib.mkOption {
        type = lib.types.str;
        default = "kartenn";
        description = "The database.";
      };
      ownerUser = lib.mkOption {
        type = lib.types.str;
        default = "kartenn";
        description = "Owner of the schema: system user of the migration, peer role with `createLocally`.";
      };
      instanceUser = lib.mkOption {
        type = lib.types.str;
        default = "kartenn_server";
        description = "System user of the instances, peer role member of `kartenn_app` with `createLocally`.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [breakglass];

    networking.firewall.allowedTCPPorts =
      lib.mkIf cfg.openFirewall (lib.mapAttrsToList (_: i: port i.listen) cfg.instances);

    services.postgresql = lib.mkIf cfg.database.createLocally {
      enable = true;
      # Above the module's own default (D15: PostgreSQL 18), below the operator's choice.
      package = lib.mkOverride 900 pkgs.postgresql_18;
      ensureDatabases = [cfg.database.name];
      ensureUsers = [
        {
          name = cfg.database.ownerUser;
          ensureDBOwnership = cfg.database.ownerUser == cfg.database.name;
        }
        {name = cfg.database.instanceUser;}
      ];
    };

    systemd.services =
      {
        kartenn-migrate = {
          description = "kartenn: database migration";
          wantedBy = ["multi-user.target"];
          after = ["network.target"] ++ dbUnits;
          requires = dbUnits;
          serviceConfig =
            hardening
            // {
              Type = "oneshot";
              RemainAfterExit = true;
              User = cfg.database.ownerUser;
              LoadCredential = credentials migrationDatabaseUrlFile;
              ExecStart = "${exe} migrate --config ${configFile "kartenn-migrate" ownerSettingsFor}";
              # Readiness compares the applied migration with the binary's (REQ-015-11),
              # but sqlx creates _sqlx_migrations for the owner alone: the instance role
              # (kartenn_app) could not read it and /health/ready stayed at 503.
              ExecStartPost = pkgs.writeShellScript "kartenn-migrate-grant" ''
                exec ${pkgs.postgresql_18}/bin/psql -v ON_ERROR_STOP=1 --no-psqlrc --quiet \
                  "$(< "$CREDENTIALS_DIRECTORY/database-url")" \
                  -c 'GRANT SELECT ON _sqlx_migrations TO kartenn_app'
              '';
            };
        };

        kartenn-audit-purge = {
          description = "kartenn: purge of the audit journal past its retention";
          after = ["kartenn-migrate.service"];
          requires = ["kartenn-migrate.service"];
          serviceConfig =
            hardening
            // {
              Type = "oneshot";
              User = cfg.database.ownerUser;
              LoadCredential = credentials migrationDatabaseUrlFile;
              ExecStart = "${exe} audit purge ${configFile "kartenn-audit-purge" ownerSettingsFor}";
            };
        };
      }
      // lib.optionalAttrs cfg.database.createLocally {
        # The group role of the instances (INV-11) exists before the migrations, which
        # grant to it; both login roles are members.
        kartenn-db-setup = {
          description = "kartenn: roles of the local database";
          after = ["postgresql.service"] ++ lib.optional (config.systemd.services ? postgresql-setup) "postgresql-setup.service";
          requires = ["postgresql.service"];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            User = "postgres";
            ExecStart = pkgs.writeShellScript "kartenn-db-setup" ''
              ${config.services.postgresql.package}/bin/psql -v ON_ERROR_STOP=1 -d ${cfg.database.name} <<'SQL'
              DO $$ BEGIN
                IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'kartenn_app') THEN
                  CREATE ROLE kartenn_app NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE;
                END IF;
              END $$;
              GRANT kartenn_app TO "${cfg.database.ownerUser}", "${cfg.database.instanceUser}";
              SQL
            '';
          };
        };
      }
      // lib.mapAttrs' (name: instance:
        lib.nameValuePair "kartenn@${name}" {
          description = "kartenn: instance ${name}";
          wantedBy = ["multi-user.target"];
          after = ["network.target" "kartenn-migrate.service"];
          requires = ["kartenn-migrate.service"];
          serviceConfig =
            hardening
            // {
              User = cfg.database.instanceUser;
              LoadCredential = credentials databaseUrlFile;
              ExecStart = "${exe} run ${configFile "kartenn@${name}" (instanceSettings name instance)}";
              # The TLS certificate and the plugins are read again on SIGHUP.
              ExecReload = "${pkgs.coreutils}/bin/kill -HUP $MAINPID";
              Restart = "on-failure";
              RestartSec = 5;
            };
        })
      cfg.instances;

    systemd.timers = {
      kartenn-migrate = {
        description = "kartenn: monthly migration (history partitions)";
        wantedBy = ["timers.target"];
        timerConfig = {
          OnCalendar = "monthly";
          Persistent = true;
        };
      };
      kartenn-audit-purge = {
        description = "kartenn: daily audit purge";
        wantedBy = ["timers.target"];
        timerConfig = {
          OnCalendar = "daily";
          Persistent = true;
        };
      };
    };
  };
}
