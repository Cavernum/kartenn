{
  pkgs,
  lib,
  config,
  ...
}: let
  # Frontend (ADR 0024): pnpm workspace under web/; the tooling stays inert until it exists.
  hasWeb = builtins.pathExists ./web/package.json;
  # Pre-commit hands paths relative to the repository; tools run from web/.
  webHook = tool:
    pkgs.writeShellScript "web-${tool}" ''
      set -euo pipefail
      [ -f web/package.json ] || exit 0
      exec pnpm -C web exec ${tool} "''${@#web/}"
    '';
  # Development console (D35, spec 012/L17): OpenObserve shows logs, traces and metrics
  # in one UI (http://127.0.0.1:5080). Development-only credentials, bound to loopback;
  # the header is the Basic encoding of the same pair.
  openobserve = {
    email = "dev@kartenn.invalid";
    password = "Dev-only-1!";
    basic = "ZGV2QGthcnRlbm4uaW52YWxpZDpEZXYtb25seS0xIQ==";
  };
in {
  imports = [./nix/options.nix];

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
    # Frontend: language servers for Claude Code's LSP tool and the editor, and the
    # browser of Claude Code (claude.code.mcpServers.playwright).
    pkgs.vue-language-server
    pkgs.vtsls
    pkgs.playwright-mcp
  ];

  # Frontend (D17, ADR 0024): Vue 3 + TypeScript shell and plugin SDK, a pnpm workspace
  # under web/. `pnpm install` runs on shell entry once web/package.json exists.
  languages.javascript = {
    enable = true;
    package = pkgs.nodejs_24;
    directory = "web";
    pnpm = {
      enable = true;
      install.enable = hasWeb;
    };
  };
  languages.typescript.enable = true;

  # Browsers come from Nix, never from npm (ADR 0024): @playwright/test in web/ must pin
  # the version of pkgs.playwright-driver.
  env.PLAYWRIGHT_BROWSERS_PATH = "${pkgs.playwright-driver.browsers}";
  env.PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";
  env.PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";

  # Vite dev server, driven through the devenv MCP server (start_process,
  # get_process_logs) rather than a foreground `pnpm dev`. The /api proxy to
  # kartenn-server belongs to the Vite configuration (spec 007).
  processes.web = lib.mkIf hasWeb {
    exec = "exec pnpm -C web --filter shell dev --host 127.0.0.1 --port 5173 --strictPort";
    ready.http.get = {
      host = "127.0.0.1";
      port = 5173;
      path = "/";
    };
  };

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

  # Short outputs for Claude Code sessions (CLAUDE.md, « Économie de contexte »): a
  # verdict, and the detail only on failure; the full logs stay in a temporary directory.
  # `k-check [nextest args…]`, e.g. `k-check -p kartenn-service rbac`.
  scripts.k-check = {
    description = "fmt, clippy and tests, printing a one-line verdict or the failures only";
    exec = ''
      set -uo pipefail
      log=$(mktemp -d -t k-check.XXXXXX)
      if ! cargo fmt --all --check >"$log/fmt" 2>&1; then
        echo "KO fmt ($log/fmt) — cargo fmt --all"; grep '^Diff in' "$log/fmt" | head -20; exit 1
      fi
      if ! cargo clippy -q --workspace --all-targets --all-features >"$log/clippy" 2>&1; then
        echo "KO clippy ($log/clippy)"; grep -E '^(error|warning)|^ *-->' "$log/clippy" | head -40; exit 1
      fi
      warnings=$(grep -c '^warning' "$log/clippy" || true)
      [ "$warnings" -gt 0 ] && warnings=", clippy : $warnings avertissement(s)" || warnings=""
      if ! cargo nextest run --workspace --all-features --hide-progress-bar \
          --status-level fail --final-status-level fail --failure-output final "$@" >"$log/test" 2>&1; then
        echo "KO tests ($log/test)"
        grep -E '^ +(FAIL|SIGSEGV|TIMEOUT|LEAK)|^error' "$log/test" | sort -u | head -20
        echo "---"; grep -E 'panicked at|assertion|left:|right:|^error(\[|:)| --> ' "$log/test" | head -40
        exit 1
      fi
      if [ $# -eq 0 ] && ! cargo test -q --workspace --doc --all-features >"$log/doc" 2>&1; then
        echo "KO doctests ($log/doc)"; grep -E 'FAILED|panicked|error' "$log/doc" | head -30; exit 1
      fi
      echo "OK — $(grep -E '^ +Summary' "$log/test" | tail -1 | sed 's/^ *//')$warnings"
    '';
  };

  # Same for the frontend (ADR 0024): `k-web-check [vitest args…]`, in ${WEB_DIR:-web}.
  # Convention: each workspace package defines a `typecheck` script (vue-tsc); vitest
  # runs from the workspace root (projects).
  scripts.k-web-check = {
    description = "prettier, eslint, vue-tsc and vitest, printing a one-line verdict or the failures only";
    exec = ''
      set -uo pipefail
      export NO_COLOR=1 FORCE_COLOR=0
      dir=''${WEB_DIR:-web}
      [ -f "$dir/package.json" ] || { echo "OK — pas de frontend ($dir/package.json absent)"; exit 0; }
      cd "$dir"
      log=$(mktemp -d -t k-web-check.XXXXXX)
      # ko <step> <log> <pattern> [hint]: the matching lines, else the end of the log.
      ko() {
        echo "KO $1 ($2)''${4:+ — $4}"
        n=$(grep -cE "$3" "$2")
        if [ "$n" -gt 0 ]; then grep -E "$3" "$2" | head -20 | sed "s#$PWD/##"; [ "$n" -gt 20 ] && echo "… $((n - 20)) de plus"; else tail -15 "$2"; fi
        exit 1
      }
      pnpm exec prettier --check . >"$log/prettier" 2>&1 ||
        ko prettier "$log/prettier" '^\[(warn|error)\]' "pnpm -C $dir exec prettier --write ."
      pnpm exec eslint . >"$log/eslint" 2>&1 ||
        ko eslint "$log/eslint" '^/|^ +[0-9]+:[0-9]+ +(error|warning)'
      # Package by package, so that error paths are prefixed with the package directory.
      tsc=0
      while read -r pkg; do
        rel=''${pkg#"$PWD"}; rel=''${rel#/}
        (cd "$pkg" && pnpm run --if-present --silent typecheck) 2>&1 |
          sed -E "s#^([^ ][^(]*\([0-9]+,[0-9]+\): error)#''${rel:+$rel/}\1#" >>"$log/tsc" || tsc=1
      done < <(pnpm -r ls --depth -1 --parseable)
      [ "$tsc" -eq 0 ] || ko vue-tsc "$log/tsc" 'error TS[0-9]+'
      pnpm exec vitest run --reporter=dot "$@" >"$log/test" 2>&1 ||
        ko vitest "$log/test" '^ *(FAIL|×)|AssertionError|Expected|Received|Error:|❯ .*:[0-9]+:[0-9]+'
      echo "OK — $(grep -E '^ *Tests ' "$log/test" | tail -1 | sed 's/^ *//')"
    '';
  };

  # End-to-end journeys (Playwright, browsers from Nix): `k-web-e2e [playwright args…]`.
  scripts.k-web-e2e = {
    description = "Playwright end-to-end tests, printing a one-line verdict or the failures only";
    exec = ''
      set -uo pipefail
      export NO_COLOR=1 FORCE_COLOR=0
      dir=''${WEB_DIR:-web}
      [ -f "$dir/package.json" ] || { echo "OK — pas de frontend ($dir/package.json absent)"; exit 0; }
      cd "$dir"
      log=$(mktemp -d -t k-web-e2e.XXXXXX)
      if ! pnpm exec playwright test --reporter=line "$@" >"$log/e2e" 2>&1; then
        echo "KO playwright ($log/e2e)"
        re='^ *[0-9]+\) |Error:|Expected|Received|^ *at .*:[0-9]+:[0-9]+'
        if grep -qE "$re" "$log/e2e"; then grep -E "$re" "$log/e2e" | head -30 | sed "s#$PWD/##"; else tail -15 "$log/e2e"; fi
        exit 1
      fi
      echo "OK — $(grep -E '^ *[0-9]+ passed' "$log/e2e" | tail -1 | sed 's/^ *//')"
    '';
  };

  # Outline of a file (items with line numbers), to read a targeted range afterwards:
  # `k-ctx <file>…`.
  scripts.k-ctx = {
    description = "Outline a Rust, SQL, Markdown, Vue or TypeScript file with line numbers";
    exec = ''
      set -uo pipefail
      for f in "$@"; do
        echo "== $f ($(wc -l <"$f") lignes)"
        case "$f" in
          *.rs) grep -nE '^\s*(pub(\([a-z]+\))? )?(async |const |unsafe )*(fn|struct|enum|trait|impl|mod|type|macro_rules!) |^\s*#\[(test|sqlx::test|tokio::test)' "$f" ;;
          *.sql) grep -niE '^\s*(create|alter|drop|comment on|grant|revoke) ' "$f" ;;
          *.md) grep -nE '^#{1,4} |^- \[[ x]\] [0-9]+\.' "$f" ;;
          *.vue) grep -nE '^<(script|template|style)|^</(script|template)>|^(export )?(async )?(function|const|interface|type|class) |define(Props|Emits|Model|Slots|Expose)' "$f" ;;
          *.ts|*.tsx|*.js|*.mjs) grep -nE '^(export )?(default )?(declare )?(abstract )?(async )?(function|const|let|interface|type|class|enum|namespace) |^export (\{|\*)|^\s*(describe|it|test)\(' "$f" ;;
          *) grep -nE '^\S' "$f" | head -80 ;;
        esac
      done
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

  # Cost of access control (REQ-005-29) on the budgets of the core, for a person with 50
  # groups and 1 000 bindings: `bench-rbac [seed] [small]`.
  scripts.bench-rbac = {
    description = "Measure the cost of access control on a fresh kartenn_bench database";
    exec = ''
      set -euo pipefail
      psql "$DATABASE_URL" -c 'DROP DATABASE IF EXISTS kartenn_bench' -c 'CREATE DATABASE kartenn_bench'
      cargo run --release -q -p kartenn-bench -- rbac "''${DATABASE_URL%/*}/kartenn_bench" "''${1:-42}" ''${2:+"$2"}
    '';
  };

  # The budgets of the core through HTTP, then 200 users and 50 writes per second
  # (REQ-006-39): `bench-http [seed] [small]`.
  scripts.bench-http = {
    description = "Measure the budgets of the core through HTTP on a fresh kartenn_bench database";
    exec = ''
      set -euo pipefail
      psql "$DATABASE_URL" -c 'DROP DATABASE IF EXISTS kartenn_bench' -c 'CREATE DATABASE kartenn_bench'
      cargo run --release -q -p kartenn-bench -- http "''${DATABASE_URL%/*}/kartenn_bench" "''${1:-42}" ''${2:+"$2"}
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
    # Frontend, with web/'s own tool versions; no-ops until web/ exists.
    web-prettier = {
      enable = true;
      name = "prettier (web)";
      entry = "${webHook "prettier"} --write --ignore-unknown";
      files = "^web/.*\\.(vue|ts|tsx|js|mjs|json|css|html|md)$";
      excludes = ["(^|/)pnpm-lock\\.yaml$"];
    };
    web-eslint = {
      enable = true;
      name = "eslint (web)";
      entry = "${webHook "eslint"} --fix";
      files = "^web/.*\\.(vue|ts|tsx|js|mjs)$";
    };
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
    k-web-check
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
    # Off locally with `kartenn.claude.openobserveMcp = false` (devenv.local.nix).
    mcpServers.openobserve = lib.mkIf config.kartenn.claude.openobserveMcp {
      type = "http";
      url = "http://127.0.0.1:5080/api/default/mcp";
      headers.Authorization = "Basic ${openobserve.basic}";
    };
    # Browser of Claude Code (ADR 0024): headless Chromium from Nix (the wrapper sets
    # PLAYWRIGHT_BROWSERS_PATH), in-memory profile, requests limited to the local Vite
    # dev and preview servers.
    mcpServers.playwright = {
      type = "stdio";
      command = "${pkgs.playwright-mcp}/bin/playwright-mcp";
      args = [
        "--headless"
        "--isolated"
        "--allowed-origins"
        "http://127.0.0.1:5173;http://localhost:5173;http://127.0.0.1:4173"
        "--output-dir"
        "${config.devenv.state}/playwright-mcp"
      ];
    };
    permissions.rules.Bash.allow = ["k-web-check:*" "k-web-e2e:*"];
    # The user-level Claude Code config already runs `pre-commit run --files`
    # after each edit; devenv's own copy of that hook would run it twice.
    hooks.git-hooks-run.enable = false;
    # Context economy (CLAUDE.md): compact while the prompt cache is warm, after a task
    # commit, and stop once before a prompt that would rewrite an expired cache.
    hooks.fin-tache = {
      name = "Advise /compact after a task commit";
      hookType = "Stop";
      command = ''"$CLAUDE_PROJECT_DIR"/.claude/hooks/fin-tache.sh'';
    };
    hooks.reprise-froide = {
      name = "Stop once before rewriting an expired cache";
      hookType = "UserPromptSubmit";
      command = ''"$CLAUDE_PROJECT_DIR"/.claude/hooks/reprise-froide.sh'';
    };
  };
}
