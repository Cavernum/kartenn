# The OCI image (spec 015, REQ-015-3; ADR 0029/E4): one image for the three uses, `run`
# (an instance), `migrate` (the migration job) and `breakglass create` (bootstrap). Non-root
# numeric user, no shell nor package manager: only the closure of the package and the root
# certificates. Configuration by a mounted file (`run /etc/kartenn/kartenn.toml`) or by the
# `KARTENN_*` variables (D35, 12-factor).
{
  lib,
  dockerTools,
  cacert,
  kartenn,
}: let
  share = "${kartenn}/share/kartenn";
in
  dockerTools.buildLayeredImage {
    name = "kartenn";
    tag = kartenn.version;
    # The store paths only, no /bin: nothing to run but the entrypoint.
    contents = [];
    # Owned by the image user (65532) without fakeroot: the tar is written by the build
    # user, and nothing outside the store needs root.
    extraCommands = ''
      mkdir -p etc/kartenn opt/kartenn tmp
      chmod 1777 tmp
      ln -s ${share}/web/shell opt/kartenn/shell
      ln -s ${share}/plugins opt/kartenn/plugins
    '';
    config = {
      User = "65532:65532";
      Entrypoint = ["${lib.getExe' kartenn "kartenn-server"}"];
      Cmd = ["run"];
      WorkingDir = "/";
      # No KARTENN_* default here: the variables override the mounted file. The shell and
      # the first-party plugins sit at fixed paths, for `[web] root = "/opt/kartenn/shell"`
      # and `plugins_dir = "/opt/kartenn/plugins"` (docs/exploitation/oci.md).
      Env = ["SSL_CERT_FILE=${cacert}/etc/ssl/certs/ca-bundle.crt"];
      ExposedPorts."8443/tcp" = {};
      Labels = {
        "org.opencontainers.image.title" = "kartenn";
        "org.opencontainers.image.version" = kartenn.version;
        "org.opencontainers.image.licenses" = "AGPL-3.0-only";
      };
    };
  }
