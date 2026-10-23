#!/usr/bin/env dash
# Continuous integration of kartenn, independent of any forge (spec 015, E14; ADR 0033): one
# line per step, the detail of a failure in a log file. The GitHub workflow
# (.github/workflows/ci.yml) runs this same script. Prerequisites: Nix with flakes, devenv,
# and the development PostgreSQL running (`devenv up -d postgres`, or `devenv up` in
# another shell).
#
#   scripts/ci.sh            every step
#   scripts/ci.sh --no-nix   without `nix flake check` (image and virtual machine)
#
# Under GitHub Actions (GITHUB_ACTIONS=true), the log of a failed step is also printed, in
# a collapsed group.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

nix_checks=1
[ "${1:-}" != --no-nix ] || nix_checks=0
logs=$(mktemp -d -t kartenn-ci.XXXXXX)
failed=0

# step <name> <command…>: runs the command in the devenv shell, prints OK or KO.
step() {
  local name=$1 start
  shift
  start=$(date +%s)
  if devenv shell -- "$@" >"$logs/$name.log" 2>&1; then
    echo "OK  $name ($(($(date +%s) - start)) s)"
  else
    echo "KO  $name ($(($(date +%s) - start)) s) — $logs/$name.log"
    tail -5 "$logs/$name.log" | sed 's/^/    /'
    if [ "${GITHUB_ACTIONS:-}" = true ]; then
      echo "::group::journal de $name"
      cat "$logs/$name.log"
      echo "::endgroup::"
    fi
    failed=1
  fi
}

if ! devenv shell -- pg_isready -q -h 127.0.0.1 >/dev/null 2>&1; then
  echo "KO  PostgreSQL de développement injoignable : lancer \`devenv up -d postgres\` d'abord"
  exit 1
fi

# The scripts of devenv.nix too (K_DASH_SCRIPTS), which the lint job does not see.
step lint k-ci-lint
step hakari dash -c 'cargo hakari generate --diff && cargo hakari manage-deps --dry-run'
# The offline metadata is compared with a database at the last migration.
step sqlx-offline k-heavy --pg dash -c \
  'sqlx migrate run --source crates/kartenn-store-pg/migrations && cargo sqlx prepare --workspace --check -- --all-targets'
step rust k-check
step web k-web-check
# Python clients (spec 023): lint, types and tests, then the wheels and their licences.
step python k-py-check
step python-wheels scripts/py-wheels.sh
step e2e k-web-e2e
step e2e-cli k-py-e2e
[ "$nix_checks" -eq 0 ] || step nix k-heavy --all nix flake check --max-jobs 1 --cores 12

[ "$failed" -eq 0 ] && echo "OK — CI verte (journaux : $logs)" || echo "KO — CI rouge (journaux : $logs)"
exit "$failed"
