# The installable package (spec 015, REQ-015-1): the server binary, the built shell under
# share/kartenn/web/shell and the first-party plugins under share/kartenn/plugins.
# Real files, not a symlinkJoin: the plugin registry refuses symbolic links, and the
# shell serves only files whose canonical path lies under its root (ADR 0025).
{
  runCommand,
  kartenn-server,
  kartenn-web,
}:
runCommand "kartenn-${kartenn-server.version}" {
  pname = "kartenn";
  inherit (kartenn-server) version;
  passthru = {inherit kartenn-server kartenn-web;};
  meta =
    kartenn-server.meta
    // {
      description = "kartenn: server, web shell and first-party plugins";
    };
} ''
  mkdir -p $out
  cp -r ${kartenn-server}/bin $out/bin
  cp -r ${kartenn-web}/share $out/share
  chmod -R u+w $out
''
