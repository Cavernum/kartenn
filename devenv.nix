{
  pkgs,
  config,
  ...
}: let
  # Development console of the traces (D35): Jaeger v2, a static binary from its
  # release (not in nixpkgs; no container runtime needed). x86_64-linux only.
  jaeger = pkgs.stdenvNoCC.mkDerivation rec {
    pname = "jaeger";
    version = "2.21.0";
    src = pkgs.fetchurl {
      url = "https://github.com/jaegertracing/jaeger/releases/download/v${version}/jaeger-${version}-linux-amd64.tar.gz";
      sha256 = "0nkszimnhv7vfmv5xv430znp4f5hs7ygmzz1l5j2wficx8c16x7m";
    };
    installPhase = "install -Dm755 jaeger $out/bin/jaeger";
  };
  yaml = pkgs.formats.yaml {};
  # Jaeger keeps the traces in memory and receives them from the collector only, on
  # ports of its own (the collector holds 4317/4318, 8888 and 13133).
  jaegerConfig = yaml.generate "jaeger.yaml" {
    service = {
      extensions = ["jaeger_storage" "jaeger_query"];
      pipelines.traces = {
        receivers = ["otlp"];
        processors = ["batch"];
        exporters = ["jaeger_storage_exporter"];
      };
      telemetry.metrics.level = "none";
    };
    extensions = {
      jaeger_storage.backends.memstore.memory.max_traces = 100000;
      jaeger_query = {
        storage.traces = "memstore";
        http.endpoint = "127.0.0.1:16686";
        grpc.endpoint = "127.0.0.1:16685";
      };
    };
    receivers.otlp.protocols.grpc.endpoint = "127.0.0.1:14317";
    processors.batch = {};
    exporters.jaeger_storage_exporter.trace_storage = "memstore";
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

  # Observability (D35): the applications send OTLP to the collector
  # (127.0.0.1:4317 gRPC, 4318 HTTP), which also accepts syslog RFC 5424 over TCP
  # (127.0.0.1:5514). Traces go to Jaeger (UI http://127.0.0.1:16686); logs and
  # metrics are printed in the collector's process log and written as OTLP JSON
  # lines in .devenv/state/otel-{logs,metrics}.jsonl.
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
        };
      };
      processors.batch = {};
      exporters = {
        "otlp_grpc/jaeger" = {
          endpoint = "127.0.0.1:14317";
          tls.insecure = true;
        };
        debug.verbosity = "basic";
        "file/logs".path = "${config.devenv.state}/otel-logs.jsonl";
        "file/metrics".path = "${config.devenv.state}/otel-metrics.jsonl";
      };
      service.pipelines = {
        traces = {
          receivers = ["otlp"];
          processors = ["batch"];
          exporters = ["otlp_grpc/jaeger"];
        };
        logs = {
          receivers = ["otlp" "syslog"];
          processors = ["batch"];
          exporters = ["debug" "file/logs"];
        };
        metrics = {
          receivers = ["otlp"];
          processors = ["batch"];
          exporters = ["file/metrics"];
        };
      };
    };
  };
  processes.jaeger = {
    exec = "exec ${jaeger}/bin/jaeger --config ${jaegerConfig}";
    ready.http.get = {
      host = "127.0.0.1";
      port = 16686;
      path = "/";
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
    # The user-level Claude Code config already runs `pre-commit run --files`
    # after each edit; devenv's own copy of that hook would run it twice.
    hooks.git-hooks-run.enable = false;
  };
}
