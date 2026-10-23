# The devenv scripts run under the dash of Nix (>= 0.5.13: `set -o pipefail`), not bash,
# unless a script sets its own `package`. devenv writes `exec` to a file without a shebang
# and runs it as `<package>/bin/<binary> <file>`, so `$0` is that file: a script that
# re-executes itself names the same dash (`exec k-heavy ${dash} "$0" "$@"`).
{
  lib,
  pkgs,
  ...
}: {
  options.scripts = lib.mkOption {
    type = lib.types.attrsOf (lib.types.submodule {
      config.package = lib.mkDefault pkgs.dash;
    });
  };
}
