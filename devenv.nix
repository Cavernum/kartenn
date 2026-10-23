{
  pkgs,
  lib,
  config,
  ...
}: let
  # Frontend (ADR 0024): pnpm workspace under web/; the tooling stays inert until it exists.
  hasWeb = builtins.pathExists ./web/package.json;
  # The shell scripts of the project are POSIX sh run by the dash of Nix (>= 0.5.13, which
  # has `set -o pipefail`; the /bin/sh of a Debian or Ubuntu runner may not): the scripts
  # written here (pkgs.writers.writeDash), the devenv scripts (nix/dash-scripts.nix) and
  # scripts/*.sh. A script re-executed under k-heavy names this dash, never `sh`.
  dash = lib.getExe pkgs.dash;
  # Pre-commit hands paths relative to the repository; tools run from the workspace (web/,
  # or web/v2/ for the new shell, spec 021 « Migration »).
  # The toolchain is absolute: pre-commit also runs outside the devenv shell.
  workspaceHook = dir: tool:
    pkgs.writers.writeDash "${builtins.replaceStrings ["/"] ["-"] dir}-${tool}" ''
      set -euo pipefail
      [ -f ${dir}/package.json ] || exit 0
      export PATH=${lib.makeBinPath [config.languages.javascript.package config.languages.javascript.pnpm.package]}:$PATH
      for path do shift; set -- "$@" "''${path#${dir}/}"; done
      exec pnpm -C ${dir} exec ${tool} "$@"
    '';
  webHook = workspaceHook "web";
  web2Hook = workspaceHook "web/v2";
  # Machine-wide semaphore for heavy commands (builds, tests, benches, end-to-end): several
  # sessions and worktrees share one machine, and each cargo would otherwise take every
  # core. At most K_HEAVY_SLOTS (default 2) commands run at once, niced, each with
  # its share of the cores (K_HEAVY_JOBS to override); the others wait their turn.
  # Re-entrant: a heavy command run under a slot does not take a second one. Variables
  # stay out of the KARTENN_ prefix, which the server reads as its configuration.
  # `k-heavy --pg <command>` also takes the exclusive lock of the shared PostgreSQL (5432):
  # the #[sqlx::test] runs of two worktrees would drop each other's test databases, and the
  # end-to-end journeys own kartenn_e2e and fixed ports (k-check, db-prepare, k-web-e2e).
  # `k-heavy --bench <command>` takes the exclusive lock of the bench instance instead
  # (5433, k-pg-bench, ADR 0032): the benches recreate kartenn_bench there, so a bench and
  # a k-check no longer wait for each other. Locks are taken in the order bench, pg, then
  # the slot: a command waiting for a lock holds no slot, so the cores stay free for builds
  # meanwhile. Nothing may ask for a lock while holding a slot (k-check takes them phase by
  # phase).
  # `k-heavy --all <command>` takes every slot, in order, for the timing-sensitive end-to-end
  # journeys: a build beside them stretches browser timeouts.
  # A waiting caller says on stderr who holds what, since when, and repeats it every
  # 5 min; it gives up with code 75 after K_HEAVY_WAIT minutes (45 for a lock, no limit
  # for a slot by default; 0 for no limit).
  kHeavy = pkgs.writers.writeDashBin "k-heavy" ''
    set -uo pipefail
    pg=0
    bench=0
    all=0
    while :; do
      case "''${1:-}" in
        --pg) pg=1; shift ;;
        --bench) bench=1; shift ;;
        --all) all=1; shift ;;
        *) break ;;
      esac
    done
    [ $# -gt 0 ] || { echo "usage: k-heavy [--bench] [--pg] [--all] <command> [args…]" >&2; exit 2; }
    slots=''${K_HEAVY_SLOTS:-2}
    jobs=''${K_HEAVY_JOBS:-$(( $(${pkgs.coreutils}/bin/nproc) / slots ))}
    [ "$jobs" -ge 1 ] || jobs=1
    dir=''${XDG_RUNTIME_DIR:-/tmp}/kartenn-heavy-$(${pkgs.coreutils}/bin/id -u)
    mkdir -p "$dir"
    flock=${pkgs.util-linux}/bin/flock
    now() { ${pkgs.coreutils}/bin/date +%s; }
    # Who holds what: `<name>.who` holds the pid, the time, the worktree and the command of
    # the holder. The command is exec'd under the lock, so nothing removes the file on
    # exit: the next holder overwrites it, and a waiter ignores it once its pid is dead.
    cmd=$(printf '%s ' "$@" | ${pkgs.coreutils}/bin/tr '\n' ' ' | ${pkgs.coreutils}/bin/cut -c1-100)
    claim() { printf '%s\n%s\n%s\n%s\n' "$$" "$(now)" "$PWD" "''${cmd% }" >"$dir/$1.who"; }
    holder() {
      local pid since tree what
      { read -r pid; read -r since; read -r tree; read -r what; } <"$dir/$1.who" 2>/dev/null
      if [ -n "''${pid:-}" ] && kill -0 "$pid" 2>/dev/null; then
        echo "tenu par $what ($tree) depuis $(( ($(now) - since) / 60 )) min"
      else
        echo "tenu par un processus inconnu"
      fi
    }
    # Waits for the lock <name> on <fd>, one line every 5 min, for <limit> minutes at most
    # (empty or 0: no limit), then exits with 75 (EX_TEMPFAIL).
    acquire() {
      local fd=$1 name=$2 limit=$3 waited=0 step
      if ! "$flock" -n "$fd"; then
        while :; do
          echo "k-heavy : attente du verrou $name, $(holder "$name")" >&2
          step=300
          if [ -n "$limit" ] && [ "$limit" -gt 0 ]; then
            [ $(( limit * 60 - waited )) -ge "$step" ] || step=$(( limit * 60 - waited ))
          fi
          "$flock" -w "$step" "$fd" && break
          waited=$(( waited + step ))
          if [ -n "$limit" ] && [ "$limit" -gt 0 ] && [ "$waited" -ge $(( limit * 60 )) ]; then
            echo "k-heavy : verrou $name toujours $(holder "$name") ; abandon après $limit min" \
              "(K_HEAVY_WAIT=<minutes> pour attendre plus, 0 sans limite)" >&2
            exit 75
          fi
        done
      fi
      claim "$name"
    }
    # Minutes of waiting: 45 for the locks, no limit for the slots, unless K_HEAVY_WAIT.
    lockwait=''${K_HEAVY_WAIT:-45}
    slotwait=''${K_HEAVY_WAIT:-}
    if [ "$bench" -eq 1 ] && [ -z "''${K_BENCH_HELD:-}" ]; then
      exec 7>"$dir/bench"
      acquire 7 bench "$lockwait"
      export K_BENCH_HELD=1
    fi
    if [ "$pg" -eq 1 ] && [ -z "''${K_PG_HELD:-}" ]; then
      exec 8>"$dir/pg"
      acquire 8 pg "$lockwait"
      export K_PG_HELD=1
    fi
    # Each command runs in its own systemd scope under a memory ceiling (K_HEAVY_MEM, in
    # percent of the RAM: 35 per slot, 60 for --all): a runaway build or bench is killed
    # alone, inside its scope. Without it, the kernel's global OOM killer hits the
    # terminal's scope, and systemd then stops the whole terminal (OOMPolicy=stop).
    run() {
      export CARGO_BUILD_JOBS=$jobs NEXTEST_TEST_THREADS=$jobs
      if [ -z "''${K_HEAVY_SCOPED:-}" ] && command -v systemd-run >/dev/null 2>&1 \
        && [ -S "''${XDG_RUNTIME_DIR:-/nonexistent}/bus" ]; then
        export K_HEAVY_SCOPED=1
        if [ "$all" -eq 1 ]; then pct=''${K_HEAVY_MEM:-60}; else pct=''${K_HEAVY_MEM:-35}; fi
        exec systemd-run --user --scope --quiet --collect \
          -p MemoryMax="$pct%" -p MemorySwapMax=2G \
          ${pkgs.coreutils}/bin/nice -n 10 "$@"
      fi
      exec ${pkgs.coreutils}/bin/nice -n 10 "$@"
    }
    [ -z "''${K_HEAVY_HELD:-}" ] || run "$@"
    if [ "$all" -eq 1 ]; then
      # In order, blocking: two --all callers cannot deadlock, and a plain caller never
      # waits while holding a slot. Slot i on fd 2+i: dash only opens fds 0 to 9, and 7 to 9
      # are taken above.
      [ "$slots" -le 4 ] || { echo "k-heavy --all : au plus 4 créneaux (K_HEAVY_SLOTS=$slots)" >&2; exit 2; }
      for i in $(${pkgs.coreutils}/bin/seq 1 "$slots"); do
        fd=$(( i + 2 ))
        eval "exec $fd>\"\$dir/slot.$i\""
        acquire "$fd" "slot.$i" "$slotwait"
      done
      export K_HEAVY_HELD=all
      jobs=$(${pkgs.coreutils}/bin/nproc)
      run "$@"
    fi
    # Any free slot: polled every 5 s, a line every 5 min while all are taken.
    begun=$(now)
    told=0
    while :; do
      for i in $(${pkgs.coreutils}/bin/seq 1 "$slots"); do
        exec 9>"$dir/slot.$i"
        # The lock stays held by the command (and its children) through the inherited fd.
        if "$flock" -n 9; then
          claim "slot.$i"
          [ "$told" -eq 0 ] || echo "k-heavy : créneau $i obtenu" >&2
          export K_HEAVY_HELD=$i
          run "$@"
        fi
        exec 9>&-
      done
      waited=$(( $(now) - begun ))
      if [ -n "$slotwait" ] && [ "$slotwait" -gt 0 ] && [ "$waited" -ge $(( slotwait * 60 )) ]; then
        echo "k-heavy : aucun créneau libre après $slotwait min, abandon" >&2
        exit 75
      fi
      if [ "$told" -eq 0 ] || [ $(( $(now) - told )) -ge 300 ]; then
        for i in $(${pkgs.coreutils}/bin/seq 1 "$slots"); do
          echo "k-heavy : attente d'un créneau, slot.$i $(holder "slot.$i")" >&2
        done
        told=$(now)
      fi
      sleep 5
    done
  '';
  # The PostgreSQL instance of the benches (ADR 0032): the package, the configuration and
  # the initial roles and databases of services.postgres, on port 5433, its data shared
  # by every worktree.
  pgBench = let
    pg = config.services.postgres;
    value = v:
      if builtins.isBool v
      then lib.boolToString v
      else if builtins.isString v
      then "'${lib.replaceStrings ["'"] ["''"] v}'"
      else toString v;
    settings =
      builtins.removeAttrs pg.settings ["listen_addresses" "port" "unix_socket_directories"];
    conf = lib.concatStrings (lib.mapAttrsToList (k: v: "${k} = ${value v}\n") settings);
    initial =
      lib.concatMapStrings (db:
        if db.user or null == null
        then "CREATE DATABASE \"${db.name}\";\n"
        else ''
          CREATE ROLE "${db.user}" LOGIN;
          CREATE DATABASE "${db.name}" OWNER "${db.user}";
        '')
      pg.initialDatabases
      + pg.initialScript;
  in
    pkgs.writers.writeDashBin "k-pg-bench" ''
      set -euo pipefail
      export PATH=${lib.makeBinPath [pg.package pkgs.coreutils pkgs.util-linux]}:$PATH
      base=''${XDG_STATE_HOME:-$HOME/.local/state}/kartenn/pg-bench
      data=$base/data
      port=5433
      ready() { pg_isready -q -h 127.0.0.1 -p "$port"; }
      ready && exit 0
      mkdir -p "$base"
      exec 6>"$base/lock"
      flock 6
      ready && exit 0
      config() {
        { printf '%s' ${lib.escapeShellArg conf}
          echo "listen_addresses = '$1'"
          echo "port = $port"
          echo "unix_socket_directories = '$base'"
        } >"$2/postgresql.conf"
      }
      if [ ! -f "$data/PG_VERSION" ]; then
        # Once, under the lock: a directory moved into place only when complete.
        echo "k-pg-bench : initialisation de $data" >&2
        rm -rf "$base/data.new"
        initdb -D "$base/data.new" --locale=C --encoding=UTF8 --auth=trust >"$base/initdb.log"
        config "" "$base/data.new"
        pg_ctl -D "$base/data.new" -l "$base/initdb.log" -w start >/dev/null
        printf '%s' ${lib.escapeShellArg initial} |
          psql -h "$base" -p "$port" -d postgres -q -v ON_ERROR_STOP=1 -f -
        pg_ctl -D "$base/data.new" -m fast -w stop >/dev/null
        mv "$base/data.new" "$data"
      fi
      config 127.0.0.1 "$data"
      echo "k-pg-bench : démarrage de l'instance des bancs (127.0.0.1:$port)" >&2
      if command -v systemd-run >/dev/null 2>&1 && [ -S "''${XDG_RUNTIME_DIR:-/nonexistent}/bus" ]; then
        # A transient user unit: the instance outlives the terminal and the k-heavy scope.
        if ! systemctl --user is-active --quiet kartenn-pg-bench; then
          systemctl --user reset-failed kartenn-pg-bench 2>/dev/null || true
          systemd-run --user --unit=kartenn-pg-bench --collect --quiet \
            --description="PostgreSQL des bancs kartenn (ADR 0032)" \
            -p KillSignal=SIGINT -p KillMode=mixed \
            ${pg.package}/bin/postgres -D "$data"
        fi
      else
        # Without the descriptors of k-heavy (3 to 9): the server would hold its locks
        # forever. dash cannot name a descriptor above 9.
        (
          for fd in "/proc/$$/fd"/*; do
            n=''${fd##*/}
            [ "$n" -le 2 ] || [ "$n" -gt 9 ] || eval "exec $n>&-" 2>/dev/null || true
          done
          pg_ctl -D "$data" -l "$base/postgres.log" -w start >/dev/null
        )
      fi
      for _ in $(seq 1 120); do
        ready && exit 0
        sleep 0.5
      done
      echo "k-pg-bench : l'instance ne répond pas sur 127.0.0.1:$port" \
        "(journalctl --user -u kartenn-pg-bench, ou $base/postgres.log)" >&2
      exit 1
    '';
  # A bench script (ADR 0032): under the bench lock, on a kartenn_bench database recreated
  # in the bench instance; `<name> [seed] [small]`.
  benchDatabase = "postgres://kartenn@127.0.0.1:5433/kartenn_bench";
  bench = command: description: {
    inherit description;
    exec = ''
      set -euo pipefail
      [ -n "''${K_BENCH_HELD:-}" ] || exec k-heavy --bench ${dash} "$0" "$@"
      k-pg-bench
      bench=''${K_BENCH_DATABASE_URL:-${benchDatabase}}
      PGOPTIONS='-c client_min_messages=warning' psql "''${bench%/*}/postgres" -q -c "DROP DATABASE IF EXISTS \"''${bench##*/}\" WITH (FORCE)" \
        -c "CREATE DATABASE \"''${bench##*/}\""
      cargo run --release -q -p kartenn-bench -- ${command} "$bench" "''${1:-42}" ''${2:+"$2"}
    '';
  };
  # The clippy hook under the machine-wide semaphore; absolute toolchain: pre-commit also
  # runs outside the devenv shell.
  clippyHook = pkgs.writers.writeDash "clippy-heavy" ''
    export PATH=${lib.makeBinPath [pkgs.cargo pkgs.clippy pkgs.rustc pkgs.stdenv.cc pkgs.pkg-config pkgs.coreutils]}:$PATH
    export SQLX_OFFLINE=true RUSTC_WRAPPER=${pkgs.sccache}/bin/sccache
    exec ${kHeavy}/bin/k-heavy cargo clippy --offline --workspace --all-targets --all-features
  '';
  # Every dash script of this file, as dash runs it, for `shellcheck -s dash`
  # (scripts/ci-lint.sh): the devenv scripts are the text of their `exec`.
  lintedScripts =
    [
      "${kHeavy}/bin/k-heavy"
      "${pgBench}/bin/k-pg-bench"
      (webHook "prettier")
      clippyHook
    ]
    ++ lib.mapAttrsToList (name: script: pkgs.writeText "${name}.sh" script.exec)
    (lib.filterAttrs (_: script: script.package == pkgs.dash) config.scripts);
  # Development console (D35, spec 012/L17): OpenObserve shows logs, traces and metrics
  # in one UI (http://127.0.0.1:5080). Development-only credentials, bound to loopback;
  # the header is the Basic encoding of the same pair.
  openobserve = {
    email = "dev@kartenn.invalid";
    password = "Dev-only-1!";
    basic = "ZGV2QGthcnRlbm4uaW52YWxpZDpEZXYtb25seS0xIQ==";
  };
