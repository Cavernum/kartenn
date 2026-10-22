#!/usr/bin/env bash
# Servers of the end-to-end journeys (spec 007, REQ-007-34), started by Playwright
# (`webServer`, in this order, before global-setup.ts):
# - `serve.sh idp`: builds the binaries, writes the material of web/e2e/.state (authority,
#   loopback certificate, client secret, key ring), then serves the issuer on 9444;
# - `serve.sh kartenn`: builds the shell and the plugins of web/plugins, gathers them under
#   web/e2e/.state/plugins, recreates the kartenn_e2e database, migrates it,
#   creates the break-glass account `e2e` (its proofs in web/e2e/.state/breakglass.txt),
#   which the subcommand binds to `administrator` (K17), then serves kartenn-server on 9443.
# The break-glass account is the only write outside the API, as for an operator bootstrapping
# an instance (REQ-005-14). Everything else goes through global-setup.ts.
set -euo pipefail
cd "$(dirname "$0")/../.."
state=web/e2e/.state
bin=${CARGO_TARGET_DIR:-target}/debug

case ${1:-} in
idp)
  cargo build -q -p kartenn-dev-idp -p kartenn-server
  mkdir -p "$state"
  "$bin/kartenn-dev-idp" certs "$state"
  exec "$bin/kartenn-dev-idp" serve web/e2e/users.toml
  ;;
kartenn)
  pnpm -C web --filter @kartenn/shell build >/dev/null
  # The plugins directory of the instance: the test plugins, the example plugin and the
  # plugins of the VLAN and machines modules, built as an operator would install them
  # (web/dist/plugins).
  pnpm -C web --filter @kartenn/plugin-example --filter @kartenn/plugin-vlan \
    --filter @kartenn/plugin-machines build >/dev/null
  rm -rf "$state/plugins"
  mkdir -p "$state/plugins"
  cp -R web/e2e/plugins/. web/dist/plugins/. "$state/plugins/"
  database="${DATABASE_URL%/*}/kartenn_e2e"
  psql "$DATABASE_URL" -q -c 'DROP DATABASE IF EXISTS kartenn_e2e WITH (FORCE)' \
    -c 'CREATE DATABASE kartenn_e2e'
  "$bin/kartenn-server" migrate "$database" >/dev/null
  "$bin/kartenn-server" breakglass create "$database" e2e 24 "$state/kartenn-keys" \
    >"$state/breakglass.txt"
  exec env SSL_CERT_FILE="$state/ca.pem" \
    "$bin/kartenn-server" run "$database" web/e2e/kartenn.toml
  ;;
budgets)
  # The instance of the browser bench (REQ-007-32, `bench-web`): kartenn-server in release
  # on the kartenn_bench database, migrated then loaded by `kartenn-bench seed` (the
  # dataset of bench-core and a container of 10⁵ networks, whose ids go to
  # .state/budgets.env), with the example plugin and a plugin that loads too slowly; the
  # break-glass account `e2e` as above, for budgets/global-setup.ts.
  release=${CARGO_TARGET_DIR:-target}/release
  cargo build -q --release -p kartenn-server -p kartenn-bench
  pnpm -C web --filter @kartenn/shell --filter @kartenn/plugin-example build >/dev/null
  rm -rf "$state/plugins"
  mkdir -p "$state/plugins"
  cp -R web/e2e/budgets/plugins/. web/dist/plugins/example "$state/plugins/"
  # The bench instance of `k-pg-bench` (ADR 0032), started by `bench-web`.
  database=${K_BENCH_DATABASE_URL:-postgres://kartenn@127.0.0.1:5433/kartenn_bench}
  psql "${database%/*}/postgres" -q -c "DROP DATABASE IF EXISTS \"${database##*/}\" WITH (FORCE)" \
    -c "CREATE DATABASE \"${database##*/}\""
  "$release/kartenn-server" migrate "$database" >/dev/null
  "$release/kartenn-bench" seed "$database" "${BENCH_SEED:-42}" ${BENCH_SMALL:+small} \
    >"$state/budgets.env"
  "$release/kartenn-server" breakglass create "$database" e2e 24 "$state/kartenn-keys" \
    >"$state/breakglass.txt"
  exec env SSL_CERT_FILE="$state/ca.pem" \
    "$release/kartenn-server" run "$database" web/e2e/kartenn.toml
  ;;
*)
  echo "usage: serve.sh idp|kartenn|budgets" >&2
  exit 2
  ;;
esac
