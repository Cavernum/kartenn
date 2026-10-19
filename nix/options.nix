# Project switches that a developer flips in the untracked devenv.local.nix.
{lib, ...}: {
  options.kartenn.claude.openobserveMcp = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = ''
      Declare the OpenObserve MCP server for Claude Code. Turn it off locally when
      the console is not running, so sessions neither try to reach it nor carry it.
    '';
  };
}
