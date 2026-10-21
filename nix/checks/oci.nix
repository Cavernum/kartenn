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
  for layer in $(jq -r '.[0].Layers[]' manifest.json); do
    tar -tf "$layer" >> files
  done
  if grep -E '(^|/)bin/(sh|bash|dash|ash|zsh|busybox)$' files; then
    fail "a shell is in the image"
  fi
  grep -q 'etc/ssl/certs/ca-bundle.crt$' files || fail "no root certificates"
  grep -q '/share/kartenn/web/shell/index.html$' files || fail "no web shell"
  grep -qE '/share/kartenn/plugins/(vlan|machines)/' files || fail "no first-party plugin"

  echo "checks.oci: $(wc -l < files) entries, $(du -k ${oci} | cut -f1) KiB compressed" > $out
''
