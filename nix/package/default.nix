# The installable package (spec 015, REQ-015-1): the server binary, the built shell under
# share/kartenn/web/shell and the first-party plugins under share/kartenn/plugins.
{
  symlinkJoin,
  kartenn-server,
  kartenn-web,
}:
symlinkJoin {
  pname = "kartenn";
  inherit (kartenn-server) version;
  paths = [kartenn-server kartenn-web];
  meta = kartenn-server.meta // {description = "kartenn: server, web shell and first-party plugins";};
}
