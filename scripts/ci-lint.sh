#!/usr/bin/env bash
# Lint of the GitHub workflows and of the release scripts (ADR 0033): actionlint, which
# also passes the `run:` scripts to ShellCheck, zizmor (security audits of the workflows),
# and ShellCheck on the scripts. zizmor runs offline unless GH_TOKEN is set: its online audits then also
# check the pinned commits (impostor commits, known vulnerable actions).
#
#   scripts/ci-lint.sh    (devenv: k-ci-lint)
set -euo pipefail
cd "$(dirname "$0")/.."

for tool in actionlint zizmor shellcheck; do
  command -v "$tool" >/dev/null || {
    echo "KO — $tool introuvable (devenv shell)"
    exit 1
  }
done
actionlint
online=(--offline)
[ -z "${GH_TOKEN:-}" ] || online=()
zizmor --quiet "${online[@]}" .github/workflows .github/dependabot.yml
shellcheck scripts/*.sh
echo "OK — actionlint, zizmor et shellcheck sans constat"
