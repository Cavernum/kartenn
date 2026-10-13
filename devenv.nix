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
    initialDatabases = [{name = "kartenn";}];
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
      cargo test --workspace --all-targets
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
