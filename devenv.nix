{
  pkgs,
  config,
  ...
}: {
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
