#!/usr/bin/env bash
# POSIX sh (linted as dash) but started by bash: Claude Code runs it outside devenv,
# where dash may be missing.
# Status line du projet : modèle, dossier, taille du contexte et temps depuis la dernière
# réponse. Le cache de prompt expire après 1 h d'inactivité : au-delà de 50 min, alerte
# pour compacter tant qu'il est chaud (CLAUDE.md, « Économie de contexte »).
input=$(cat)
field() { printf '%s\n' "$input" | jq -r "$1"; }
model=$(field '.model.display_name // "?"')
dir=$(basename "$(field '.workspace.current_dir // "."')")
transcript=$(field '.transcript_path // ""')
read -r ctx idle _ <<EOF
$(python3 "$(dirname "$0")/hooks/etat-session.py" "$transcript")
EOF
line="[$model] 📁 $dir"
if [ "${ctx:-0}" -gt 0 ]; then
  line="$line · ctx $((ctx / 1000)) k"
  min=$((idle / 60))
  if [ "$idle" -ge 3600 ]; then
    line="$line · ⚠ cache expiré (${min} min) → /compact"
  elif [ "$idle" -ge 3000 ] && [ "$ctx" -ge 100000 ]; then
    line="$line · ⏳ cache expire dans ~$((60 - min)) min → /compact"
  elif [ "$ctx" -ge 300000 ]; then
    line="$line · contexte lourd → /compact"
  fi
fi
echo "$line"
