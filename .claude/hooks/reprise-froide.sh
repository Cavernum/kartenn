#!/usr/bin/env bash
# Hook UserPromptSubmit : après plus d'une heure sans réponse, le cache de prompt a
# expiré et la requête suivante réécrirait tout le contexte. Au-delà de 150 k tokens,
# bloque le prompt une fois et propose de compacter ; renvoyer le prompt passe outre.
input=$(cat)
prompt=$(jq -r '.prompt // ""' <<<"$input")
case "$prompt" in /compact* | /clear*) exit 0 ;; esac
session=$(jq -r '.session_id // "inconnue"' <<<"$input")
marker="${XDG_RUNTIME_DIR:-/tmp}/kartenn-claude/$session.froid"
if [ -e "$marker" ]; then rm -f "$marker"; exit 0; fi
read -r ctx idle _ < <(python3 "$(dirname "$0")/etat-session.py" "$(jq -r '.transcript_path // ""' <<<"$input")")
if [ "${idle:-0}" -ge 3600 ] && [ "${ctx:-0}" -ge 150000 ]; then
  mkdir -p "$(dirname "$marker")" && touch "$marker"
  echo "Cache expiré ($((idle / 60)) min d'inactivité) et contexte de $((ctx / 1000)) k : ce prompt réécrirait tout le cache. Lance /compact (ou /clear puis /tache), ou renvoie le prompt pour passer outre." >&2
  exit 2
fi
exit 0
