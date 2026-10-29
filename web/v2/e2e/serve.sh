#!/usr/bin/env dash
# Servers of the journeys of the new shell (spec 021, REQ-021-31), started by Playwright
# (web/v2/e2e/journeys.config.ts, `webServer`, in this order):
# - `serve.sh idp`: the issuer of the old journeys (web/e2e/serve.sh idp: binaries,
#   material of web/e2e/.state, issuer on 9444);
# - `serve.sh kartenn`: builds the new shell and the plugins of web/v2/plugins (into
#   web/v2/dist), recreates the kartenn_e2e_v2 database, migrates it, creates the
#   break-glass account `e2e` (its proofs in web/e2e/.state/breakglass-v2.txt), then
#   serves kartenn-server on 9443 with web/v2/e2e/kartenn.toml.
# Same ports as web/e2e: k-web-e2e and k-web2-e2e hold every slot of k-heavy, never side
# by side.
set -euo pipefail
cd "$(dirname "$0")/../../.."
state=web/e2e/.state
bin=${CARGO_TARGET_DIR:-target}/debug

case ${1:-} in
idp)
  exec dash web/e2e/serve.sh idp
  ;;
kartenn)
  pnpm -C web/v2 --filter @kartenn/shell --filter "./plugins/*" build >/dev/null
  database="${DATABASE_URL%/*}/kartenn_e2e_v2"
  psql "$DATABASE_URL" -q -c 'DROP DATABASE IF EXISTS kartenn_e2e_v2 WITH (FORCE)' \
    -c 'CREATE DATABASE kartenn_e2e_v2'
  "$bin/kartenn-server" migrate "$database" >/dev/null
  "$bin/kartenn-server" breakglass create "$database" e2e 24 "$state/kartenn-keys" \
    >"$state/breakglass-v2.txt"
  exec env SSL_CERT_FILE="$state/ca.pem" \
    "$bin/kartenn-server" run "$database" web/v2/e2e/kartenn.toml
  ;;
*)
  echo "usage: serve.sh idp|kartenn" >&2
  exit 2
  ;;
esac
