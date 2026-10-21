# The configuration and content of the OCI image (spec 015, REQ-015-3), read without
# running it: non-root user, entrypoint and command, no shell anywhere in the layers, the
# root certificates and the shell present.
{
  runCommand,
  jq,
  gnutar,
  oci,
}:
runCommand "kartenn-oci-check" {nativeBuildInputs = [jq gnutar];} ''
  set -euo pipefail
  work=$(mktemp -d)
  tar -xf ${oci} -C "$work"
  cd "$work"
  config=$(jq -r '.[0].Config' manifest.json)
  fail() { echo "checks.oci: $*" >&2; exit 1; }

  [ "$(jq -r .config.User "$config")" = "65532:65532" ] || fail "User is not 65532:65532"
  jq -e '.config.Entrypoint | length == 1 and (.[0] | endswith("/bin/kartenn-server"))' "$config" >/dev/null ||
    fail "Entrypoint is not kartenn-server"
  jq -e '.config.Cmd == ["run"]' "$config" >/dev/null || fail 'Cmd is not ["run"]'
  jq -e '.config.Env | any(startswith("SSL_CERT_FILE="))' "$config" >/dev/null || fail "SSL_CERT_FILE unset"

  : > files
  : > listing
  for layer in $(jq -r '.[0].Layers[]' manifest.json); do
    tar -tf "$layer" >> files
    tar -tvf "$layer" >> listing
  done
  # The plugin registry refuses symbolic links and the shell serves only files under its
  # canonical root: the shell and the plugins are real files.
  if grep -E '^l.*/share/kartenn/(web|plugins)/.* -> ' listing; then
    fail "symbolic links in the shell or the plugins"
  fi
  if grep -E '(^|/)bin/(sh|bash|dash|ash|zsh|busybox)$' files; then
    fail "a shell is in the image"
  fi
  # `/opt/kartenn/{shell,plugins}` are the paths of the configuration (docs/exploitation/
  # oci.md): links into the store, followed by the server (canonical shell root, plugin
  # directory read through), whose target must lie in the layers with real content.
  target() {
    sed -n "s#^l.* \(\./\)\?opt/kartenn/$1 -> /\(.*\)\$#\2#p" listing | head -n 1
  }
  shell=$(target shell)
  plugins=$(target plugins)
  [ -n "$shell" ] || fail "no /opt/kartenn/shell"
  [ -n "$plugins" ] || fail "no /opt/kartenn/plugins"
  # Store paths are listed absolute, the files of extraCommands relative (`./`).
  in_layers() { grep -qxF "/$1" files || grep -qxF "$1" files; }
  in_layers "$shell/index.html" || fail "/opt/kartenn/shell leads to no index.html"
  grep -F "$plugins/" files | grep -qE '/(vlan|machines)/[^/]+/manifest\.json$' ||
    fail "/opt/kartenn/plugins leads to no first-party plugin"

  grep -q 'etc/ssl/certs/ca-bundle.crt$' files || fail "no root certificates"
  grep -q '/share/kartenn/web/shell/index.html$' files || fail "no web shell"
  grep -qE '/share/kartenn/plugins/(vlan|machines)/' files || fail "no first-party plugin"

  echo "checks.oci: $(wc -l < files) entries, $(du -k ${oci} | cut -f1) KiB compressed" > $out
''
