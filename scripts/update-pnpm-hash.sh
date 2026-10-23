#!/usr/bin/env dash
# Recomputes the hash of the pnpm dependencies in nix/package/web.nix after a change of
# web/pnpm-lock.yaml (spec 015, design « Flake »): builds the dependencies with a dummy
# hash, reads the one Nix expects from the error and writes it back.
# usage: scripts/update-pnpm-hash.sh (from the repository root)
set -euo pipefail
file=nix/package/web.nix
dummy="sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
current=$(grep -oE 'hash = "sha256-[^"]+"' "$file" | head -1 | cut -d'"' -f2)
sed -i "s#$current#$dummy#" "$file"
log=$(mktemp)
if nix build --no-link ".#kartenn-web.pnpmDeps" >"$log" 2>&1; then
  sed -i "s#$dummy#$current#" "$file"
  echo "unchanged: the dummy hash built, restored $current"
  exit 0
fi
got=$(grep -oE 'got: +sha256-[A-Za-z0-9+/=]+' "$log" | awk '{print $2}' | tail -1)
if [ -z "$got" ]; then
  sed -i "s#$dummy#$current#" "$file"
  echo "no hash found in the build log ($log)" >&2
  exit 1
fi
sed -i "s#$dummy#$got#" "$file"
echo "pnpm dependencies: $got"
