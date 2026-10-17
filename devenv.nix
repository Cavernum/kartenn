{
  pkgs,
  config,
  ...
}: let
  # Development console (D35, spec 012/L17): OpenObserve shows logs, traces and metrics
  # in one UI (http://127.0.0.1:5080). Development-only credentials, bound to loopback;
  # the header is the Basic encoding of the same pair.
  openobserve = {
    email = "dev@kartenn.invalid";
    password = "Dev-only-1!";
    basic = "ZGV2QGthcnRlbm4uaW52YWxpZDpEZXYtb25seS0xIQ==";
  };
in {
  # Backend (D16): Rust workspace under crates/.
  languages.rust = {
    enable = true;
    # Components: rustc, cargo, clippy, rustfmt, rust-analyzer. Crates linking
    # system libraries (openssl-sys…) need the library and pkgs.pkg-config in
    # `packages`.
  };

  # Workspace tooling (docs/practices/rust-build.md): feature unification,
  # sqlx offline metadata, per-process test runner.
  packages = [
    pkgs.cargo-hakari
    pkgs.sqlx-cli
    pkgs.cargo-nextest
  ];

  # Frontend (D17): Vue 3 + TypeScript shell and plugin SDK under web/.
  languages.javascript = {
    enable = true;
    package = pkgs.nodejs_24;
  };
  languages.typescript.enable = true;

  # Storage adapter (D15): PostgreSQL >= 18 for local development.
  services.postgres = {
    enable = true;
    package = pkgs.postgresql_18;
    listen_addresses = "127.0.0.1";
    # Development owner role: creates the throwaway test databases of #[sqlx::test].
    initialDatabases = [
      {
        name = "kartenn";
        user = "kartenn";
      }
    ];
    # Application role without administration rights (INV-11): tests switch to it
    # with SET ROLE to prove what the application can and cannot do.
    initialScript = ''
      ALTER ROLE kartenn CREATEDB;
      CREATE ROLE kartenn_app NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE;
      GRANT kartenn_app TO kartenn;
    '';
  };

  # sqlx (compile-time checks, #[sqlx::test]) and integration tests use the local
  # server; #[sqlx::test] creates a throwaway database per test.
  env.DATABASE_URL = "postgres://kartenn@127.0.0.1:5432/kartenn";
  # Builds never need the database: sqlx macros read the versioned .sqlx/ metadata
  # (docs/practices/rust-build.md §2.3). Refresh it with `db-prepare`.
  env.SQLX_OFFLINE = "true";

  scripts.db-prepare = {
    description = "Migrate the development database and refresh the sqlx offline metadata";
    exec = ''
      set -euo pipefail
      sqlx migrate run --source crates/kartenn-store-pg/migrations
      cargo sqlx prepare --workspace -- --all-targets
    '';
  };

  # Budgets of the properties (spec 003, REQ-003-26) on the reference dataset of
  # REQ-002-45, in a database recreated for the run: `bench-properties [seed] [small]`.
  scripts.bench-properties = {
    description = "Check the budgets of the properties on a fresh kartenn_bench database";
    exec = ''
      set -euo pipefail
      psql "$DATABASE_URL" -c 'DROP DATABASE IF EXISTS kartenn_bench' -c 'CREATE DATABASE kartenn_bench'
      cargo run --release -q -p kartenn-bench -- props "''${DATABASE_URL%/*}/kartenn_bench" "''${1:-42}" ''${2:+"$2"}
    '';
  };

  # Budgets of the core (REQ-002-45, REQ-004-38) on the same dataset with three years of
  # history: `bench-core [seed] [small]`.
  scripts.bench-core = {
    description = "Check the budgets of the core on a fresh kartenn_bench database";
    exec = ''
      set -euo pipefail
      psql "$DATABASE_URL" -c 'DROP DATABASE IF EXISTS kartenn_bench' -c 'CREATE DATABASE kartenn_bench'
      cargo run --release -q -p kartenn-bench -- core "''${DATABASE_URL%/*}/kartenn_bench" "''${1:-42}" ''${2:+"$2"}
    '';
  };

  # Cost of the telemetry (REQ-012-23) on the budgets of the core, and volume of the
  # audit of reads (REQ-012-18): `bench-telemetry [seed] [small]`.
  scripts.bench-telemetry = {
    description = "Measure the cost of the telemetry on a fresh kartenn_bench database";
    exec = ''
      set -euo pipefail
      psql "$DATABASE_URL" -c 'DROP DATABASE IF EXISTS kartenn_bench' -c 'CREATE DATABASE kartenn_bench'
      cargo run --release -q -p kartenn-bench -- telemetry "''${DATABASE_URL%/*}/kartenn_bench" "''${1:-42}" ''${2:+"$2"}
    '';
  };

  # Observability (D35): the applications send OTLP to the collector
  # (127.0.0.1:4317 gRPC, 4318 HTTP), which also accepts syslog RFC 5424 over TCP with
  # octet counting (127.0.0.1:5514). Everything goes to OpenObserve (logs, traces,
  # metrics; UI http://127.0.0.1:5080); logs are also printed in the collector's
  # process log.
  services.opentelemetry-collector = {
    enable = true;
    settings = {
      receivers = {
        otlp.protocols = {
          grpc.endpoint = "127.0.0.1:4317";
          http.endpoint = "127.0.0.1:4318";
        };
        syslog = {
          tcp.listen_address = "127.0.0.1:5514";
          protocol = "rfc5424";
          enable_octet_counting = true;
          # kartenn writes its event in JSON as MSG (after the RFC 5424 BOM): parsed
          # into the `kartenn` attribute, so that the console indexes each field. Any
          # other message stays as text.
          operators = [
            {
              type = "regex_parser";
              "if" = ''attributes.message matches "^\\x{FEFF}?\\{"'';
              parse_from = "attributes.message";
              regex = ''^\x{FEFF}?(?P<message_json>\{.*)$'';
            }
            {
              type = "json_parser";
              "if" = "attributes.message_json != nil";
              parse_from = "attributes.message_json";
              parse_to = "attributes.kartenn";
            }
            {
              type = "remove";
              "if" = "attributes.message_json != nil";
              field = "attributes.message_json";
            }
          ];
        };
      };
      processors.batch = {};
      exporters = {
        "otlp_http/openobserve" = {
          endpoint = "http://127.0.0.1:5080/api/default";
          headers.Authorization = "Basic ${openobserve.basic}";
        };
        debug.verbosity = "basic";
      };
      service.pipelines = {
        traces = {
          receivers = ["otlp"];
          processors = ["batch"];
          exporters = ["otlp_http/openobserve"];
        };
        logs = {
          receivers = ["otlp" "syslog"];
          processors = ["batch"];
          exporters = ["debug" "otlp_http/openobserve"];
        };
        metrics = {
          receivers = ["otlp"];
          processors = ["batch"];
          exporters = ["otlp_http/openobserve"];
        };
      };
    };
  };
  # ZO_TELEMETRY=false: no usage reports sent by the console.
  processes.openobserve = {
    exec = ''
      exec env ZO_DATA_DIR=${config.devenv.state}/openobserve \
        ZO_HTTP_ADDR=127.0.0.1 ZO_GRPC_ADDR=127.0.0.1 ZO_TELEMETRY=false \
        ZO_ROOT_USER_EMAIL='${openobserve.email}' ZO_ROOT_USER_PASSWORD='${openobserve.password}' \
        ${pkgs.openobserve}/bin/openobserve
    '';
    ready.http.get = {
      host = "127.0.0.1";
      port = 5080;
      path = "/healthz";
    };
  };

  git-hooks.hooks = {
    # Generic hygiene, every project.
    check-merge-conflicts.enable = true;
    end-of-file-fixer = {
      enable = true;
      # devenv writes its lock without a final newline, on every update.
      excludes = ["^devenv\\.lock$"];
    };
    trim-trailing-whitespace.enable = true;
    rustfmt.enable = true;
    clippy.enable = true;
  };

  enterTest = ''
    # No application code yet: the workspace appears with the first crate.
    if [ -f Cargo.toml ]; then
      cargo hakari generate --diff
      cargo hakari manage-deps --dry-run
      sqlx migrate run --source crates/kartenn-store-pg/migrations
      cargo sqlx prepare --workspace --check -- --all-targets
      cargo test --workspace --all-targets --all-features
    fi
    postgres --version
  '';

  # Project-local devenv MCP server: devenv writes .mcp.json from this.
  claude.code = {
    enable = true;
    mcpServers.devenv = {
      type = "stdio";
      command = "devenv";
      args = ["mcp"];
      env.DEVENV_ROOT = config.devenv.root;
    };
    # The console's own MCP server (SQL over the logs, stream schemas, traces,
    # PromQL), available once `devenv up` runs OpenObserve. Development credentials.
    mcpServers.openobserve = {
      type = "http";
      url = "http://127.0.0.1:5080/api/default/mcp";
      headers.Authorization = "Basic ${openobserve.basic}";
    };
    # The user-level Claude Code config already runs `pre-commit run --files`
    # after each edit; devenv's own copy of that hook would run it twice.
    hooks.git-hooks-run.enable = false;
  };
}
