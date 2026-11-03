#!/usr/bin/env dash
# Servers of the journeys of the new shell (spec 021, REQ-021-31), started by Playwright
# (web/v2/e2e/journeys.config.ts, `webServer`, in this order):
# - `serve.sh idp`: the issuer of the old journeys (web/e2e/serve.sh idp: binaries,
#   material of web/e2e/.state, issuer on 9444);
# - `serve.sh kartenn`: builds the new shell and the plugins of web/v2/plugins (into
#   web/v2/dist), gathers them with the test plugins of web/v2/e2e/plugins under
#   web/e2e/.state/plugins-v2, recreates the kartenn_e2e_v2 database, migrates it, creates the
#   break-glass account `e2e` (its proofs in web/e2e/.state/breakglass-v2.txt), then
#   serves kartenn-server on 9443 with web/v2/e2e/kartenn.toml;
# - `serve.sh budgets`: the same instance in release on the dataset of bench-core, for
#   the browser bench (web/v2/e2e/budgets.config.ts, `bench-web`).
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
  # The plugins directory of the instance: the builds of web/v2/plugins and the test
  # plugins of web/v2/e2e/plugins (one of each point, and those set aside), as an operator
  # would install them.
  rm -rf "$state/plugins-v2"
  mkdir -p "$state/plugins-v2"
  cp -R web/v2/e2e/plugins/. web/v2/dist/plugins/. "$state/plugins-v2/"
  database="${DATABASE_URL%/*}/kartenn_e2e_v2"
  psql "$DATABASE_URL" -q -c 'DROP DATABASE IF EXISTS kartenn_e2e_v2 WITH (FORCE)' \
    -c 'CREATE DATABASE kartenn_e2e_v2'
  "$bin/kartenn-server" migrate "$database" >/dev/null
  "$bin/kartenn-server" breakglass create "$database" e2e 24 "$state/kartenn-keys" \
    >"$state/breakglass-v2.txt"
  exec env SSL_CERT_FILE="$state/ca.pem" \
    "$bin/kartenn-server" run "$database" web/v2/e2e/kartenn.toml
  ;;
budgets)
  # The instance of the browser bench (REQ-021-30, `bench-web`): kartenn-server in release
  # on the kartenn_bench database of `k-pg-bench` (ADR 0032), migrated then loaded by
  # `kartenn-bench seed` (the dataset of bench-core and a container of 10⁵ networks, whose
  # ids go to .state/budgets-v2.env), with the plugins of web/v2/plugins and one that
  # defines itself too late (web/v2/e2e/budgets/plugins); the break-glass account `e2e`
  # as above, for budgets/global-setup.ts. The build of web/v2 is bench-web's when it says
  # so (K_WEB2_BUILT: the one it weighed). `kartenn-server migrate` deploys the core and the
  # modules; the migrations of the core `kartenn-bench seed` runs again find them applied.
  release=${CARGO_TARGET_DIR:-target}/release
  cargo build -q --release -p kartenn-server -p kartenn-bench
  [ -n "${K_WEB2_BUILT:-}" ] ||
    pnpm -C web/v2 --filter @kartenn/shell --filter "./plugins/*" build >/dev/null
  rm -rf "$state/plugins-v2"
  mkdir -p "$state/plugins-v2"
  cp -R web/v2/e2e/budgets/plugins/. web/v2/dist/plugins/. "$state/plugins-v2/"
  database=${K_BENCH_DATABASE_URL:-postgres://kartenn@127.0.0.1:5433/kartenn_bench}
  PGOPTIONS='-c client_min_messages=warning' psql "${database%/*}/postgres" -q \
    -c "DROP DATABASE IF EXISTS \"${database##*/}\" WITH (FORCE)" \
    -c "CREATE DATABASE \"${database##*/}\""
  "$release/kartenn-server" migrate "$database" >/dev/null
  "$release/kartenn-bench" seed "$database" "${BENCH_SEED:-42}" ${BENCH_SMALL:+small} \
    >"$state/budgets-v2.env"
  "$release/kartenn-server" breakglass create "$database" e2e 24 "$state/kartenn-keys" \
    >"$state/breakglass-v2.txt"
  exec env SSL_CERT_FILE="$state/ca.pem" \
    "$release/kartenn-server" run "$database" web/v2/e2e/kartenn.toml
  ;;
*)
  echo "usage: serve.sh idp|kartenn|budgets" >&2
  exit 2
  ;;
esac
