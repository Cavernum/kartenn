#!/usr/bin/env bash
# Hook Stop : quand le tour qui s'achève a commité une tâche (commit touchant un
# specs/*/tasks.md depuis le dernier prompt), conseille de compacter tant que le cache
# est chaud. Compacter plus tard, cache expiré, relit tout le contexte au plein tarif.
input=$(cat)
[ "$(jq -r '.stop_hook_active // false' <<<"$input")" = true ] && exit 0
transcript=$(jq -r '.transcript_path // ""' <<<"$input")
cd "$(jq -r '.cwd // "."' <<<"$input")" 2>/dev/null || exit 0
read -r ctx _ prompt < <(python3 "$(dirname "$0")/etat-session.py" "$transcript")
[ "${prompt:-0}" -gt 0 ] && [ "${ctx:-0}" -ge 60000 ] || exit 0
task=$(git log --since="@$prompt" --format=%s -- 'specs/*/tasks.md' 2>/dev/null | head -1)
[ -n "$task" ] || exit 0
jq -n --arg m "Tâche commitée (« $task ») — contexte $((ctx / 1000)) k : /compact (ou /clear) maintenant, tant que le cache est chaud." \
  '{systemMessage: $m}'
