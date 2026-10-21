#!/usr/bin/env bash
# Continuous integration of kartenn, independent of any forge (spec 015, E14): one line per
# step, the detail of a failure in a log file. Prerequisites: Nix with flakes, devenv, and
# the development PostgreSQL running (`devenv up -d`, or `devenv up` in another shell).
#
#   scripts/ci.sh            every step
#   scripts/ci.sh --no-nix   without `nix flake check` (image and virtual machine)
set -uo pipefail
cd "$(dirname "$0")/.."

nix_checks=1
[ "${1:-}" != --no-nix ] || nix_checks=0
logs=$(mktemp -d -t kartenn-ci.XXXXXX)
failed=0

# step <name> <command…>: runs the command in the devenv shell, prints OK or KO.
step() {
  local name=$1
  shift
  local start=$SECONDS
  if devenv shell -- "$@" >"$logs/$name.log" 2>&1; then
    echo "OK  $name ($((SECONDS - start)) s)"
  else
    echo "KO  $name ($((SECONDS - start)) s) — $logs/$name.log"
    tail -5 "$logs/$name.log" | sed 's/^/    /'
    failed=1
  fi
}

if ! devenv shell -- pg_isready -q -h 127.0.0.1 >/dev/null 2>&1; then
  echo "KO  PostgreSQL de développement injoignable : lancer \`devenv up -d\` d'abord"
  exit 1
fi

step hakari bash -c 'cargo hakari generate --diff && cargo hakari manage-deps --dry-run'
step sqlx-offline k-heavy --pg cargo sqlx prepare --workspace --check -- --all-targets
step rust k-check
step web k-web-check
step e2e k-web-e2e
[ "$nix_checks" -eq 0 ] || step nix k-heavy --all nix flake check --max-jobs 1 --cores 12

[ "$failed" -eq 0 ] && echo "OK — CI verte (journaux : $logs)" || echo "KO — CI rouge (journaux : $logs)"
exit "$failed"
