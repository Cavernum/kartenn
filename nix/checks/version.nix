# One version for the release (spec 015, REQ-015-12): the packages of the flake carry the
# version of Cargo (server) and of the web packages (shell), and both agree. Cargo and the
# web packages are compared with each other by kartenn-server/tests/it/version.rs.
{
  runCommand,
  kartenn,
  kartenn-server,
  kartenn-web,
}: let
  versions = {
    kartenn = kartenn.version;
    kartenn-server = kartenn-server.version;
    kartenn-web = kartenn-web.version;
  };
  distinct = builtins.attrNames (builtins.listToAttrs (map (v: {
    name = v;
    value = null;
  }) (builtins.attrValues versions)));
  describe = builtins.concatStringsSep ", " (map (n: "${n} ${versions.${n}}") (builtins.attrNames versions));
in
  runCommand "kartenn-version" {} (
    if builtins.length distinct == 1
    then "touch $out"
    else ''
      echo "the packages of the flake disagree on the version: ${describe}" >&2
      exit 1
    ''
  )
