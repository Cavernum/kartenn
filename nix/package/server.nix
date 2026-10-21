# kartenn-server, with the business modules of its default features (vlan, machines),
# built offline from Cargo.lock (spec 015, REQ-015-1). The sqlx macros read the
# versioned .sqlx/ metadata: the build never needs a database.
{
  lib,
  rustPlatform,
}: let
  root = ../..;
  # One source for the version: the package's Cargo.toml, or the workspace's when the
  # package inherits it (`version.workspace = true`).
  manifest = lib.importTOML (root + "/crates/kartenn-server/Cargo.toml");
  workspace = lib.importTOML (root + "/Cargo.toml");
  version =
    if builtins.isAttrs manifest.package.version
    then workspace.workspace.package.version
    else manifest.package.version;
in
  rustPlatform.buildRustPackage {
    pname = "kartenn-server";
    inherit version;
    src = lib.fileset.toSource {
      inherit root;
      fileset = lib.fileset.unions [
        (root + "/Cargo.toml")
        (root + "/Cargo.lock")
        (root + "/crates")
        (root + "/.sqlx")
      ];
    };
    cargoLock.lockFile = root + "/Cargo.lock";
    buildAndTestSubdir = "crates/kartenn-server";
    env.SQLX_OFFLINE = "true";
    # The tests need PostgreSQL and run under k-check, not in the sandbox.
    doCheck = false;

    meta = {
      description = "kartenn server: REST API, web shell and plugins";
      license = lib.licenses.agpl3Only;
      mainProgram = "kartenn-server";
      platforms = lib.platforms.linux;
    };
  }
