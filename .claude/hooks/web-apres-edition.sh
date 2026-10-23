#!/usr/bin/env bash
# POSIX sh (linted as dash) but started by bash: Claude Code runs it outside devenv,
# where dash may be missing.
# Hook PostToolUse (Edit|Write) des fichiers web/** (ADR 0039, § 4) : Prettier, puis
# Stylelint (feuilles et blocs de style, là où l'espace en a la configuration) et ESLint,
# sur le seul fichier touché, avec les outils de son espace pnpm (web/ ou web/v2/). Une
# ligne en succès ; en échec, les diagnostics sont rendus à l'agent (code 2). Ni tests ni
# build : ils restent à k-web-check.
input=$(cat)
field() { printf '%s\n' "$input" | jq -r "$1"; }
file=$(field '.tool_input.file_path // ""')
[ -n "$file" ] && [ -f "$file" ] || exit 0
top=$(git -C "$(dirname "$file")" rev-parse --show-toplevel 2>/dev/null) || exit 0
rel=${file#"$top"/}
case "$rel" in
  web/v2/*) ws=web/v2 ;;
  web/*) ws=web ;;
  *) exit 0 ;;
esac
path=${rel#"$ws"/}
bin="$top/$ws/node_modules/.bin"
case "$path" in node_modules/* | */node_modules/* | dist/* | */dist/*) exit 0 ;; esac
if [ ! -x "$bin/prettier" ] || ! command -v node >/dev/null 2>&1; then
  echo "web : outils absents ($ws/node_modules ou node) ; lancer devenv shell" >&2
  exit 0
fi
cd "$top/$ws" || exit 0
log=$(mktemp -t kartenn-web-hook.XXXXXX)
trap 'rm -f "$log"' EXIT
steps=""
# ko <outil> : les diagnostics, au plus 30 lignes, à l'agent.
ko() {
  {
    echo "$1 en échec sur $rel :"
    grep -vE '^\s*$' "$log" | sed "s#$top/##" | head -30
  } >&2
  exit 2
}
"$bin/prettier" --write --ignore-unknown --log-level warn "$path" >"$log" 2>&1 || ko prettier
steps="prettier"
case "$path" in
  *.css | *.vue)
    if [ -f stylelint.config.js ] && [ -x "$bin/stylelint" ]; then
      "$bin/stylelint" --fix --allow-empty-input "$path" >"$log" 2>&1 || ko stylelint
      steps="$steps, stylelint"
    fi
    ;;
esac
case "$path" in
  *.vue | *.ts | *.tsx | *.js | *.mjs)
    "$bin/eslint" --fix --no-warn-ignored "$path" >"$log" 2>&1 || ko eslint
    steps="$steps, eslint"
    ;;
esac
jq -n --arg m "web : $rel conforme ($steps)" '{systemMessage: $m}'
