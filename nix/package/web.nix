# The web shell and the first-party plugins (spec 015, REQ-015-1; ADR 0025): the shell
# under share/kartenn/web/shell, each plugin under share/kartenn/plugins/<name>/<version>,
# the layout `[web] root` and `[web] plugins_dir` expect. The example plugin stays out.
# The dependencies come from web/pnpm-lock.yaml; after a change of that file,
# scripts/update-pnpm-hash.sh recomputes `hash`.
{
  lib,
  stdenvNoCC,
  nodejs_24,
  pnpm_12,
  fetchPnpmDeps,
  pnpmConfigHook,
}: let
  root = ../../web;
  shell = lib.importJSON (root + "/shell/package.json");
in
  stdenvNoCC.mkDerivation (finalAttrs: {
    pname = "kartenn-web";
    inherit (shell) version;
    src = lib.fileset.toSource {
      inherit root;
      # The sources only: never a local node_modules, build or end-to-end state.
      fileset = lib.fileset.difference root (lib.fileset.unions (map lib.fileset.maybeMissing [
        (root + "/node_modules")
        (root + "/dist")
        (root + "/e2e/.state")
        (root + "/e2e/test-results")
      ]));
    };

    pnpmDeps = fetchPnpmDeps {
      inherit (finalAttrs) pname version src;
      pnpm = pnpm_12;
      fetcherVersion = 4;
      hash = "sha256-uCHUfcNoeIUwls5ZS5WGHrrfIOxXbaFFnlusRPSFl3E=";
    };

    nativeBuildInputs = [nodejs_24 pnpm_12 pnpmConfigHook];

    env.PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";

    buildPhase = ''
      runHook preBuild
      pnpm --filter @kartenn/shell --filter @kartenn/plugin-vlan \
        --filter @kartenn/plugin-machines run build
      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall
      mkdir -p $out/share/kartenn/web $out/share/kartenn/plugins
      cp -R dist/shell $out/share/kartenn/web/shell
      # The build manifest serves the size bench (bench-web) only.
      rm -rf $out/share/kartenn/web/shell/.vite
      cp -R dist/plugins/vlan dist/plugins/machines $out/share/kartenn/plugins/
      runHook postInstall
    '';

    meta = {
      description = "kartenn web shell and first-party plugins";
      license = with lib.licenses; [agpl3Only mpl20];
      platforms = lib.platforms.all;
    };
  })
