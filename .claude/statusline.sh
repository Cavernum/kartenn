#!/usr/bin/env bash
# Status line du projet : modèle, dossier, taille du contexte et temps depuis la dernière
# réponse. Le cache de prompt expire après 1 h d'inactivité : au-delà de 50 min, alerte
# pour compacter tant qu'il est chaud (CLAUDE.md, « Économie de contexte »).
input=$(cat)
model=$(jq -r '.model.display_name // "?"' <<<"$input")
dir=$(basename "$(jq -r '.workspace.current_dir // "."' <<<"$input")")
transcript=$(jq -r '.transcript_path // ""' <<<"$input")
read -r ctx idle _ < <(python3 "$(dirname "$0")/hooks/etat-session.py" "$transcript")
line="[$model] 📁 $dir"
if [ "${ctx:-0}" -gt 0 ]; then
  line+=" · ctx $((ctx / 1000)) k"
  min=$((idle / 60))
  if [ "$idle" -ge 3600 ]; then
    line+=" · ⚠ cache expiré (${min} min) → /compact"
  elif [ "$idle" -ge 3000 ] && [ "$ctx" -ge 100000 ]; then
    line+=" · ⏳ cache expire dans ~$((60 - min)) min → /compact"
  elif [ "$ctx" -ge 300000 ]; then
    line+=" · contexte lourd → /compact"
  fi
fi
echo "$line"