in {
  imports = [./nix/options.nix ./nix/dash-scripts.nix];

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
    kHeavy
    pgBench
    pkgs.sccache
    # Frontend: language servers for Claude Code's LSP tool and the editor, and the
    # browser of Claude Code (claude.code.mcpServers.playwright).
    pkgs.vue-language-server
    pkgs.vtsls
    pkgs.playwright-mcp
    # Releases (ADR 0033): notes from the Conventional Commits. gpg and gh stay those of
    # the system: the maintainer's agent and keyring, the maintainer's login.
    pkgs.git-cliff
    pkgs.jq
    # Lint of the GitHub workflows and of the scripts (ADR 0033).
    pkgs.actionlint
    pkgs.zizmor
    pkgs.shellcheck
    # Python lint and format (spec 023, T25).
    pkgs.ruff
    # The shell of the scripts (`#!/usr/bin/env dash`, see `dash` above).
    pkgs.dash
  ];
  # The scripts of this file as dash runs them, for `shellcheck -s dash` (scripts/ci-lint.sh).
  env.K_DASH_SCRIPTS = toString lintedScripts;

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

  # Python client library and CLI (spec 023, T25): a uv workspace under clients/python/,
  # tested on the floor version of the packages (Python 3.11, Q-023-4). The interpreter
  # comes from Nix, never downloaded by uv; pyright is the language server of devenv
  # (pkgs.pyright), ruff the one of nixpkgs; pytest and its plugins are locked in
  # clients/python/uv.lock.
  languages.python = {
    enable = true;
    package = pkgs.python311;
    directory = "clients/python";
    uv.enable = true;
  };
  env.UV_PYTHON_DOWNLOADS = "never";
  env.UV_PYTHON = lib.getExe config.languages.python.package;

  # Browsers come from Nix, never from npm (ADR 0024): @playwright/test in web/ must pin
  # the version of pkgs.playwright-driver.
  env.PLAYWRIGHT_BROWSERS_PATH = "${pkgs.playwright-driver.browsers}";
  env.PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";
  env.PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";

  # Vite dev server, driven through the devenv MCP server (start_process,
  # get_process_logs) rather than a foreground `pnpm dev`. Vite relays /v1 and /plugins
  # to the `kartenn` process (spec 007, REQ-007-36): the browser sees one origin.
  processes.web = lib.mkIf hasWeb {
    exec = "exec pnpm -C web --filter shell dev --host 127.0.0.1 --port 5173 --strictPort";
    ready.http.get = {
      host = "127.0.0.1";
      port = 5173;
      path = "/";
    };
  };

  # Development OIDC issuer (spec 007, REQ-007-35), https://127.0.0.1:8444: users and
  # groups in dev/idp.toml; authority, certificate and secrets written once under
  # dev/state. Never shipped.
  processes.dev-idp.exec = ''
    cargo run -q -p kartenn-dev-idp -- certs dev/state
    exec cargo run -q -p kartenn-dev-idp -- serve dev/idp.toml
  '';

  # An instance for development, https://127.0.0.1:8443 (dev/kartenn.toml), trusting the
  # development authority for its calls to the issuer only through SSL_CERT_FILE.
  processes.kartenn = {
    exec = ''
      cargo run -q -p kartenn-dev-idp -- certs dev/state
      cargo run -q -p kartenn-server -- migrate "$DATABASE_URL"
      exec env SSL_CERT_FILE=dev/state/ca.pem \
        cargo run -q -p kartenn-server -- run "$DATABASE_URL" dev/kartenn.toml
    '';
    process-compose.depends_on.postgres.condition = "process_healthy";
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

  # Load guards (see kHeavy): a cargo run outside k-heavy (rust-analyzer, an ad hoc
  # command) takes 6 jobs, not every core; and one compilation cache shared by every
  # worktree, so a new worktree does not rebuild all dependencies.
  env.CARGO_BUILD_JOBS = "6";
  env.RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";
  env.SCCACHE_CACHE_SIZE = "40G";

  scripts.db-prepare = {
    description = "Migrate the development database and refresh the sqlx offline metadata";
    exec = ''
      set -euo pipefail
      [ -n "''${K_PG_HELD:-}" ] || exec k-heavy --pg ${dash} "$0" "$@"
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
      # Phase by phase under k-heavy: builds take a slot, tests take the PostgreSQL lock
      # then a slot; no slot is held while waiting for the lock.
      log=$(mktemp -d -t k-check.XXXXXX)
      if ! cargo fmt --all --check >"$log/fmt" 2>&1; then
        echo "KO fmt ($log/fmt) — cargo fmt --all"; grep '^Diff in' "$log/fmt" | head -20; exit 1
      fi
      if ! k-heavy cargo clippy -q --workspace --all-targets --all-features >"$log/clippy" 2>&1; then
        echo "KO clippy ($log/clippy)"; grep -E '^(error|warning)|^ *-->' "$log/clippy" | head -40; exit 1
      fi
      warnings=$(grep -c '^warning' "$log/clippy" || true)
      [ "$warnings" -gt 0 ] && warnings=", clippy : $warnings avertissement(s)" || warnings=""
      # `-p <crate>` narrows the run: --workspace would add every other crate back.
      ws=--workspace
      for a in "$@"; do case $a in -p | -p* | --package | --package=*) ws= ;; esac; done
      # The build takes the package selection only: run options (--no-fail-fast, filters…)
      # are refused next to --no-run. Crate names hold no blank: a word list is enough.
      sel=
      prev=
      for a in "$@"; do
        case $prev in -p | --package) sel="$sel $prev $a" ;; esac
        case $a in -p?* | --package=*) sel="$sel $a" ;; esac
        prev=$a
      done
      # Build the test binaries before taking the PostgreSQL lock: another worktree only
      # waits for our test run, not for our compilation.
      # shellcheck disable=SC2086 # $ws and $sel: word lists
      if ! k-heavy cargo nextest run --no-run $ws --all-features $sel >"$log/build" 2>&1; then
        echo "KO build ($log/build)"; grep -E '^error|^ *--> ' "$log/build" | head -40; exit 1
      fi
      # Stale test databases and templates (ADR 0028), dropped under the lock: no other
      # suite is running then.
      if [ -d crates/kartenn-testing ]; then
        k-heavy cargo build -q -p kartenn-testing --bin kartenn-testing-clean >>"$log/build" 2>&1 &&
          k-heavy --pg cargo run -q -p kartenn-testing --bin kartenn-testing-clean >"$log/clean" 2>&1 ||
          echo "k-check: nettoyage des bases de test en échec ($log/clean)" >&2
      fi
      if ! k-heavy --pg cargo nextest run $ws --all-features --hide-progress-bar \
          --status-level fail --final-status-level fail --failure-output final "$@" >"$log/test" 2>&1; then
        echo "KO tests ($log/test)"
        grep -E '^ +(FAIL|SIGSEGV|TIMEOUT|LEAK)|^error' "$log/test" | sort -u | head -20
        echo "---"; grep -E 'panicked at|assertion|left:|right:|^error(\[|:)| --> ' "$log/test" | head -40
        exit 1
      fi
      # Doctests never touch the database: no PostgreSQL lock.
      if [ $# -eq 0 ] && ! k-heavy cargo test -q --workspace --doc --all-features >"$log/doc" 2>&1; then
        echo "KO doctests ($log/doc)"; grep -E 'FAILED|panicked|error' "$log/doc" | head -30; exit 1
      fi
      echo "OK — $(grep -E '^ +Summary' "$log/test" | tail -1 | sed 's/^ *//')$warnings"
    '';
  };

  # Same for the frontend (ADR 0024): `k-web-check [vitest args…]`, in ${WEB_DIR:-web}.
  # Convention: each workspace package defines a `typecheck` script (vue-tsc); vitest
  # runs from the workspace root (projects).
  scripts.k-web-check = {
    description = "prettier, eslint, stylelint and captures (web/v2), vue-tsc and vitest, printing a one-line verdict or the failures only";
    exec = ''
      set -uo pipefail
      export NO_COLOR=1 FORCE_COLOR=0
      [ -n "''${K_HEAVY_HELD:-}" ] || exec k-heavy ${dash} "$0" "$@"
      dir=''${WEB_DIR:-web}
      [ -f "$dir/package.json" ] || { echo "OK — pas de frontend ($dir/package.json absent)"; exit 0; }
      cd "$dir" || exit 1
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
      # Style lint where the workspace has one (web/v2, spec 020, REQ-020-13).
      if [ -f stylelint.config.js ]; then
        pnpm exec stylelint --allow-empty-input '**/*.{css,vue}' >"$log/stylelint" 2>&1 ||
          ko stylelint "$log/stylelint" '^[^ ]|^ +[0-9]+:[0-9]+ +✖'
      fi
      # Package by package, so that error paths are prefixed with the package directory.
      tsc=0
      pnpm -r ls --depth -1 --parseable >"$log/packages"
      while read -r pkg; do
        rel=''${pkg#"$PWD"}; rel=''${rel#/}
        (cd "$pkg" && pnpm run --if-present --silent typecheck) 2>&1 |
          sed -E "s#^([^ ][^(]*\([0-9]+,[0-9]+\): error)#''${rel:+$rel/}\1#" >>"$log/tsc" || tsc=1
      done <"$log/packages"
      [ "$tsc" -eq 0 ] || ko vue-tsc "$log/tsc" 'error TS[0-9]+'
      pnpm exec vitest run --reporter=dot "$@" >"$log/test" 2>&1 ||
        ko vitest "$log/test" '^ *(FAIL|×)|AssertionError|Expected|Received|Error:|❯ .*:[0-9]+:[0-9]+'
      tests=$(grep -E '^ *Tests ' "$log/test" | tail -1 | sed 's/^ *//')
      # Captures of the catalogue where the workspace has them (web/v2, spec 020, REQ-020-26):
      # the Playwright project `visual` against `vite preview` (127.0.0.1:4173).
      if [ -d e2e/visual ]; then
        pnpm -C e2e exec playwright test --project visual --reporter=line >"$log/visual" 2>&1 ||
          ko visual "$log/visual" '^ *[0-9]+\) |Error:|pixels|^ *at .*:[0-9]+:[0-9]+' "k-web-shots --update, puis relire les images"
        tests="$tests ; captures $(grep -E '^ *[0-9]+ passed' "$log/visual" | tail -1 | sed 's/^ *//')"
      fi
      echo "OK — $tests"
    '';
  };
  # The new shell (spec 021, « Migration »): the same checks on its own workspace.
  scripts.k-web2-check = {
    description = "k-web-check on the workspace of the new shell (web/v2)";
    exec = ''WEB_DIR=web/v2 exec k-web-check "$@"'';
  };
  scripts.k-web2-e2e = {
    description = "k-web-e2e on the workspace of the new shell (web/v2)";
    exec = ''WEB_DIR=web/v2 exec k-web-e2e "$@"'';
  };
  # Reference captures of the design system (spec 020, REQ-020-26): `k-web-shots [--update]
  # [playwright args…]`, in ${WEB_DIR:-web/v2}. Only in devenv: the references depend on
  # the browser and the fonts of Nix, and a capture taken elsewhere would not compare.
  scripts.k-web-shots = {
    description = "Compare (or with --update, regenerate) the reference captures of the catalogue";
    exec = ''
      set -uo pipefail
      export NO_COLOR=1 FORCE_COLOR=0
      if [ -z "''${DEVENV_ROOT:-}" ] || [ -z "''${PLAYWRIGHT_BROWSERS_PATH:-}" ]; then
        echo "k-web-shots : refusé hors de devenv shell (références prises avec le navigateur et les polices de Nix)" >&2
        exit 2
      fi
      [ -n "''${K_HEAVY_HELD:-}" ] || exec k-heavy ${dash} "$0" "$@"
      update=""
      if [ "''${1:-}" = --update ]; then update=--update-snapshots; shift; fi
      dir=''${WEB_DIR:-web/v2}
      [ -d "$dir/e2e/visual" ] || { echo "KO — pas de captures ($dir/e2e/visual absent)"; exit 1; }
      log=$(mktemp -t k-web-shots.XXXXXX)
      if ! pnpm -C "$dir/e2e" exec playwright test --project visual --reporter=line $update "$@" >"$log" 2>&1; then
        echo "KO visual ($log)"
        grep -E '^ *[0-9]+\) |Error:|pixels' "$log" | head -30
        exit 1
      fi
      echo "OK — $(grep -E '^ *[0-9]+ passed' "$log" | tail -1 | sed 's/^ *//')''${update:+ ; références réécrites : git status $dir/e2e/visual, relire les images avant commit}"
    '';
  };

  # Same for the Python clients (spec 023, T25), in ${PY_DIR:-clients/python}: ruff (lint
  # and format) and pyright on the uv workspace, then pytest with branch coverage in each
  # package. pytest runs as many workers as k-heavy gives cores to a slot.
  scripts.k-py-check = {
    description = "ruff, pyright and pytest of the Python clients, printing a one-line verdict or the failures only";
    exec = ''
      set -uo pipefail
      # FORCE_COLOR stays unset: ruff takes any value of it as a request for colours.
      export NO_COLOR=1 PY_COLORS=0
      unset FORCE_COLOR
      [ $# -eq 0 ] || { echo "usage: k-py-check (sans argument)" >&2; exit 2; }
      [ -n "''${K_HEAVY_HELD:-}" ] || exec k-heavy ${dash} "$0"
      dir=''${PY_DIR:-clients/python}
      [ -f "$dir/pyproject.toml" ] || { echo "OK — pas de client Python ($dir/pyproject.toml absent)"; exit 0; }
      cd "$dir" || exit 1
      log=$(mktemp -d -t k-py-check.XXXXXX)
      # ko <step> <log> <pattern> [hint]: the matching lines, else the end of the log.
      ko() {
        echo "KO $1 ($2)''${4:+ — $4}"
        n=$(grep -cE "$3" "$2")
        if [ "$n" -gt 0 ]; then grep -E "$3" "$2" | head -20 | sed "s#$PWD/##"; [ "$n" -gt 20 ] && echo "… $((n - 20)) de plus"; else tail -15 "$2"; fi
        exit 1
      }
      # The environment of the lock, exactly: a stale lock fails here, not in a test.
      uv sync --locked --all-packages >"$log/uv-sync" 2>&1 ||
        ko uv-sync "$log/uv-sync" '^error|^ *×' "uv lock dans $dir"
      ruff check --output-format=concise . >"$log/ruff" 2>&1 ||
        ko ruff "$log/ruff" ':[0-9]+:[0-9]+: ' "ruff check --fix $dir"
      ruff format --check --output-format=concise . >"$log/ruff-format" 2>&1 ||
        ko ruff-format "$log/ruff-format" ': unformatted: ' "ruff format $dir"
      uv run --no-sync pyright >"$log/pyright" 2>&1 ||
        ko pyright "$log/pyright" ' - error: '
      verdict=
      for pkg in kartenn kartenn-cli; do
        (cd "$pkg" && uv run --no-sync pytest -n "''${NEXTEST_TEST_THREADS:-auto}" -p no:sugar -q) \
          >"$log/pytest-$pkg" 2>&1 ||
          ko pytest "$log/pytest-$pkg" '^(FAILED|ERROR|INTERNALERROR) |Required test coverage .* not reached'
        verdict="$verdict''${verdict:+, }$pkg $(grep -oE '[0-9]+ passed' "$log/pytest-$pkg" | tail -1 | cut -d' ' -f1) tests"
      done
      echo "OK — $verdict"
    '';
  };

  # End-to-end sessions of the CLI (spec 023, task 26): `k-py-e2e [pytest args…]`.
  # web/e2e/serve.sh cli-prepare builds kartenn-server (with and without `policy`) and
  # kartenn-dev-idp and recreates kartenn_cli and kartenn_cli_labo; the pytest session of
  # clients/python/e2e starts the issuer and both instances, then replays the maquette.
  # Same locks as k-web-e2e: the fixed loopback ports and web/e2e/.state are shared.
  scripts.k-py-e2e = {
    description = "End-to-end sessions of the Python CLI against kartenn-server, printing a one-line verdict or the failures only";
    exec = ''
      set -uo pipefail
      export NO_COLOR=1 PY_COLORS=0
      unset FORCE_COLOR
      [ -n "''${K_PG_HELD:-}" ] || exec k-heavy --pg --all ${dash} "$0" "$@"
      root=$(git rev-parse --show-toplevel)
      cd "$root" || exit 1
      log=$(mktemp -d -t k-py-e2e.XXXXXX)
      ${dash} web/e2e/serve.sh cli-prepare >"$log/prepare" 2>&1 ||
        { echo "KO serve.sh cli-prepare ($log/prepare)"; tail -15 "$log/prepare"; exit 1; }
      (cd clients/python && uv sync --locked --all-packages) >"$log/uv-sync" 2>&1 ||
        { echo "KO uv-sync ($log/uv-sync)"; tail -15 "$log/uv-sync"; exit 1; }
      if ! (cd clients/python && uv run --no-sync pytest -c e2e/pytest.toml --rootdir=e2e -p no:sugar e2e "$@") \
        >"$log/e2e" 2>&1; then
        echo "KO pytest ($log/e2e)"
        re='^(FAILED|ERROR|INTERNALERROR) |^E  '
        if grep -qE "$re" "$log/e2e"; then grep -E "$re" "$log/e2e" | head -30; else tail -15 "$log/e2e"; fi
        exit 1
      fi
      echo "OK — $(grep -E '[0-9]+ passed' "$log/e2e" | tail -1 | sed 's/^=* *//; s/ *=*$//')"
    '';
  };

  # End-to-end journeys (Playwright, browsers from Nix): `k-web-e2e [playwright args…]`.
  scripts.k-web-e2e = {
    description = "Playwright end-to-end tests, printing a one-line verdict or the failures only";
    exec = ''
      set -uo pipefail
      export NO_COLOR=1 FORCE_COLOR=0
      # Exclusive: the journeys own the kartenn_e2e database and fixed loopback ports, and
      # take every slot, since a build beside them stretches the browser timeouts.
      [ -n "''${K_PG_HELD:-}" ] || exec k-heavy --pg --all ${dash} "$0" "$@"
      dir=''${WEB_DIR:-web}
      [ -f "$dir/package.json" ] || { echo "OK — pas de frontend ($dir/package.json absent)"; exit 0; }
      cd "$dir" || exit 1
      log=$(mktemp -d -t k-web-e2e.XXXXXX)
      # The journeys and their configuration live in the e2e package (spec 007, task 10).
      if ! pnpm -C e2e exec playwright test --reporter=line "$@" >"$log/e2e" 2>&1; then
        echo "KO playwright ($log/e2e)"
        re='^ *[0-9]+\) |Error:|Expected|Received|^ *at .*:[0-9]+:[0-9]+'
        if grep -qE "$re" "$log/e2e"; then grep -E "$re" "$log/e2e" | head -30 | sed "s#$PWD/##"; else tail -15 "$log/e2e"; fi
        exit 1
      fi
      echo "OK — $(grep -E '^ *[0-9]+ passed' "$log/e2e" | tail -1 | sed 's/^ *//')"
      # A journey that passed only on its retry (CI) is named, never hidden.
      if grep -qE '^ *[0-9]+ flaky' "$log/e2e"; then
        echo "ATTENTION — $(grep -E '^ *[0-9]+ flaky' "$log/e2e" | tail -1 | sed 's/^ *//')"
        grep -A3 -E '^ *[0-9]+ flaky' "$log/e2e" | tail -n +2 | sed "s#$PWD/##"
      fi
    '';
  };

  # Releases (ADR 0033, docs/developpement/publication.md): the logic lives in
  # scripts/release.sh, which the GitHub workflows call too. The tree is the one of the
  # current directory, not $DEVENV_ROOT, which a worktree may inherit from another checkout.
  scripts.k-release-notes = {
    description = "Preview the tag message (release notes) of the next or given version";
    exec = ''exec "$(git rev-parse --show-toplevel)/scripts/release.sh" notes "$@"'';
  };
  scripts.k-release = {
    description = "Release X.Y.Z: version, CHANGELOG.md, commit, signed tag, push after confirmation";
    exec = ''exec "$(git rev-parse --show-toplevel)/scripts/release.sh" prepare "$@"'';
  };
  scripts.k-release-verify = {
    description = "Verify a release tag: signature by a key of .github/release-keys, main, versions";
    exec = ''exec "$(git rev-parse --show-toplevel)/scripts/release.sh" verify "$@"'';
  };
  # Lint of the GitHub workflows (ADR 0033).
  scripts.k-ci-lint = {
    description = "actionlint, zizmor and shellcheck on the GitHub workflows and release scripts";
    exec = ''exec "$(git rev-parse --show-toplevel)/scripts/ci-lint.sh" "$@"'';
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

  # The benches (ADR 0032) recreate the kartenn_bench database of the bench instance
  # (k-pg-bench, port 5433; K_BENCH_DATABASE_URL to aim elsewhere) under `k-heavy --bench`,
  # at the scale K_BENCH_SCALE: reduced by default, `full` only for a budget measure, run
  # once by the orchestrator at the end of a series.
  env.K_BENCH_DATABASE_URL = benchDatabase;

  # Budgets of the properties (spec 003, REQ-003-26) on the reference dataset of
  # REQ-002-45: `bench-properties [seed] [small]`.
  scripts.bench-properties =
    bench "props" "Check the budgets of the properties on a fresh kartenn_bench database";

  # Budgets of the core (REQ-002-45, REQ-004-38) on the same dataset with three years of
  # history: `bench-core [seed] [small]`.
  scripts.bench-core =
    bench "core" "Check the budgets of the core on a fresh kartenn_bench database";

  # Budgets of the flow policy module (REQ-010-28, 54) on the dataset of the core, the VLAN
  # and machines modules deployed: `bench-policy [seed] [small]`.
  scripts.bench-policy =
    bench "policy" "Check the budgets of the flow policy on a fresh kartenn_bench database";

  # Budgets of the flow registry (REQ-011-19): the four modules on the dataset of the
  # core, a policy of 10⁴ rules and 10⁵ flows: `bench-flow [seed] [small]`.
  scripts.bench-flow =
    bench "flow" "Check the budgets of the flow registry on a fresh kartenn_bench database";

  # Cost of access control (REQ-005-29) on the budgets of the core, for a person with 50
  # groups and 1 000 bindings: `bench-rbac [seed] [small]`.
  scripts.bench-rbac =
    bench "rbac" "Measure the cost of access control on a fresh kartenn_bench database";

  # The budgets of the core through HTTP, then 200 users and 50 writes per second
  # (REQ-006-39): `bench-http [seed] [small]`.
  scripts.bench-http =
    bench "http" "Measure the budgets of the core through HTTP on a fresh kartenn_bench database";

  # The budgets of the event stream (spec 022, REQ-022-26): latency at 50 changesets per
  # second, memory per stream, a storm of resumptions: `bench-events [seed] [small]`.
  scripts.bench-events =
    bench "events" "Measure the budgets of the event stream on a fresh kartenn_bench database";

  # The budgets of the browser (spec 007, REQ-007-32): the size of the initial shell, then
  # the Playwright project `budgets` against kartenn-server in release on the dataset of
  # bench-core (web/e2e/serve.sh budgets): `bench-web [seed] [small]`. It also takes the
  # PostgreSQL lock: the journeys of k-web-e2e use the same fixed ports and web/e2e/.state.
  scripts.bench-web = lib.mkIf hasWeb {
    description = "Check the budgets of the browser on a fresh kartenn_bench database";
    exec = ''
      set -euo pipefail
      [ -n "''${K_BENCH_HELD:-}" ] && [ -n "''${K_PG_HELD:-}" ] || exec k-heavy --bench --pg ${dash} "$0" "$@"
      k-pg-bench
      export NO_COLOR=1 FORCE_COLOR=0 BENCH_SEED=''${1:-42}
      [ "''${2:-}" != small ] || export BENCH_SMALL=1
      pnpm -C web --filter @kartenn/shell build >/dev/null
      node web/shell/scripts/size.mjs
      pnpm -C web/e2e exec playwright test -c budgets.config.ts --reporter=line
    '';
  };

  # Cost of the telemetry (REQ-012-23) on the budgets of the core, and volume of the
  # audit of reads (REQ-012-18): `bench-telemetry [seed] [small]`.
  scripts.bench-telemetry =
    bench "telemetry" "Measure the cost of the telemetry on a fresh kartenn_bench database";

  # The budgets of the CLI (spec 023, REQ-023-39): `bench-cli [runs]`. The client alone,
  # against a stub of the contract on the loopback (clients/python/bench/cli.py): no
  # PostgreSQL, but the bench lock all the same, so that timings never share the machine
  # with another bench.
  scripts.bench-cli = {
    description = "Check the budgets of the Python CLI (start, help, completion, first contact)";
    exec = ''
      set -euo pipefail
      [ -n "''${K_BENCH_HELD:-}" ] || exec k-heavy --bench ${dash} "$0" "$@"
      cd "$(git rev-parse --show-toplevel)/clients/python"
      uv sync --locked --all-packages -q
      exec uv run --no-sync python bench/cli.py "$@"
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

  # The git hooks are shared by every worktree of the repository (they live in the common
  # git directory), but devenv writes them with the absolute configuration path of the
  # worktree that entered its shell last: commits elsewhere then ran that worktree's
  # configuration, and failed once it was removed. Installed here instead, they read the
  # configuration of the worktree being committed.
  git-hooks.install.enable = false;
  enterShell = let
    hook = stage:
      pkgs.writers.writeDash "kartenn-${stage}" ''
        # File generated by prek: https://github.com/j178/prek
        # ID: 182c10f181da4464a3eec51b83331688
        # (prek's marker above, without which hook-impl refuses to run.) Installed by
        # devenv.nix (kartenn): the configuration of the current worktree.
        top=$(git rev-parse --show-toplevel) || exit 1
        config="$top/.pre-commit-config.yaml"
        if [ ! -e "$config" ]; then
          common=$(git rev-parse --path-format=absolute --git-common-dir) || exit 1
          config="''${common%/.git}/.pre-commit-config.yaml"
          echo "hook ${stage} : pas de configuration dans $top (lancer devenv shell) ; celle de $config" >&2
        fi
        PREK=${lib.getExe config.git-hooks.package}
        [ -x "$PREK" ] || PREK=prek
        exec "$PREK" hook-impl --hook-dir "$(cd "$(dirname "$0")" && pwd)" --script-version 4 \
          --hook-type=${stage} --config="$config" -- "$@"
      '';
  in ''
    hooks=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)/hooks
    if [ -d "$hooks" ]; then
      for pair in pre-commit:${hook "pre-commit"} pre-push:${hook "pre-push"}; do
        stage=''${pair%%:*} source=''${pair#*:}
        cmp -s "$source" "$hooks/$stage" || install -m 755 "$source" "$hooks/$stage"
        # A worktree on an older devenv.nix lets prek reinstall its own hook, which moves
        # this one to <stage>.legacy; prek then refuses to run ("migration mode").
        if [ -f "$hooks/$stage.legacy" ] && grep -qe 'File generated by prek' -e 'Installed by devenv.nix (kartenn)' "$hooks/$stage.legacy"; then
          rm -f "$hooks/$stage.legacy"
        fi
      done
    fi
    # The new shell (spec 021, « Migration »): its own pnpm workspace beside web/.
    if [ -f web/v2/package.json ]; then
      pnpm -C web/v2 install --frozen-lockfile --reporter=silent || echo "pnpm install de web/v2 en échec" >&2
    fi
  '';

  git-hooks.hooks = {
    # Generic hygiene, every project.
    check-merge-conflicts.enable = true;
    end-of-file-fixer = {
      enable = true;
      # devenv writes its lock without a final newline, on every update; Playwright its
      # ARIA snapshots (spec 020, k-web-shots --update) the same way.
      excludes = ["^devenv\\.lock$" "/__screenshots__/"];
    };
    trim-trailing-whitespace = {
      enable = true;
      # syrupy snapshots (spec 023) keep the output as printed, indented blank lines too;
      # so do the ARIA snapshots of Playwright (spec 020).
      excludes = ["\\.ambr$" "/__screenshots__/"];
    };
    rustfmt.enable = true;
    # Under the machine-wide semaphore (kHeavy): the hook runs after every edit.
    clippy = {
      enable = true;
      # Absolute toolchain: pre-commit also runs outside the devenv shell.
      entry = lib.mkForce "${clippyHook}";
    };
    # GitHub workflows and scripts (ADR 0033), with absolute tools: pre-commit also runs
    # outside the devenv shell. zizmor offline here; `GH_TOKEN=… k-ci-lint` online.
    ci-lint = {
      enable = true;
      name = "actionlint, zizmor, shellcheck";
      entry = "${pkgs.writers.writeDash "ci-lint" ''
        export PATH=${lib.makeBinPath [pkgs.actionlint pkgs.zizmor pkgs.shellcheck pkgs.dash]}:$PATH
        export K_DASH_SCRIPTS=${lib.escapeShellArg (toString lintedScripts)}
        unset GH_TOKEN
        exec scripts/ci-lint.sh
      ''}";
      files = "^(\\.github/.*\\.ya?ml|scripts/.*\\.sh|web/e2e/serve\\.sh|\\.claude/.*\\.sh|devenv\\.nix|nix/dash-scripts\\.nix)$";
      pass_filenames = false;
    };
    # Frontend, with web/'s own tool versions; no-ops until web/ exists.
    web-prettier = {
      enable = true;
      name = "prettier (web)";
      entry = "${webHook "prettier"} --write --ignore-unknown";
      files = "^web/.*\\.(vue|ts|tsx|js|mjs|json|css|html|md)$";
      excludes = ["(^|/)pnpm-lock\\.yaml$" "^web/v2/"];
    };
    web-eslint = {
      enable = true;
      name = "eslint (web)";
      entry = "${webHook "eslint"} --fix";
      files = "^web/.*\\.(vue|ts|tsx|js|mjs)$";
      excludes = ["^web/v2/"];
    };
    # The new shell, with the tools of its own workspace (spec 021, « Migration »).
    web2-prettier = {
      enable = true;
      name = "prettier (web/v2)";
      entry = "${web2Hook "prettier"} --write --ignore-unknown";
      files = "^web/v2/.*\\.(vue|ts|tsx|js|mjs|json|css|html|md)$";
      excludes = ["(^|/)pnpm-lock\\.yaml$"];
    };
    web2-eslint = {
      enable = true;
      name = "eslint (web/v2)";
      entry = "${web2Hook "eslint"} --fix";
      files = "^web/v2/.*\\.(vue|ts|tsx|js|mjs)$";
    };
    web2-stylelint = {
      enable = true;
      name = "stylelint (web/v2)";
      entry = "${web2Hook "stylelint"} --fix --allow-empty-input";
      files = "^web/v2/.*\\.(vue|css)$";
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
    permissions.rules.Bash.allow = ["k-web-check:*" "k-web-e2e:*" "k-web2-check:*" "k-web2-e2e:*" "k-web-shots:*"];
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
    # Interface quality (ADR 0039, § 4): Prettier, Stylelint and ESLint on the one web/**
    # file just edited; diagnostics go back to the agent.
    hooks.web-apres-edition = {
      name = "Format and lint the edited web file";
      hookType = "PostToolUse";
      matcher = "^(Edit|MultiEdit|Write)$";
      command = ''"$CLAUDE_PROJECT_DIR"/.claude/hooks/web-apres-edition.sh'';
    };
    hooks.reprise-froide = {
      name = "Stop once before rewriting an expired cache";
      hookType = "UserPromptSubmit";
      command = ''"$CLAUDE_PROJECT_DIR"/.claude/hooks/reprise-froide.sh'';
    };
  };
}
