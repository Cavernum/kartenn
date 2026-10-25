#!/usr/bin/env dash
# Servers of the end-to-end journeys (spec 007, REQ-007-34), started by Playwright
# (`webServer`, in this order, before global-setup.ts):
# - `serve.sh idp`: builds the binaries, writes the material of web/e2e/.state (authority,
#   loopback certificate, client secret, key ring), then serves the issuer on 9444;
# - `serve.sh kartenn`: builds the shell and the plugins of web/plugins, gathers them under
#   web/e2e/.state/plugins, recreates the kartenn_e2e database, migrates it,
#   creates the break-glass account `e2e` (its proofs in web/e2e/.state/breakglass.txt),
#   which the subcommand makes a member of the delivered group `secours` (K17, spec 025),
#   then serves kartenn-server on 9443.
# The end-to-end sessions of the Python CLI (spec 023, `k-py-e2e`) reuse `idp`, then:
# - `serve.sh cli-prepare`: builds kartenn-server without the `policy` module (copied to
#   web/e2e/.state/kartenn-server-labo), then with every module; writes the material of
#   web/e2e/.state as `idp` does (kept when present); recreates the kartenn_cli
#   and kartenn_cli_labo databases, migrates them and creates the break-glass account `e2e`
#   of each (web/e2e/.state/breakglass-cli.txt, breakglass-cli-labo.txt);
# - `serve.sh cli` and `serve.sh cli-labo`: serve them on 9443 and 9445, without the shell.
# The break-glass account is the only write outside the API, as for an operator bootstrapping
# an instance (REQ-005-14). Everything else goes through global-setup.ts, or the CLI itself
# (clients/python/e2e).
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
cli-prepare)
  cargo build -q -p kartenn-server --no-default-features --features machines
  mkdir -p "$state"
  cp "$bin/kartenn-server" "$state/kartenn-server-labo"
  cargo build -q -p kartenn-dev-idp -p kartenn-server
  # The material is written once and kept: `serve.sh idp` finds it as it is.
  "$bin/kartenn-dev-idp" certs "$state"
  base=${DATABASE_URL%/*}
  for name in kartenn_cli kartenn_cli_labo; do
    psql "$DATABASE_URL" -q -c "DROP DATABASE IF EXISTS $name WITH (FORCE)" \
      -c "CREATE DATABASE $name"
  done
  "$bin/kartenn-server" migrate "$base/kartenn_cli" >/dev/null
  "$state/kartenn-server-labo" migrate "$base/kartenn_cli_labo" >/dev/null
  "$bin/kartenn-server" breakglass create "$base/kartenn_cli" e2e 24 \
    "$state/kartenn-keys" >"$state/breakglass-cli.txt"
  "$state/kartenn-server-labo" breakglass create "$base/kartenn_cli_labo" e2e 24 \
    "$state/kartenn-keys" >"$state/breakglass-cli-labo.txt"
  ;;
cli)
  exec env SSL_CERT_FILE="$state/ca.pem" \
    "$bin/kartenn-server" run "${DATABASE_URL%/*}/kartenn_cli" clients/python/e2e/kartenn.toml
  ;;
cli-labo)
  exec env SSL_CERT_FILE="$state/ca.pem" \
    "$state/kartenn-server-labo" run "${DATABASE_URL%/*}/kartenn_cli_labo" \
    clients/python/e2e/labo.toml
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
  PGOPTIONS='-c client_min_messages=warning' psql "${database%/*}/postgres" -q -c "DROP DATABASE IF EXISTS \"${database##*/}\" WITH (FORCE)" \
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
  echo "usage: serve.sh idp|kartenn|cli-prepare|cli|cli-labo|budgets" >&2
  exit 2
  ;;
esac
