# flake.lock and devenv.lock pin the same nixpkgs (spec 015, REQ-015-2): the flake builds
# with the toolchain the development environment tests.
{runCommand}: let
  locked = file: name: (builtins.fromJSON (builtins.readFile file)).nodes.${name}.locked.rev or null;
  pair = name: {
    flake = locked ../../flake.lock name;
    devenv = locked ../../devenv.lock name;
  };
  drift = builtins.filter (p: p.flake != p.devenv) (map (name: pair name // {inherit name;}) ["nixpkgs" "nixpkgs-src"]);
  describe = p: "${p.name}: flake.lock ${toString p.flake}, devenv.lock ${toString p.devenv}";
in
  runCommand "kartenn-lock-sync" {} (
    if drift == []
    then "touch $out"
    else ''
      echo "flake.lock and devenv.lock pin different nixpkgs commits (ADR 0029):" >&2
      ${builtins.concatStringsSep "\n" (map (p: "echo '  ${describe p}' >&2") drift)}
      exit 1
    ''
  )
