# The Python clients (spec 023, REQ-023-42): the library `kartenn` (MPL-2.0) and the CLI
# `kartenn-cli` (AGPL-3.0), built from clients/python with the interpreter of nixpkgs.
# Their dependencies come from nixpkgs, except those whose version there is below the
# bound of the pyproject (httpx2, its httpcore2, click): these are the pure wheels of
# clients/python/uv.lock, URL and hash read from the lock, so that one file pins them.
# The packages carry their own versions (pyproject.toml), released apart from the server.
# The CLI installs its completion scripts and a manual page written by help2man.
{
  lib,
  python3,
  fetchurl,
  installShellFiles,
  help2man,
}: let
  root = ../..;
  lock = lib.importTOML (root + "/clients/python/uv.lock");
  locked = name: lib.findFirst (p: p.name == name) (throw "${name} is not in uv.lock") lock.package;
  # The pure wheel of a locked package.
  wheel = ps: name: dependencies: let
    p = locked name;
    w = lib.findFirst (w: lib.hasSuffix "-py3-none-any.whl" w.url) (throw "no pure wheel for ${name}") p.wheels;
  in
    ps.buildPythonPackage {
      pname = name;
      inherit (p) version;
      format = "wheel";
      src = fetchurl {
        inherit (w) url;
        hash = w.hash;
      };
      inherit dependencies;
    };
  # Not an override of the package set: click there would rebuild every package that
  # depends on it. Only rich-click, the one dependency of the CLI on click, takes it.
  ps = python3.pkgs;
  click = wheel ps "click" [];
  rich-click = ps.rich-click.override {inherit click;};
  httpcore2 = wheel ps "httpcore2" [ps.h11 ps.truststore];
  httpx2 = wheel ps "httpx2" [ps.anyio httpcore2 ps.idna ps.truststore ps.typing-extensions];
  project = dir: lib.importTOML (root + "/clients/python/${dir}/pyproject.toml");
  source = dir:
    lib.fileset.toSource {
      root = root + "/clients/python/${dir}";
      fileset = lib.fileset.unions [
        (root + "/clients/python/${dir}/pyproject.toml")
        (root + "/clients/python/${dir}/README.md")
        (root + "/clients/python/${dir}/LICENSE")
        (root + "/clients/python/${dir}/src")
      ];
    };
  # uv_build of nixpkgs may lag behind the bound of the pyproject: the backend is the
  # same, the bound is relaxed here only.
  relaxBackend = ''
    sed -i 's/"uv_build>=[^"]*"/"uv_build"/' pyproject.toml
  '';
  kartenn = ps.buildPythonPackage {
    pname = "kartenn";
    inherit ((project "kartenn").project) version;
    pyproject = true;
    src = source "kartenn";
    postPatch = relaxBackend;
    build-system = [ps.uv-build];
    dependencies = [httpx2 ps.platformdirs];
    # The tests run under k-py-check (uv lock, coverage), not in the sandbox.
    doCheck = false;
    pythonImportsCheck = ["kartenn"];
    meta = {
      description = (project "kartenn").project.description;
      license = lib.licenses.mpl20;
      platforms = lib.platforms.unix;
    };
  };
  kartenn-cli = ps.buildPythonApplication {
    pname = "kartenn-cli";
    inherit ((project "kartenn-cli").project) version;
    pyproject = true;
    src = source "kartenn-cli";
    postPatch = relaxBackend;
    build-system = [ps.uv-build];
    dependencies = [kartenn click rich-click ps.rich ps.keyring ps.pyyaml ps.jmespath];
    nativeBuildInputs = [installShellFiles help2man];
    doCheck = false;
    pythonImportsCheck = ["kartenn_cli"];
    # Completion scripts and manual page, from the CLI itself: no server, no profile.
    postInstall = ''
      export HOME=$TMPDIR XDG_CONFIG_HOME=$TMPDIR/config XDG_CACHE_HOME=$TMPDIR/cache \
        LC_ALL=C.UTF-8 PYTHONIOENCODING=utf-8
      installShellCompletion --cmd kartenn \
        --bash <($out/bin/kartenn completion bash) \
        --zsh <($out/bin/kartenn completion zsh) \
        --fish <($out/bin/kartenn completion fish)
      help2man --no-info --locale C.UTF-8 --name "client of kartenn, derived from the contract of its API" \
        --version-string "$version" $out/bin/kartenn >kartenn.1
      installManPage kartenn.1
    '';
    passthru = {inherit kartenn;};
    meta = {
      description = (project "kartenn-cli").project.description;
      license = lib.licenses.agpl3Only;
      mainProgram = "kartenn";
      platforms = lib.platforms.unix;
    };
  };
in {
  inherit kartenn kartenn-cli;
}
