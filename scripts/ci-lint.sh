#!/usr/bin/env dash
# Lint of the GitHub workflows and of the scripts (ADR 0033): actionlint, which also passes
# the `run:` scripts to ShellCheck, zizmor (security audits of the workflows), and
# ShellCheck as dash on the scripts of the repository and, from the devenv shell or the
# pre-commit hook, on the scripts of devenv.nix (K_DASH_SCRIPTS). zizmor runs offline
# unless GH_TOKEN is set: its online audits then also check the pinned commits (impostor
# commits, known vulnerable actions).
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
offline=--offline
[ -z "${GH_TOKEN:-}" ] || offline=
zizmor --quiet ${offline:+"$offline"} .github/workflows .github/dependabot.yml
shellcheck --shell=dash scripts/*.sh web/e2e/serve.sh .claude/*.sh .claude/hooks/*.sh
devenv=
if [ -n "${K_DASH_SCRIPTS:-}" ]; then
  # shellcheck disable=SC2086 # a list of store paths, without blanks
  shellcheck --shell=dash $K_DASH_SCRIPTS
  devenv=" (scripts de devenv.nix compris)"
fi
echo "OK — actionlint, zizmor et shellcheck sans constat$devenv"
