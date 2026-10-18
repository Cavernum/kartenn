#!/usr/bin/env bash
# Enchaîne les tâches non cochées d'une spec, chacune dans une session Claude Code neuve
# (`claude -p "/tache NNN-M"`) : chaque tâche repart d'un contexte minimal au lieu
# d'hériter de l'historique des précédentes (CLAUDE.md, « Économie de contexte »).
#
#   scripts/claude-taches.sh <NNN> [nombre max de tâches]
#
# S'arrête au premier échec, à une tâche non cochée après son passage (arbitrage demandé)
# ou à un arbre de travail modifié. Journal : .devenv/state/claude-taches/.
# CLAUDE_PERMISSION_MODE (défaut : auto) est passé à --permission-mode.
set -euo pipefail

spec=${1:?usage: $0 <NNN> [max]}
max=${2:-1000}
cd "$(git rev-parse --show-toplevel)"
tasks=$(ls specs/"$spec"-*/tasks.md)
logs=.devenv/state/claude-taches
mkdir -p "$logs"

next_task() { grep -m1 -oE '^- \[ \] [0-9]+\.' "$tasks" | grep -oE '[0-9]+' || true; }

if [ -n "$(git status --porcelain)" ]; then
  echo "Arbre de travail modifié : committe ou remise les changements avant de lancer." >&2
  exit 1
fi

done_count=0
while [ "$done_count" -lt "$max" ]; do
  n=$(next_task)
  if [ -z "$n" ]; then echo "Spec $spec : toutes les tâches sont cochées."; break; fi
  id="$spec-$n"
  log="$logs/$(date +%Y%m%dT%H%M%S)-$id.json"
  echo "== $id"
  if ! claude -p "/tache $id" --output-format json \
      --permission-mode "${CLAUDE_PERMISSION_MODE:-auto}" \
      --append-system-prompt "Mode autonome, sans utilisateur : ne pose aucune question. Si un arbitrage est nécessaire, expose-le et arrête-toi sans cocher la tâche." \
      >"$log"; then
    echo "Échec de claude sur $id (journal : $log)" >&2; exit 1
  fi
  jq -r '"\(.result)\n-- \(.num_turns) tours, \(.total_cost_usd // "?") USD"' "$log"
  if grep -qE "^- \[ \] $n\." "$tasks"; then
    echo "Tâche $id non cochée : arbitrage demandé ou tâche inachevée. Arrêt." >&2; exit 2
  fi
  if [ -n "$(git status --porcelain)" ]; then
    echo "Changements non commités après $id. Arrêt." >&2; exit 3
  fi
  done_count=$((done_count + 1))
done
